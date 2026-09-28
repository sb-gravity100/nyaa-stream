//! Append-only event logs for extracted subtitle tracks, so the frontend can
//! fetch only what's new instead of the whole script on every poll.
//!
//! Why: a heavily typeset release (Kaleido-subs' Fate/strange Fake: a 40 MB
//! script, ~77k events of frame-by-frame vector signs) never rendered - the
//! renderer re-downloaded and re-parsed the entire growing script every
//! poll and was still parsing when the next one arrived. Each run file
//! (`sub_<stream>_<start|bg>.ass`) is read incrementally from where the last
//! poll stopped; unseen `Dialogue:` lines are appended in arrival order, so
//! an event's position in the log never changes and `from=N` is a stable
//! cursor.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Mutex as AsyncMutex;

use crate::TorrentId;

#[derive(Default)]
struct TrackLog {
    /// Script header through the `[Events]` section's `Format:` line.
    header: Option<String>,
    events: Vec<String>,
    seen: HashSet<String>,
    /// Bytes of each run file already consumed (complete lines only).
    offsets: HashMap<PathBuf, u64>,
}

impl TrackLog {
    fn ingest(&mut self, text: &str) {
        if self.header.is_none() {
            if let Some(events_pos) = text.find("[Events]") {
                let after = &text[events_pos..];
                if let Some(format_pos) = after.find("Format:") {
                    if let Some(eol) = after[format_pos..].find('\n') {
                        self.header = Some(text[..events_pos + format_pos + eol + 1].to_string());
                    }
                }
            }
        }
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with("Dialogue:") && self.seen.insert(line.to_string()) {
                self.events.push(line.to_string());
            }
        }
    }
}

/// What a poll gets back.
pub(crate) struct TrackSlice {
    /// Header + events when `from` was 0, otherwise just the new event lines.
    pub body: String,
    /// Events in the log - the next poll's `from`.
    pub total: usize,
}

#[derive(Clone, Default)]
pub(crate) struct SubtitleLogs {
    tracks: Arc<AsyncMutex<HashMap<(TorrentId, usize, usize), Arc<AsyncMutex<TrackLog>>>>>,
}

impl SubtitleLogs {
    /// Reads whatever the run files in `dir` gained since the last call and
    /// returns events from `from` on. None until some run has written its
    /// header.
    pub(crate) async fn poll(&self, dir: &Path, torrent_id: &TorrentId, file_idx: usize, stream_index: usize, from: usize) -> Option<TrackSlice> {
        let log = {
            let mut tracks = self.tracks.lock().await;
            tracks.entry((torrent_id.clone(), file_idx, stream_index)).or_default().clone()
        };
        let mut log = log.lock().await;
        let prefix = format!("sub_{stream_index}_");
        let mut runs = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                // `<start>` for a transcode run, `bg` for the full-file pass
                // (first: it starts at byte 0).
                let order = name.strip_prefix(&prefix).and_then(|rest| rest.strip_suffix(".ass")).and_then(|n| if n == "bg" { Some(0) } else { n.parse::<usize>().ok() });
                if let Some(order) = order {
                    runs.push((order, entry.path()));
                }
            }
        }
        runs.sort();
        for (_, path) in runs {
            let offset = log.offsets.get(&path).copied().unwrap_or(0);
            match read_complete_lines(&path, offset).await {
                Ok(Some((text, start, consumed))) => {
                    log.ingest(&text);
                    log.offsets.insert(path, start + consumed);
                }
                Ok(None) => {}
                Err(err) => tracing::warn!(torrent_id = %torrent_id, file_idx, stream_index, ?path, %err, "failed to read subtitle run file"),
            }
        }
        let header = log.header.as_ref()?;
        let total = log.events.len();
        let from = from.min(total);
        let mut body = if from == 0 { header.clone() } else { String::new() };
        for event in &log.events[from..] {
            body.push_str(event);
            body.push('\n');
        }
        Some(TrackSlice { body, total })
    }

    pub(crate) async fn remove_torrent(&self, torrent_id: &TorrentId) {
        self.tracks.lock().await.retain(|(id, _, _), _| id != torrent_id);
    }
}

/// New newline-terminated text in `path` after `offset` (a line still being
/// written is left for the next call), the offset actually read from, and
/// how many bytes that covered.
async fn read_complete_lines(path: &Path, offset: u64) -> std::io::Result<Option<(String, u64, u64)>> {
    let mut file = tokio::fs::File::open(path).await?;
    let len = file.metadata().await?.len();
    // Shorter than what was consumed: a run restarted at the same offset
    // rewrote the file - read it again (already-seen events are skipped).
    let offset = if len < offset { 0 } else { offset };
    if len == offset {
        return Ok(None);
    }
    file.seek(std::io::SeekFrom::Start(offset)).await?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).await?;
    let Some(end) = bytes.iter().rposition(|&b| b == b'\n') else {
        return Ok(None);
    };
    bytes.truncate(end + 1);
    Ok(Some((String::from_utf8_lossy(&bytes).into_owned(), offset, (end + 1) as u64)))
}

#[cfg(test)]
mod tests {
    use super::SubtitleLogs;

    const HEADER: &str = "[Script Info]
ScriptType: v4.00+

[V4+ Styles]
Style: Default,Arial,16

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
";

    fn line(t: &str) -> String {
        format!("Dialogue: 0,0:00:{t}.00,0:00:{t}.50,Default,,0,0,0,,{t}
")
    }

    #[tokio::test]
    async fn appends_only_new_events_across_runs_and_partial_lines() {
        let dir = std::env::temp_dir().join(format!("nyaa_sublog_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let logs = SubtitleLogs::default();
        let id = "t".to_string();
        assert!(logs.poll(&dir, &id, 0, 2, 0).await.is_none());

        // Run 0 with a half-written last line.
        std::fs::write(dir.join("sub_2_0.ass"), format!("{HEADER}{}{}Dialogue: 0,0:00:1", line("01"), line("02"))).unwrap();
        let first = logs.poll(&dir, &id, 0, 2, 0).await.unwrap();
        assert!(first.body.starts_with("[Script Info]"));
        assert_eq!(first.total, 2);

        // Run 0 finishes the line; a seek-restart run overlaps it.
        std::fs::write(dir.join("sub_2_0.ass"), format!("{HEADER}{}{}{}", line("01"), line("02"), line("10"))).unwrap();
        std::fs::write(dir.join("sub_2_5.ass"), format!("{HEADER}{}{}", line("10"), line("30"))).unwrap();
        let next = logs.poll(&dir, &id, 0, 2, first.total).await.unwrap();
        assert!(!next.body.contains("[Script Info]"));
        assert_eq!(next.body.matches("Dialogue:").count(), 2);
        assert_eq!(next.total, 4);

        // Nothing new: empty delta, same total.
        let idle = logs.poll(&dir, &id, 0, 2, next.total).await.unwrap();
        assert!(idle.body.is_empty());
        assert_eq!(idle.total, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
