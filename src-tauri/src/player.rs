//! Embedded mpv playback (the approach stremio-shell-ng uses, here with
//! in-process libmpv): mpv draws
//! into the app window via `--wid`, its child window is pushed to the
//! bottom of the z-order, and the webview's own background is made
//! transparent (alpha 0) so the HTML player controls sit on top of the
//! video. Crucially the *window* is never `transparent: true` - that is
//! what broke click input app-wide in the earlier attempt (PLAN.md).

use std::sync::Arc;
use std::time::Duration;

use mpv_player::EmbeddedMpv;
use serde_json::Value;
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::{mpsc, Mutex};

#[derive(Default)]
pub struct PlayerState {
    mpv: Mutex<Option<Arc<EmbeddedMpv>>>,
    /// Player views currently attached (`mpv_start` minus `mpv_stop`). On an
    /// episode change the new view's start can land before the old view's
    /// stop; only the last stop may unload the file and make the webview
    /// opaque again, or the video ends up hidden behind an opaque webview
    /// (or the new file gets stopped).
    attached: std::sync::atomic::AtomicUsize,
}

/// Whether libmpv can be loaded - the player falls back to HLS without it.
/// The frontend asks once per run (and again when the path override changes).
#[tauri::command]
pub async fn mpv_available() -> bool {
    let found = tokio::task::spawn_blocking(mpv_player::is_available).await.unwrap_or(false);
    tracing::info!(found, "libmpv availability checked");
    found
}

/// Points libmpv loading at `path` (a `libmpv-2.dll` or its folder; empty/
/// `None`: the default search) - the Settings override.
#[tauri::command]
pub fn set_mpv_path(path: Option<String>) {
    let path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    tracing::debug!(?path, "set_mpv_path invoked");
    mpv_player::set_library_path(path.map(std::path::PathBuf::from));
}

/// Delay between the page finishing its first load and the mpv pre-spawn,
/// so it doesn't compete with the first paint and the home page's requests.
const WARM_MPV_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);
static MPV_WARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `warm_mpv` once per app run, `WARM_MPV_DELAY` after the page's first
/// load finished - dev reloads fire page loads again. Spawning mpv into the
/// window before the page had loaded stalled WebView2 (launch-to-first-script
/// 0.5-2s without it, 3-34s with it).
pub fn warm_mpv_once(app: AppHandle) {
    if MPV_WARMED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tracing::debug!(delay_ms = WARM_MPV_DELAY.as_millis() as u64, "page loaded, scheduling the mpv pre-spawn");
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(WARM_MPV_DELAY).await;
        warm_mpv(app);
    });
}

/// Spawns the embedded mpv early - idle, below the still-opaque webview - so
/// the first episode skips libmpv load + font install. Skipped when libmpv
/// can't be found (the player then uses the HLS fallback, or `mpv_start`
/// spawns lazily after a Settings path override).
pub fn warm_mpv(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let available = tokio::task::spawn_blocking(mpv_player::is_available).await.unwrap_or(false);
        if !available {
            tracing::info!("libmpv not found, embedded mpv not pre-spawned");
            return;
        }
        let Some(window) = app.get_webview_window("main").or_else(|| app.webview_windows().into_values().next()) else {
            tracing::warn!("no window to pre-spawn embedded mpv in");
            return;
        };
        let started = std::time::Instant::now();
        let state = app.state::<PlayerState>();
        match ensure_mpv(&app, &window, &state).await {
            Ok(()) => tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "embedded mpv pre-spawned after page load"),
            Err(err) => tracing::warn!(%err, "couldn't pre-spawn embedded mpv, the player will start it"),
        }
    });
}

/// Starts mpv inside `window` unless it's already running. The webview's
/// background is left alone, so an idle mpv stays hidden behind it.
async fn ensure_mpv(app: &AppHandle, window: &WebviewWindow, state: &PlayerState) -> Result<(), String> {
    let mut slot = state.mpv.lock().await;
    if slot.is_none() {
        let wid = window_handle(&window)?;
        let (events_tx, mut events_rx) = mpsc::unbounded_channel::<Value>();
        let mut extra_args = match install_fonts().await {
            Ok(dir) => vec![format!("--sub-fonts-dir={}", dir.display())],
            Err(err) => {
                tracing::warn!(%err, "couldn't install bundled subtitle fonts for mpv");
                Vec::new()
            }
        };
        // Overrides embedded.rs's built-in 10 s: sbtl tracks its own buffer
        // (see torrent_engine::MPV_CACHE_PAUSE_WAIT_SECS).
        let pause_wait = torrent_engine::MPV_CACHE_PAUSE_WAIT_SECS;
        tracing::debug!(pause_wait, "mpv cache-pause-wait");
        extra_args.push(format!("--cache-pause-wait={pause_wait}"));
        let mpv = EmbeddedMpv::spawn(wid, &extra_args, events_tx).await.map_err(|err| err.to_string())?;
        tokio::spawn(push_mpv_window_to_bottom(wid));
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
    } else {
        tracing::debug!("embedded mpv already running");
    }
    Ok(())
}

