//! Hardware media keys (play/pause, next, previous) while the player is open.
//!
//! Registered as global shortcuts so they work with the window unfocused,
//! but only between `set_media_keys(true)` and `(false)` - the player calls
//! them on mount/unmount so the keys aren't taken from other apps
//! (Spotify...) the rest of the time. The webview's own MediaSession can't
//! be used: it needs a real media element, and playback is in mpv.

use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

const KEYS: [(Code, &str); 3] = [(Code::MediaPlayPause, "play-pause"), (Code::MediaTrackNext, "next"), (Code::MediaTrackPrevious, "previous")];

/// The plugin, with the handler that forwards a pressed key to the frontend
/// as a `media-key` event.
pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if let Some((_, name)) = KEYS.iter().find(|(code, _)| shortcut.key == *code) {
                tracing::debug!(key = name, "media key pressed");
                let _ = app.emit("media-key", *name);
            }
        })
        .build()
}

#[tauri::command]
pub fn set_media_keys(app: AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(enabled, "set_media_keys invoked");
    let shortcuts = app.global_shortcut();
    for (code, name) in KEYS {
        let shortcut = Shortcut::new(None, code);
        if enabled {
            if !shortcuts.is_registered(shortcut) {
                shortcuts.register(shortcut).map_err(|err| format!("couldn't register {name}: {err}"))?;
            }
        } else if shortcuts.is_registered(shortcut) {
            shortcuts.unregister(shortcut).map_err(|err| format!("couldn't unregister {name}: {err}"))?;
        }
    }
    Ok(())
}
