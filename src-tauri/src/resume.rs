//! Continue-watching resume buffers (PLAN.md "Continue-watching resume
//! buffer"): per episode, the verified bytes needed to reopen its file and
//! play ~16 MB from the saved position, under
//! `<cache_dir>/nyaa-stream/resume/<info_hash>-<file_idx>/`.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use torrent_engine::resume_buffer::{self, intersect_ranges, merge_ranges};
use torrent_engine::{TorrentEngine, TorrentId};

/// Bytes kept from the keyframe at or before the saved position.
const PLAYBACK_BYTES: u64 = 16 * 1024 * 1024;
/// Hard caps across all buffers; oldest `updated_at` go first.
const MAX_BUFFERS: usize = 25;
const MAX_TOTAL_BYTES: u64 = 600 * 1024 * 1024;
const ENTRY_FILE: &str = "entry.json";

pub fn root() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream").join("resume")
}

/// What the player sends with `stop_playback` when the episode is still in
/// progress.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResumeRequest {
    pub anime_id: i64,
    pub episode_key: String,
    pub file_idx: usize,
    /// Seconds, in file time.
    pub position: f64,
    pub magnet: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntryMeta {
    pub anime_id: i64,
    pub episode_key: String,
    pub info_hash: String,
    pub file_idx: usize,
    pub file_name: String,
    pub position: f64,
    pub magnet: String,
    pub bytes: u64,
    pub updated_at: u64,
}

impl EntryMeta {
    pub fn episode(&self) -> String {
        format!("{}:{}", self.anime_id, self.episode_key)
    }
}

fn dir_name(info_hash: &str, file_idx: usize) -> String {
    format!("{}-{file_idx}", info_hash.to_ascii_lowercase())
}

/// Every saved buffer with its directory. Unreadable ones are removed.
pub fn list() -> Vec<(PathBuf, EntryMeta)> {
    let Ok(read) = std::fs::read_dir(root()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in read.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        match std::fs::read(dir.join(ENTRY_FILE)).ok().and_then(|b| serde_json::from_slice::<EntryMeta>(&b).ok()) {
            Some(meta) => out.push((dir, meta)),
            None => {
                tracing::debug!(dir = %dir.display(), "removing unreadable resume buffer");
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
    out
}

fn remove(dir: &PathBuf, meta: &EntryMeta, reason: &str) {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => tracing::info!(episode = %meta.episode(), bytes = meta.bytes, reason, "resume buffer removed"),
        Err(err) => tracing::warn!(dir = %dir.display(), %err, "failed to remove resume buffer"),
    }
}

/// Saves the resume buffer for `request` from the torrent `torrent_id`
/// (still in the engine). Best effort: bytes not verified locally are
/// skipped.
pub async fn save(engine: &TorrentEngine, torrent_id: &TorrentId, request: ResumeRequest) -> anyhow::Result<()> {
    tracing::debug!(torrent_id = %torrent_id, ?request, "saving resume buffer");
    let file = engine.verified_file(torrent_id, request.file_idx).await?;
    let mut wanted = engine.open_read_ranges(torrent_id, request.file_idx);
    match TorrentEngine::keyframe_byte_offset(file.path.clone(), request.position).await {
        Ok(Some(offset)) => wanted.push((offset, (offset + PLAYBACK_BYTES).min(file.size))),
        Ok(None) => tracing::warn!(position = request.position, "no keyframe offset, buffer holds open reads only"),
        Err(err) => tracing::warn!(position = request.position, %err, "keyframe lookup failed, buffer holds open reads only"),
    }
    let ranges = intersect_ranges(&merge_ranges(wanted), &file.verified);
    if ranges.is_empty() {
        tracing::info!(torrent_id = %torrent_id, "nothing verified to buffer, resume buffer skipped");
        return Ok(());
    }

    let info_hash = torrent_id.to_ascii_lowercase();
    let root = root();
    let dir = root.join(dir_name(&info_hash, request.file_idx));
    let tmp = root.join(format!("{}.tmp", dir_name(&info_hash, request.file_idx)));
    let meta = EntryMeta {
        anime_id: request.anime_id,
        episode_key: request.episode_key,
        info_hash,
        file_idx: request.file_idx,
        file_name: file.name.clone(),
        position: request.position,
        magnet: request.magnet,
        bytes: 0,
        updated_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    };
    let meta = tokio::task::spawn_blocking(move || -> anyhow::Result<EntryMeta> {
        let _ = std::fs::remove_dir_all(&tmp);
        let bytes = resume_buffer::write_buffer(&tmp, &file.path, file.size, &ranges)?;
        let meta = EntryMeta { bytes, ..meta };
        std::fs::write(tmp.join(ENTRY_FILE), serde_json::to_vec_pretty(&meta)?)?;
        // One buffer per episode: an older one (other source) goes.
        for (other, other_meta) in list() {
            if other != dir && other_meta.episode() == meta.episode() {
                remove(&other, &other_meta, "replaced");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&tmp, &dir)?;
        enforce_caps();
        Ok(meta)
    })
    .await??;
    tracing::info!(episode = %meta.episode(), bytes = meta.bytes, position = meta.position, file = %meta.file_name, "resume buffer saved");
    Ok(())
}

/// Oldest buffers go until at most `MAX_BUFFERS` / `MAX_TOTAL_BYTES` remain.
fn enforce_caps() {
    let mut buffers = list();
    buffers.sort_by_key(|(_, meta)| std::cmp::Reverse(meta.updated_at));
    let mut total = 0;
    for (count, (dir, meta)) in buffers.iter().enumerate() {
        total += meta.bytes;
        if count >= MAX_BUFFERS || total > MAX_TOTAL_BYTES {
            remove(dir, meta, "cap");
        }
    }
}
