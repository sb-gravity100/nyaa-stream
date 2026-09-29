//! On-disk caches under `<cache_dir>/nyaa-stream`: sizes for Settings, the
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
pub async fn get_cache_sizes() -> Result<CacheSizes, String> {
    tracing::debug!("get_cache_sizes invoked");
    tokio::task::spawn_blocking(|| {
        let root = root();
        CacheSizes {
            nyaa: dir_size(&root.join("nyaa_cache")),
            thumbnails: dir_size(&root.join("thumbnails")),
            hls: dir_size(&hls_dir()),
            torrents: dir_size(&root.join("downloads")) + dir_size(&root.join("engine_cache")),
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
        "hls" | "torrents" => {
            if state.current_torrent.lock().await.is_some() {
                return Err("Stop playback first - this cache is in use.".into());
            }
            if kind == "hls" {
                vec![hls_dir()]
            } else {
                vec![root.join("downloads"), root.join("engine_cache")]
            }
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
