//! Embedded mpv playback (the approach stremio-shell-ng uses): mpv draws
//! into the app window via `--wid`, its child window is pushed to the
//! bottom of the z-order, and the webview's own background is made
//! transparent (alpha 0) so the HTML player controls sit on top of the
//! video. Crucially the *window* is never `transparent: true` - that is
//! what broke click input app-wide in the earlier attempt (PLAN.md).

use std::sync::Arc;
use std::time::Duration;

use mpv_ipc::EmbeddedMpv;
use serde_json::Value;
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::{mpsc, Mutex};

#[derive(Default)]
pub struct PlayerState {
    mpv: Mutex<Option<Arc<EmbeddedMpv>>>,
}

/// Starts mpv inside the window if it isn't running yet and makes the
/// webview transparent over it. Idempotent - the player calls it on every
/// open.
#[tauri::command]
pub async fn mpv_start(app: AppHandle, window: WebviewWindow, state: State<'_, PlayerState>) -> Result<(), String> {
    tracing::debug!("mpv_start invoked");
    let mut slot = state.mpv.lock().await;
    if slot.is_none() {
        let wid = window_handle(&window)?;
        let (events_tx, mut events_rx) = mpsc::unbounded_channel::<Value>();
        let mpv = EmbeddedMpv::spawn(wid, events_tx).await.map_err(|err| err.to_string())?;
        if let Some(pid) = mpv.pid() {
            tokio::spawn(push_mpv_window_to_bottom(wid, pid));
        }
        let mpv = Arc::new(mpv);
        *slot = Some(mpv.clone());

        let forward_app = app.clone();
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                if let Err(err) = forward_app.emit("mpv-event", event) {
                    tracing::warn!(%err, "failed to forward mpv event");
                }
            }
            // mpv exited or its pipe broke: forget it so the next
            // mpv_start respawns, and tell the player.
            tracing::warn!("embedded mpv went away");
            let state = forward_app.state::<PlayerState>();
            let mut slot = state.mpv.lock().await;
            if slot.as_ref().is_some_and(|current| Arc::ptr_eq(current, &mpv)) {
                *slot = None;
            }
            let _ = forward_app.emit("mpv-exit", ());
        });
        tracing::info!(wid, "embedded mpv started");
    }
    window.set_background_color(Some(Color(0, 0, 0, 0))).map_err(|err| err.to_string())?;
    Ok(())
}

/// One raw mpv IPC command (`["loadfile", url]`, `["observe_property", 1,
/// "time-pos"]`...) - the frontend owns playback logic.
#[tauri::command]
pub async fn mpv_command(state: State<'_, PlayerState>, args: Vec<Value>) -> Result<Value, String> {
    tracing::debug!(?args, "mpv_command");
    let mpv = state.mpv.lock().await.clone().ok_or_else(|| "mpv is not running".to_string())?;
    mpv.command(&args).await.map_err(|err| err.to_string())
}

/// Unloads the file and makes the webview opaque again. mpv itself stays
/// idle so the next episode starts without a respawn.
#[tauri::command]
pub async fn mpv_stop(window: WebviewWindow, state: State<'_, PlayerState>) -> Result<(), String> {
    tracing::debug!("mpv_stop invoked");
    if let Some(mpv) = state.mpv.lock().await.clone() {
        if let Err(err) = mpv.command(&["stop".into()]).await {
            tracing::warn!(%err, "mpv stop failed");
        }
    }
    window.set_background_color(None).map_err(|err| err.to_string())?;
    tracing::info!("mpv_stop completed");
    Ok(())
}

#[cfg(windows)]
fn window_handle(window: &WebviewWindow) -> Result<i64, String> {
    window.hwnd().map(|hwnd| hwnd.0 as i64).map_err(|err| err.to_string())
}

#[cfg(not(windows))]
fn window_handle(_window: &WebviewWindow) -> Result<i64, String> {
    Err("embedded mpv is only implemented on Windows".into())
}

/// mpv creates its own child window inside `--wid` once it starts, which
/// lands on top of the webview's. Wait for it (matched by process id) and
/// send it to the bottom so the webview stays on top.
#[cfg(windows)]
async fn push_mpv_window_to_bottom(parent: i64, pid: u32) {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetWindowThreadProcessId, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    struct Search {
        pid: u32,
        found: HWND,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if owner == search.pid {
            search.found = hwnd;
            return 0;
        }
        1
    }

    // Raw handles aren't Send, so the lookup stays in a sync helper.
    let find = || {
        let mut search = Search { pid, found: std::ptr::null_mut() };
        // SAFETY: `visit` only writes through the `Search` pointer, which
        // outlives the synchronous enumeration.
        unsafe { EnumChildWindows(parent as HWND, Some(visit), &mut search as *mut Search as LPARAM) };
        (!search.found.is_null()).then(|| search.found as isize)
    };
    for _ in 0..100 {
        if let Some(found) = find() {
            // SAFETY: plain z-order change on a live window handle.
            let ok = unsafe { SetWindowPos(found as HWND, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
            tracing::info!(ok, "mpv window moved below the webview");
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tracing::error!("mpv never created its window - video will not be visible");
}

#[cfg(not(windows))]
async fn push_mpv_window_to_bottom(_parent: i64, _pid: u32) {}
