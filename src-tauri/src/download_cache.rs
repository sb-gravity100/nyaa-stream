//! Download cache: played torrents' files stay in `downloads/` so a re-open
//! reuses every verified piece (libtorrent re-hashes them on add), tracked in
//! `downloads/.cache-index.json` - see PLAN.md "Download cache".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use torrent_engine::TorrentFile;

const INDEX_FILE: &str = ".cache-index.json";
/// Settings -> "Download cache" default.
const DEFAULT_LIMIT_BYTES: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Index {
    /// Cap in bytes; `None` = `DEFAULT_LIMIT_BYTES`, 0 = delete on stop.
    #[serde(default)]
    limit_bytes: Option<u64>,
    #[serde(default)]
    torrents: HashMap<String, Entry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Entry {
    /// What was played, for logs.
    name: String,
    /// File paths relative to `downloads/`, as libtorrent lays them out.
    files: Vec<String>,
    /// Unix seconds; updated whenever a file of it starts streaming.
    last_used: u64,
    /// `<animeId>:<episodeKey>` of each episode played from it.
    #[serde(default)]
    episodes: Vec<String>,
}

pub struct DownloadCache {
    dir: PathBuf,
    index: std::sync::Mutex<Index>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl DownloadCache {
    /// Loads the index from `dir` (`<cache_dir>/nyaa-stream/downloads`); a
    /// missing or unreadable index starts empty.
    pub fn load(dir: PathBuf) -> Self {
        let path = dir.join(INDEX_FILE);
        let index = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                tracing::warn!(%err, path = %path.display(), "download cache index unreadable, starting empty");
                Index::default()
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Index::default(),
            Err(err) => {
                tracing::warn!(%err, path = %path.display(), "failed to read download cache index, starting empty");
                Index::default()
            }
        };
        tracing::info!(entries = index.torrents.len(), "download cache index loaded");
        Self { dir, index: std::sync::Mutex::new(index) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Index> {
        self.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self, index: &Index) {
        let path = self.dir.join(INDEX_FILE);
        let tmp = self.dir.join(format!("{INDEX_FILE}.tmp"));
        let result = serde_json::to_vec_pretty(index)
            .map_err(std::io::Error::other)
            .and_then(|json| {
                std::fs::create_dir_all(&self.dir)?;
                std::fs::write(&tmp, json)?;
                std::fs::rename(&tmp, &path)
            });
        match result {
            Ok(()) => tracing::debug!(entries = index.torrents.len(), "download cache index saved"),
            Err(err) => tracing::error!(%err, path = %path.display(), "failed to save download cache index"),
        }
    }

    /// Records that `info_hash` (`files` on disk) started streaming, for
    /// `episode` (`<animeId>:<episodeKey>`) when known.
    pub fn touch(&self, info_hash: &str, name: &str, files: &[TorrentFile], episode: Option<&str>) {
        let info_hash = info_hash.to_ascii_lowercase();
        let mut index = self.lock();
        let entry = index.torrents.entry(info_hash.clone()).or_insert_with(|| Entry {
            name: name.to_string(),
            files: Vec::new(),
            last_used: 0,
            episodes: Vec::new(),
        });
        entry.files = files.iter().map(|f| f.name.clone()).collect();
        entry.last_used = now_secs();
        if let Some(episode) = episode {
            if !entry.episodes.iter().any(|e| e == episode) {
                entry.episodes.push(episode.to_string());
            }
        }
        tracing::debug!(%info_hash, name, files = entry.files.len(), ?episode, "download cache entry touched");
        self.save(&index);
    }

    /// Everything on disk that belongs to a torrent: its files and
    /// libtorrent's part file (pieces of skipped files).
    fn entry_paths(&self, info_hash: &str, entry: &Entry) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = entry.files.iter().map(|f| self.dir.join(Path::new(f))).collect();
        paths.push(self.dir.join(format!(".{info_hash}.parts")));
        paths
    }

    /// Top-level items in `downloads/` no index entry claims - left by
    /// versions before the index, or a crash before `touch`.
    fn orphans(&self, index: &Index) -> Vec<PathBuf> {
        let mut claimed: Vec<std::ffi::OsString> = vec![INDEX_FILE.into(), format!("{INDEX_FILE}.tmp").into()];
        for (hash, entry) in &index.torrents {
            claimed.push(format!(".{hash}.parts").into());
            for file in &entry.files {
                if let Some(first) = Path::new(file).components().next() {
                    claimed.push(first.as_os_str().to_os_string());
                }
            }
        }
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        read.flatten().filter(|e| !claimed.contains(&e.file_name())).map(|e| e.path()).collect()
    }

    /// Deletes `paths` (files or directories) and prunes directories left
    /// empty up to `downloads/`. Missing paths are fine.
    fn delete_paths(&self, paths: &[PathBuf]) -> std::io::Result<()> {
        for path in paths {
            let result = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
            match result {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
            let mut parent = path.parent();
            while let Some(dir) = parent {
                if dir == self.dir || !dir.starts_with(&self.dir) || std::fs::remove_dir(dir).is_err() {
                    break;
                }
                parent = dir.parent();
            }
        }
        Ok(())
    }

    /// Deletes least-recently-used torrents until the cache fits its cap.
    /// Never `playing`. `include_orphans` (startup only - nothing is being
    /// written then) also considers unindexed items, by modification time.
    pub fn evict(&self, playing: Option<&str>, include_orphans: bool, reason: &str) {
        struct Item {
            key: Option<String>,
            name: String,
            bytes: u64,
            last_used: u64,
            paths: Vec<PathBuf>,
        }
        let playing = playing.map(str::to_ascii_lowercase);
        let mut index = self.lock();
        let limit = index.limit_bytes.unwrap_or(DEFAULT_LIMIT_BYTES);
        tracing::debug!(reason, limit, ?playing, include_orphans, "download cache eviction started");

        let mut total = 0u64;
        let mut items = Vec::new();
        let mut stale = Vec::new();
        for (hash, entry) in &index.torrents {
            let paths = self.entry_paths(hash, entry);
            let bytes: u64 = paths.iter().map(|p| disk_size(p)).sum();
            total += bytes;
            if playing.as_deref() == Some(hash.as_str()) {
                continue;
            }
            if bytes == 0 {
                stale.push(hash.clone());
                continue;
            }
            items.push(Item { key: Some(hash.clone()), name: entry.name.clone(), bytes, last_used: entry.last_used, paths });
        }
        if include_orphans {
            for path in self.orphans(&index) {
                let bytes = disk_size(&path);
                let last_used = std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                total += bytes;
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                items.push(Item { key: None, name, bytes, last_used, paths: vec![path] });
            }
        }
        let mut changed = !stale.is_empty();
        for hash in stale {
            tracing::debug!(%hash, "download cache entry has no files left, dropped");
            index.torrents.remove(&hash);
        }

        items.sort_by_key(|item| item.last_used);
        for item in items {
            if total <= limit {
                break;
            }
            match self.delete_paths(&item.paths) {
                Ok(()) => {
                    total -= item.bytes;
                    if let Some(key) = &item.key {
                        index.torrents.remove(key);
                        changed = true;
                    }
                    tracing::info!(name = %item.name, bytes = item.bytes, reason, orphan = item.key.is_none(), "download cache evicted");
                }
                Err(err) => tracing::warn!(name = %item.name, %err, "download cache eviction failed, retried next time"),
            }
        }
        if changed {
            self.save(&index);
        }
        tracing::info!(total, limit, reason, "download cache eviction done");
    }

    /// Deletes a scratch torrent's files (thumbnail capture) unless it is a
    /// cached release someone played.
    pub fn discard_unindexed(&self, info_hash: &str, files: &[TorrentFile]) {
        let info_hash = info_hash.to_ascii_lowercase();
        let index = self.lock();
        if index.torrents.contains_key(&info_hash) {
            tracing::debug!(%info_hash, "scratch torrent is cached, files kept");
            return;
        }
        let entry = Entry { name: String::new(), files: files.iter().map(|f| f.name.clone()).collect(), last_used: 0, episodes: Vec::new() };
        match self.delete_paths(&self.entry_paths(&info_hash, &entry)) {
            Ok(()) => tracing::debug!(%info_hash, "scratch torrent files deleted"),
            Err(err) => tracing::warn!(%info_hash, %err, "failed to delete scratch torrent files"),
        }
    }
}

/// Bytes a file or directory tree actually occupies - libtorrent's files
/// are sparse, so their length overstates a partial download.
fn disk_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        return std::fs::read_dir(path).map(|read| read.flatten().map(|e| disk_size(&e.path())).sum()).unwrap_or(0);
    }
    allocated_size(path).unwrap_or(meta.len())
}

#[cfg(windows)]
fn allocated_size(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetCompressedFileSizeW, INVALID_FILE_SIZE};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut high = 0u32;
    // SAFETY: `wide` is NUL-terminated and `high` outlives the call.
    let low = unsafe { GetCompressedFileSizeW(wide.as_ptr(), &mut high) };
    if low == INVALID_FILE_SIZE && std::io::Error::last_os_error().raw_os_error() != Some(0) {
        return None;
    }
    Some((u64::from(high) << 32) | u64::from(low))
}

#[cfg(not(windows))]
fn allocated_size(_path: &Path) -> Option<u64> {
    None
}
