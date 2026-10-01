//! Continue-watching resume buffer support (PLAN.md "Continue-watching
//! resume buffer"): which byte ranges mpv reads to *open* a file, recorded
//! by `stream_handler` until the player reports `file-loaded`, and the
//! on-disk buffer format (`data.bin` + `ranges.json`) - verified bytes of
//! one torrent file at their real offsets.

use std::collections::HashMap;
use std::future::Future;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
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

/// `[start, end)` runs covered by both `a` and `b` (each sorted, merged).
pub fn intersect_ranges(a: &[(u64, u64)], b: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let start = a[i].0.max(b[j].0);
        let end = a[i].1.min(b[j].1);
        if start < end {
            out.push((start, end));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

const DATA_FILE: &str = "data.bin";
const RANGES_FILE: &str = "ranges.json";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RangesFile {
    file_size: u64,
    /// `[start, end)` file offsets, stored back to back in `data.bin`.
    ranges: Vec<(u64, u64)>,
}

/// Copies `ranges` (verified runs) of the file at `source` into a buffer
/// in `dir`. Returns the bytes written. Blocking.
pub fn write_buffer(dir: &Path, source: &Path, file_size: u64, ranges: &[(u64, u64)]) -> std::io::Result<u64> {
    std::fs::create_dir_all(dir)?;
    let mut input = std::fs::File::open(source)?;
    let mut output = std::io::BufWriter::new(std::fs::File::create(dir.join(DATA_FILE))?);
    let mut written = 0;
    for &(start, end) in ranges {
        input.seek(SeekFrom::Start(start))?;
        let copied = std::io::copy(&mut (&mut input).take(end - start), &mut output)?;
        if copied != end - start {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("source ended inside {start}..{end}")));
        }
        written += copied;
    }
    output.flush()?;
    let index = RangesFile { file_size, ranges: ranges.to_vec() };
    std::fs::write(dir.join(RANGES_FILE), serde_json::to_vec(&index).map_err(std::io::Error::other)?)?;
    Ok(written)
}

/// A buffer loaded for serving: its runs, in memory.
pub struct LoadedBuffer {
    pub file_size: u64,
    /// Sorted, non-overlapping `(start, bytes)`.
    runs: Vec<(u64, Vec<u8>)>,
}

impl LoadedBuffer {
    /// Reads the buffer in `dir`. Blocking.
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let index: RangesFile = serde_json::from_slice(&std::fs::read(dir.join(RANGES_FILE))?).map_err(std::io::Error::other)?;
        let data = std::fs::read(dir.join(DATA_FILE))?;
        let mut runs = Vec::with_capacity(index.ranges.len());
        let mut offset = 0usize;
        for (start, end) in index.ranges {
            let len = usize::try_from(end - start).map_err(std::io::Error::other)?;
            let bytes = data.get(offset..offset + len).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "data.bin shorter than its ranges"))?;
            runs.push((start, bytes.to_vec()));
            offset += len;
        }
        Ok(Self { file_size: index.file_size, runs })
    }

    pub fn bytes(&self) -> u64 {
        self.runs.iter().map(|(_, b)| b.len() as u64).sum()
    }

    /// Start of the first run after `position` (where a torrent read that
    /// began in a gap should stop), if any.
    fn next_run_after(&self, position: u64) -> Option<u64> {
        self.runs.iter().map(|(start, _)| *start).find(|start| *start > position)
    }

    /// The buffered bytes starting exactly at `position`, if covered.
    pub fn slice_at(&self, position: u64) -> Option<&[u8]> {
        let idx = self.runs.partition_point(|(start, _)| *start <= position).checked_sub(1)?;
        let (start, bytes) = &self.runs[idx];
        let offset = usize::try_from(position - start).ok()?;
        (offset < bytes.len()).then(|| &bytes[offset..])
    }
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

/// Resume buffers attached to torrents being played (see
/// `TorrentEngine::attach_resume_buffer`).
#[derive(Clone, Default)]
pub struct ResumeBuffers {
    buffers: Arc<Mutex<HashMap<Key, Arc<LoadedBuffer>>>>,
}

impl ResumeBuffers {
    pub fn attach(&self, torrent_id: &TorrentId, file_idx: usize, buffer: Arc<LoadedBuffer>) {
        tracing::info!(torrent_id = %torrent_id, file_idx, bytes = buffer.bytes(), "resume buffer attached");
        self.buffers.lock().unwrap_or_else(|e| e.into_inner()).insert((torrent_id.to_ascii_lowercase(), file_idx), buffer);
    }

    pub fn get(&self, torrent_id: &TorrentId, file_idx: usize) -> Option<Arc<LoadedBuffer>> {
        self.buffers.lock().unwrap_or_else(|e| e.into_inner()).get(&(torrent_id.to_ascii_lowercase(), file_idx)).cloned()
    }

    pub fn remove_torrent(&self, torrent_id: &TorrentId) {
        let id = torrent_id.to_ascii_lowercase();
        self.buffers.lock().unwrap_or_else(|e| e.into_inner()).retain(|(t, _), _| *t != id);
    }
}

pub(crate) type OpenFuture<F> = Pin<Box<dyn Future<Output = Option<F>> + Send>>;

enum Inner<F> {
    /// Not polled until a read leaves the buffer - an async block does
    /// nothing until then, so a resume served from the buffer never waits
    /// on the torrent (re-hash, reconnect).
    Opening(OpenFuture<F>),
    Ready { reader: F, position: Option<u64>, seeking: bool },
    Failed,
}

