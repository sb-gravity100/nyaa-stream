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

/// Whether a system `mpv` is on PATH - the player falls back to HLS
/// without it. The frontend asks once per run (and again when the path override changes).
#[tauri::command]
pub async fn mpv_available() -> bool {
    let mut command = tokio::process::Command::new(mpv_ipc::mpv_program());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let found = command
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success());
    tracing::info!(found, "mpv availability checked");
    found
}

/// Points every mpv spawn at `path` (empty/`None`: mpv from PATH) - the
/// Settings override.
#[tauri::command]
pub fn set_mpv_path(path: Option<String>) {
    let path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    tracing::debug!(?path, "set_mpv_path invoked");
    mpv_ipc::set_mpv_path(path.map(std::path::PathBuf::from));
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
        let extra_args = match install_fonts().await {
            Ok(dir) => vec![format!("--sub-fonts-dir={}", dir.display())],
            Err(err) => {
                tracing::warn!(%err, "couldn't install bundled subtitle fonts for mpv");
                Vec::new()
            }
        };
        let mpv = EmbeddedMpv::spawn(wid, &extra_args, events_tx).await.map_err(|err| err.to_string())?;
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
    // Property reads are the stats menu's once-a-second polling - trace, not
    // debug, so they don't bury everything else.
    if args.first().and_then(Value::as_str) == Some("get_property") {
        tracing::trace!(?args, "mpv_command");
    } else {
        tracing::debug!(?args, "mpv_command");
    }
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

/// Longest side of the last-frame thumbnail - matches the headless
/// capture's width (mpv-ipc's `THUMBNAIL_WIDTH`), both feeding 16:9 cards.
const THUMBNAIL_WIDTH: u32 = 640;

/// The current video frame (no subtitles) as a JPEG `width` pixels wide -
/// the player's last-frame thumbnail. Raw bytes over binary IPC.
#[tauri::command]
pub async fn mpv_frame(state: State<'_, PlayerState>, width: Option<u32>) -> Result<tauri::ipc::Response, String> {
    let width = width.unwrap_or(THUMBNAIL_WIDTH);
    tracing::debug!(width, "mpv_frame invoked");
    let path = screenshot(&state, "jpg", "video").await?;
    let jpeg = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let frame = image::open(&path).map_err(|err| err.to_string())?;
        let _ = std::fs::remove_file(&path);
        let height = (width as f64 * frame.height() as f64 / frame.width().max(1) as f64).round() as u32;
        let small = frame.resize_exact(width, height.max(1), image::imageops::FilterType::Triangle).to_rgb8();
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode_image(&small).map_err(|err| err.to_string())?;
        Ok(out)
    })
    .await
    .map_err(|err| err.to_string())??;
    tracing::info!(bytes = jpeg.len(), "mpv frame captured");
    Ok(tauri::ipc::Response::new(jpeg))
}

/// Copies the frame on screen, rendered subtitles included, to the native
/// clipboard at the video's own resolution (Ctrl+C in the player).
#[tauri::command]
pub async fn mpv_copy_frame(state: State<'_, PlayerState>) -> Result<(), String> {
    tracing::debug!("mpv_copy_frame invoked");
    let path = screenshot(&state, "png", "subtitles").await?;
    let (width, height, pixels) = tokio::task::spawn_blocking(move || -> Result<(usize, usize, Vec<u8>), String> {
        let frame = image::open(&path).map_err(|err| err.to_string())?.to_rgba8();
        let _ = std::fs::remove_file(&path);
        Ok((frame.width() as usize, frame.height() as usize, frame.into_raw()))
    })
    .await
    .map_err(|err| err.to_string())??;
    crate::set_clipboard_image(width, height, pixels).await?;
    tracing::info!(width, height, "mpv frame copied to clipboard");
    Ok(())
}

/// App-owned scratch directory for mpv (screenshots, fonts).
fn scratch_dir() -> std::path::PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream").join("mpv")
}

/// Writes one screenshot (`flags`: "video" or "subtitles") to a temp file.
async fn screenshot(state: &PlayerState, extension: &str, flags: &str) -> Result<std::path::PathBuf, String> {
    let mpv = state.mpv.lock().await.clone().ok_or_else(|| "mpv is not running".to_string())?;
    let dir = scratch_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|err| err.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let path = dir.join(format!("frame-{stamp}.{extension}"));
    mpv.command(&["screenshot-to-file".into(), path.to_string_lossy().into_owned().into(), flags.into()])
        .await
        .map_err(|err| {
            tracing::warn!(%err, "mpv screenshot failed");
            err.to_string()
        })?;
    Ok(path)
}

/// The default subtitle style's font (Gandhi Sans) isn't installed on the
/// system - write the bundled copies where mpv's `--sub-fonts-dir` finds
/// them.
async fn install_fonts() -> std::io::Result<std::path::PathBuf> {
    const FONTS: &[(&str, &[u8])] = &[
        ("GandhiSans-Bold.otf", include_bytes!("../../src/assets/fonts/GandhiSans-Bold.otf")),
        ("GandhiSans-BoldItalic.otf", include_bytes!("../../src/assets/fonts/GandhiSans-BoldItalic.otf")),
    ];
    let dir = scratch_dir().join("fonts");
    tokio::fs::create_dir_all(&dir).await?;
    for (name, bytes) in FONTS {
        let path = dir.join(name);
        if tokio::fs::metadata(&path).await.map(|m| m.len() as usize != bytes.len()).unwrap_or(true) {
            tokio::fs::write(&path, bytes).await?;
        }
    }
    Ok(dir)
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
