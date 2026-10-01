//! Backup file I/O and on-disk caches under `<cache_dir>/nyaa-stream`: sizes for Settings, the
//! clear buttons, and the HLS segment cache's startup/exit purge.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::AppState;

fn root() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream")
}

/// Transcoded HLS segments - only meaningful while their torrent plays, and
/// they grow to several GB, so they're purged rather than kept.
fn hls_dir() -> PathBuf {
    root().join("hls_cache")
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => dir_size(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Deletes the HLS segment cache. Best effort - a file still held open by a
/// running job just stays until the next purge. Called at startup (a crash
/// skips the exit purge) and on exit.
pub fn purge_hls_cache() {
    let dir = hls_dir();
    let size = dir_size(&dir);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => tracing::info!(bytes = size, "purged hls cache"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => tracing::warn!(%err, "couldn't fully purge hls cache"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSizes {
    nyaa: u64,
    thumbnails: u64,
    hls: u64,
    torrents: u64,
}

/// Bytes each cache occupies, for Settings.
#[tauri::command]
pub async fn get_cache_sizes(state: State<'_, Arc<AppState>>) -> Result<CacheSizes, String> {
    tracing::debug!("get_cache_sizes invoked");
    let app = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let root = root();
        CacheSizes {
            nyaa: dir_size(&root.join("nyaa_cache")),
            thumbnails: dir_size(&root.join("thumbnails")),
            hls: dir_size(&hls_dir()),
            // Allocated bytes - the download cache's files are sparse.
            torrents: app.download_cache.status().0 + dir_size(&root.join("engine_cache")) + dir_size(&crate::resume::root()),
        }
    })
    .await
    .map_err(|err| err.to_string())
}

/// Empties one cache: `nyaa`, `thumbnails`, `hls` or `torrents`. The last
/// two are in use while something plays, so they refuse then.
#[tauri::command]
pub async fn clear_cache(state: State<'_, Arc<AppState>>, kind: String) -> Result<(), String> {
    tracing::debug!(%kind, "clear_cache invoked");
    let root = root();
    let dirs: Vec<PathBuf> = match kind.as_str() {
        "nyaa" => vec![root.join("nyaa_cache")],
        "thumbnails" => vec![root.join("thumbnails")],
        "hls" => {
            if state.current_torrent.lock().await.is_some() {
                return Err("Stop playback first - this cache is in use.".into());
            }
            vec![hls_dir()]
        }
        // Everything but the playing torrent (see download_cache::clear).
        "torrents" => {
            let playing = state.current_torrent.lock().await.clone();
            let app = state.inner().clone();
            let cleared = tokio::task::spawn_blocking({
                let playing = playing.clone();
                move || app.download_cache.clear(playing.as_deref())
            })
            .await
            .map_err(|err| err.to_string())?;
            if let Err(err) = cleared {
                tracing::warn!(%err, "download cache partly cleared");
                return Err(format!("Some downloads couldn't be deleted: {err}"));
            }
            // Resume buffers go too; one attached to the playing torrent is
            // already in memory.
            vec![crate::resume::root()].into_iter().chain(playing.is_none().then(|| root.join("engine_cache"))).collect()
        }
        other => return Err(format!("unknown cache {other}")),
    };
    tokio::task::spawn_blocking(move || {
        for dir in dirs {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(format!("Couldn't clear {}: {err}", dir.display())),
            }
        }
        Ok(())
    })
    .await
    .map_err(|err| err.to_string())??;
    tracing::info!(%kind, "cache cleared");
    Ok(())
}

fn backup_path() -> PathBuf {
    dirs::document_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream-backup.json")
}

/// Writes the frontend's backup JSON (library, progress, settings...) to
/// `<Documents>/nyaa-stream-backup.json` and returns the path.
#[tauri::command]
pub async fn export_backup(json: String) -> Result<String, String> {
    let path = backup_path();
    tracing::debug!(path = %path.display(), bytes = json.len(), "export_backup invoked");
    tokio::fs::write(&path, json).await.map_err(|err| format!("Couldn't write {}: {err}", path.display()))?;
    tracing::info!(path = %path.display(), "backup written");
    Ok(path.to_string_lossy().into_owned())
}

/// Reads the backup written by `export_backup` (also where a copied file
/// from another machine should be placed).
#[tauri::command]
pub async fn import_backup() -> Result<String, String> {
    let path = backup_path();
    tracing::debug!(path = %path.display(), "import_backup invoked");
    tokio::fs::read_to_string(&path).await.map_err(|err| format!("Couldn't read {}: {err}", path.display()))
}