/// Serves a file from its resume buffer where it covers the read position,
/// and from the torrent (`open`ed lazily) where it doesn't. Only verified
/// bytes are ever buffered, so both sources agree byte for byte.
pub(crate) struct BufferedReader<F> {
    buffer: Arc<LoadedBuffer>,
    inner: Inner<F>,
    position: u64,
}

impl<F> BufferedReader<F> {
    pub(crate) fn new(buffer: Arc<LoadedBuffer>, open: OpenFuture<F>) -> Self {
        Self { buffer, inner: Inner::Opening(open), position: 0 }
    }
}

impl<F: AsyncRead + AsyncSeek + Unpin> AsyncRead for BufferedReader<F> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = &mut *self;
        if this.position >= this.buffer.file_size {
            return Poll::Ready(Ok(()));
        }
        if let Some(slice) = this.buffer.slice_at(this.position) {
            let n = slice.len().min(buf.remaining());
            buf.put_slice(&slice[..n]);
            this.position += n as u64;
            return Poll::Ready(Ok(()));
        }
        loop {
            match &mut this.inner {
                Inner::Opening(open) => match open.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Some(reader)) => {
                        tracing::debug!(position = this.position, "resume buffer ran out, opening the torrent stream");
                        this.inner = Inner::Ready { reader, position: None, seeking: false };
                    }
                    Poll::Ready(None) => {
                        tracing::warn!("torrent stream unavailable behind the resume buffer");
                        this.inner = Inner::Failed;
                    }
                },
                Inner::Failed => return Poll::Ready(Err(std::io::Error::other("torrent stream unavailable"))),
                Inner::Ready { reader, position, seeking } => {
                    if *position != Some(this.position) {
                        if !*seeking {
                            Pin::new(&mut *reader).start_seek(SeekFrom::Start(this.position))?;
                            *seeking = true;
                        }
                        match Pin::new(&mut *reader).poll_complete(cx) {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(result) => {
                                *seeking = false;
                                *position = Some(result?);
                                continue;
                            }
                        }
                    }
                    // Stop at the next buffered run - no need to wait on the
                    // torrent for bytes already here.
                    let limit = this.buffer.next_run_after(this.position).map_or(u64::MAX, |next| next - this.position);
                    let limit = usize::try_from(limit).unwrap_or(usize::MAX).min(buf.remaining());
                    let mut sub = buf.take(limit);
                    match Pin::new(&mut *reader).poll_read(cx, &mut sub) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                        Poll::Ready(Ok(())) => {
                            let n = sub.filled().len();
                            // SAFETY: `sub` filled these `n` bytes of the
                            // unfilled region of `buf`.
                            unsafe { buf.assume_init(n) };
                            buf.advance(n);
                            this.position += n as u64;
                            *position = Some(this.position);
                            return Poll::Ready(Ok(()));
                        }
                    }
                }
            }
        }
    }
}

impl<F: Unpin> AsyncSeek for BufferedReader<F> {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        let size = self.buffer.file_size;
        let target = match position {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(delta) => size.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
        };
        self.position = target.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.position))
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
    fn intersects() {
        assert_eq!(intersect_ranges(&[(0, 10), (20, 30)], &[(5, 25)]), vec![(5, 10), (20, 25)]);
        assert!(intersect_ranges(&[(0, 10)], &[(10, 20)]).is_empty());
    }

    #[test]
    fn buffer_round_trip() {
        let dir = std::env::temp_dir().join(format!("nyaa-rb-{}", std::process::id()));
        let source = dir.join("source.bin");
        std::fs::create_dir_all(&dir).unwrap();
        let content: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        std::fs::write(&source, &content).unwrap();
        let buf_dir = dir.join("buf");
        assert_eq!(write_buffer(&buf_dir, &source, 1000, &[(0, 10), (500, 600)]).unwrap(), 110);
        let loaded = LoadedBuffer::load(&buf_dir).unwrap();
        assert_eq!(loaded.bytes(), 110);
        assert_eq!(loaded.slice_at(3).unwrap(), &content[3..10]);
        assert_eq!(loaded.slice_at(550).unwrap(), &content[550..600]);
        assert!(loaded.slice_at(10).is_none());
        assert!(loaded.slice_at(499).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn buffered_reader_serves_buffer_then_torrent() {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let dir = std::env::temp_dir().join(format!("nyaa-rb-read-{}", std::process::id()));
        let source = dir.join("source.bin");
        std::fs::create_dir_all(&dir).unwrap();
        let content: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        std::fs::write(&source, &content).unwrap();
        write_buffer(&dir.join("buf"), &source, 1000, &[(0, 100), (500, 600)]).unwrap();
        let buffer = Arc::new(LoadedBuffer::load(&dir.join("buf")).unwrap());

        // Fully buffered read: the torrent is never opened.
        let never: OpenFuture<std::io::Cursor<Vec<u8>>> = Box::pin(async { panic!("torrent opened") });
        let mut reader = BufferedReader::new(buffer.clone(), never);
        let mut out = vec![0; 50];
        reader.seek(SeekFrom::Start(520)).await.unwrap();
        reader.read_exact(&mut out).await.unwrap();
        assert_eq!(out, content[520..570]);

        // Across buffer and gaps: identical to the source.
        let torrent = std::io::Cursor::new(content.clone());
        let mut reader = BufferedReader::new(buffer, Box::pin(async move { Some(torrent) }));
        reader.seek(SeekFrom::Start(50)).await.unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await.unwrap();
        assert_eq!(out, content[50..]);
        let _ = std::fs::remove_dir_all(&dir);
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
