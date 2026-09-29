//! Download cache: played torrents' files stay in `downloads/` so a re-open
//! reuses every verified piece (libtorrent re-hashes them on add), tracked in
//! `downloads/.cache-index.json` - see PLAN.md "Download cache".

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use torrent_engine::TorrentFile;

const INDEX_FILE: &str = ".cache-index.json";

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Index {
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
        let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
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
}
