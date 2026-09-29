//! Continue-watching resume buffer support (PLAN.md "Continue-watching
//! resume buffer"): which byte ranges mpv reads to *open* a file, recorded
//! by `stream_handler` until the player reports `file-loaded`.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

use crate::TorrentId;

/// Stop recording a file's open reads past this much (a player that never
/// reports `file-loaded` would otherwise record the whole episode).
const MAX_OPEN_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Default)]
struct OpenLog {
    /// Cleared by `OpenReads::finish` (the player's `file-loaded`).
    recording: bool,
    /// Merged `[start, end)` file-relative ranges.
    ranges: Vec<(u64, u64)>,
}

impl OpenLog {
    fn total(&self) -> u64 {
        self.ranges.iter().map(|(s, e)| e - s).sum()
    }

    fn add(&mut self, start: u64, end: u64) {
        if !self.recording || start >= end {
            return;
        }
        self.ranges.push((start, end));
        self.ranges = merge_ranges(std::mem::take(&mut self.ranges));
        if self.total() >= MAX_OPEN_BYTES {
            tracing::warn!(bytes = self.total(), "open-read recording hit its cap, stopped");
            self.recording = false;
        }
    }
}

/// Sorts and merges overlapping or touching `[start, end)` ranges.
pub fn merge_ranges(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

type Key = (TorrentId, usize);

/// Per (torrent, file): the ranges read while the player opened it.
#[derive(Clone, Default)]
pub struct OpenReads {
    logs: Arc<Mutex<HashMap<Key, Arc<Mutex<OpenLog>>>>>,
}

impl OpenReads {
    /// The log a foreground stream of the file records into - starting a
    /// recording on the file's first stream.
    pub(crate) fn log_for(&self, torrent_id: &TorrentId, file_idx: usize) -> OpenLogHandle {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        let log = logs
            .entry((torrent_id.to_ascii_lowercase(), file_idx))
            .or_insert_with(|| {
                tracing::debug!(torrent_id = %torrent_id, file_idx, "recording open reads");
                Arc::new(Mutex::new(OpenLog { recording: true, ranges: Vec::new() }))
            })
            .clone();
        OpenLogHandle(log)
    }

    /// The player finished opening the file: later reads are playback.
    pub fn finish(&self, torrent_id: &TorrentId, file_idx: usize) {
        let logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(log) = logs.get(&(torrent_id.to_ascii_lowercase(), file_idx)) {
            let mut log = log.lock().unwrap_or_else(|e| e.into_inner());
            if log.recording {
                log.recording = false;
                tracing::info!(torrent_id = %torrent_id, file_idx, ranges = log.ranges.len(), bytes = log.total(), "open reads recorded");
            }
        }
    }

    pub fn ranges(&self, torrent_id: &TorrentId, file_idx: usize) -> Vec<(u64, u64)> {
        let logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.get(&(torrent_id.to_ascii_lowercase(), file_idx))
            .map(|log| log.lock().unwrap_or_else(|e| e.into_inner()).ranges.clone())
            .unwrap_or_default()
    }

    pub fn remove_torrent(&self, torrent_id: &TorrentId) {
        let id = torrent_id.to_ascii_lowercase();
        self.logs.lock().unwrap_or_else(|e| e.into_inner()).retain(|(t, _), _| *t != id);
    }
}

#[derive(Clone)]
pub(crate) struct OpenLogHandle(Arc<Mutex<OpenLog>>);

impl OpenLogHandle {
    fn add(&self, start: u64, end: u64) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).add(start, end);
    }
}

/// Passes reads through, recording each served range into an `OpenLog`
/// (none for background readers).
pub(crate) struct RecordingReader<R> {
    inner: R,
    position: u64,
    log: Option<OpenLogHandle>,
}

impl<R> RecordingReader<R> {
    pub(crate) fn new(inner: R, log: Option<OpenLogHandle>) -> Self {
        Self { inner, position: 0, log }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for RecordingReader<R> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = result {
            let n = (buf.filled().len() - before) as u64;
            let start = self.position;
            self.position += n;
            if let Some(log) = &self.log {
                log.add(start, start + n);
            }
        }
        result
    }
}

impl<R: AsyncSeek + Unpin> AsyncSeek for RecordingReader<R> {
    fn start_seek(mut self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        Pin::new(&mut self.inner).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        let result = Pin::new(&mut self.inner).poll_complete(cx);
        if let Poll::Ready(Ok(position)) = result {
            self.position = position;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_overlapping_and_touching() {
        assert_eq!(merge_ranges(vec![(10, 20), (0, 5), (5, 8), (15, 30), (40, 41)]), vec![(0, 8), (10, 30), (40, 41)]);
    }

    #[test]
    fn stops_recording_after_finish() {
        let reads = OpenReads::default();
        let id: TorrentId = "ABC".into();
        reads.log_for(&id, 0).add(0, 100);
        reads.finish(&id, 0);
        reads.log_for(&id, 0).add(500, 600);
        assert_eq!(reads.ranges(&"abc".into(), 0), vec![(0, 100)]);
    }
}