/// Starts mpv inside the window if it isn't running yet (normally it was
/// pre-spawned after the page loads, see `warm_mpv_once`) and makes the webview transparent
/// over it. Idempotent - the player calls it on every open.
#[tauri::command]
pub async fn mpv_start(app: AppHandle, window: WebviewWindow, state: State<'_, PlayerState>) -> Result<(), String> {
    tracing::debug!("mpv_start invoked");
    ensure_mpv(&app, &window, &state).await?;
    window.set_background_color(Some(Color(0, 0, 0, 0))).map_err(|err| err.to_string())?;
    let attached = state.attached.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    tracing::debug!(attached, "mpv_start completed");
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

/// libmpv's version, when the player has been started this run.
pub async fn mpv_version(state: &PlayerState) -> Option<String> {
    let mpv = state.mpv.lock().await.clone()?;
    let version = mpv.command(&["get_property".into(), "mpv-version".into()]).await.ok()?;
    version.as_str().map(str::to_string)
}

/// Unloads the file and makes the webview opaque again. mpv itself stays
/// idle so the next episode starts without a respawn.
#[tauri::command]
pub async fn mpv_stop(window: WebviewWindow, state: State<'_, PlayerState>) -> Result<(), String> {
    tracing::debug!("mpv_stop invoked");
    let previous = state
        .attached
        .fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| Some(n.saturating_sub(1)))
        .unwrap_or(0);
    if previous > 1 {
        // Another player view already took over (episode change): its
        // loadfile replaces this file, and the webview must stay transparent.
        tracing::debug!(still_attached = previous - 1, "mpv_stop skipped, a newer player is attached");
        return Ok(());
    }
    if let Some(mpv) = state.mpv.lock().await.clone() {
        if let Err(err) = mpv.command(&["stop".into()]).await {
            tracing::warn!(%err, "mpv stop failed");
        }
        reset_per_file_state(&mpv).await;
    }
    window.set_background_color(None).map_err(|err| err.to_string())?;
    tracing::info!("mpv_stop completed");
    Ok(())
}

/// Per-file playback state mpv keeps across `stop`/`loadfile` - reset so
/// nothing (an A-B loop, a speed or delay tweak, a pause) leaks into the
/// next file played by the persistent mpv.
fn per_file_defaults() -> [(&'static str, Value); 6] {
    [
        ("ab-loop-a", Value::from("no")),
        ("ab-loop-b", Value::from("no")),
        ("speed", Value::from(1.0)),
        ("sub-delay", Value::from(0.0)),
        ("audio-delay", Value::from(0.0)),
        ("pause", Value::from(false)),
    ]
}

async fn reset_per_file_state(mpv: &EmbeddedMpv) {
    for (property, value) in per_file_defaults() {
        if let Err(err) = mpv.command(&["set_property".into(), property.into(), value.clone()]).await {
            tracing::warn!(property, %value, %err, "couldn't reset mpv per-file state");
        }
    }
    tracing::debug!("mpv per-file state reset");
}

/// Longest side of the last-frame thumbnail - matches the headless
/// capture's width (mpv-player's `THUMBNAIL_WIDTH`), both feeding 16:9 cards.
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

/// Where saved frames go unless Settings picked a folder.
fn default_screenshot_dir() -> std::path::PathBuf {
    dirs::picture_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream screenshots")
}

/// The folder frame saves start with.
#[tauri::command]
pub fn default_screenshot_folder() -> String {
    default_screenshot_dir().to_string_lossy().into_owned()
}

/// Saves the frame on screen (subtitles included, at the video's native
/// resolution) as `<folder>/<name>.png` and returns the path. Never
/// overwrites an earlier file.
#[tauri::command]
pub async fn mpv_save_frame(state: State<'_, PlayerState>, folder: Option<String>, name: String) -> Result<String, String> {
    tracing::debug!(?folder, %name, "mpv_save_frame invoked");
    let mpv = state.mpv.lock().await.clone().ok_or_else(|| "mpv is not running".to_string())?;
    let dir = match folder.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        Some(folder) => std::path::PathBuf::from(folder),
        None => default_screenshot_dir(),
    };
    tokio::fs::create_dir_all(&dir).await.map_err(|err| format!("Couldn't create {}: {err}", dir.display()))?;
    let stem: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let stem = stem.trim().chars().take(100).collect::<String>();
    let mut path = dir.join(format!("{stem}.png"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}).png"));
        n += 1;
    }
    mpv.command(&["screenshot-to-file".into(), path.to_string_lossy().into_owned().into(), "subtitles".into()])
        .await
        .map_err(|err| {
            tracing::warn!(%err, "mpv frame save failed");
            err.to_string()
        })?;
    tracing::info!(path = %path.display(), "frame saved");
    Ok(path.to_string_lossy().into_owned())
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
pub(crate) async fn install_fonts() -> std::io::Result<std::path::PathBuf> {
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

/// mpv creates its own child window (class `mpv`) inside `wid` once it
/// starts, which lands on top of the webview's. Wait for it and send it to
/// the bottom so the webview stays on top.
#[cfg(windows)]
async fn push_mpv_window_to_bottom(parent: i64) {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let found = &mut *(lparam as *mut HWND);
        let mut class = [0u16; 16];
        let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        if String::from_utf16_lossy(&class[..len]) == "mpv" {
            *found = hwnd;
            return 0;
        }
        1
    }

    // Raw handles aren't Send, so the lookup stays in a sync helper.
    let find = || {
        let mut found: HWND = std::ptr::null_mut();
        // SAFETY: `visit` only writes through the `HWND` pointer, which
        // outlives the synchronous enumeration.
        unsafe { EnumChildWindows(parent as HWND, Some(visit), &mut found as *mut HWND as LPARAM) };
        (!found.is_null()).then(|| found as isize)
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
async fn push_mpv_window_to_bottom(_parent: i64) {}
