//! One torrent in the engine: its files, stats, readers and the streams
//! that decide what else downloads.
//!
//! What downloads, by stream:
//! - foreground readers: sequential, tail prefetch, then the rest of their
//!   file in order (idle SEQ_AHEAD);
//! - background readers: sequential, nothing beyond their window;
//! - `set_preload_file`: a reader-less stream that fetches the file in
//!   order behind everything else;
//! - each torrent keeps one idle stream (idle NONE) open, so files nobody
//!   asked for (other episodes in a batch) are not downloaded.

use std::collections::HashMap;
use std::io::Seek;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

/// Readers report no data after this long (`direct_input` reopens on error).
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// sbtl's ready threshold: playback time verified ahead before starting.
pub const READY_MS: u64 = 3000;

/// What a reader is for, which decides what sbtl downloads around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadKind {
    /// The player: read-ahead window, file tail early, then the rest of the
    /// file in order. Its buffer is what `Torrent::buffer` reports.
    Foreground,
    /// Never competes with playback (subtitle passes, resume-buffer reads,
    /// probes): only its read-ahead window.
    Background,
}

/// A torrent's state; `Checking` = re-checking data already on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TorrentState {
    Checking,
    Metadata,
    Downloading,
    Finished,
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    /// Torrent-relative path with native separators (`Show\Episode 01.mkv`).
    pub name: String,
    pub length: u64,
}

#[derive(Clone, Debug)]
pub struct FileStats {
    /// File name without directories.
    pub name: String,
    /// Torrent-relative path, as in `FileInfo::name`.
    pub path: String,
    pub length: u64,
    /// Bytes in `ranges`.
    pub downloaded: u64,
    /// Verified `[start, end)` runs already written to disk: safe for
    /// anything reading the file directly (resume buffers, probes).
    pub ranges: Vec<(u64, u64)>,
}

#[derive(Clone, Debug)]
pub struct TorrentStats {
    pub name: String,
    pub state: TorrentState,
    pub has_metadata: bool,
    pub finished: bool,
    /// Bytes/s.
    pub download_rate: f64,
    pub upload_rate: f64,
    pub downloaded: u64,
    pub uploaded: u64,
    pub peers: u64,
    /// Peers known from trackers, DHT and peer exchange.
    pub peers_known: u64,
    pub files: Vec<FileStats>,
}

/// A foreground reader's buffer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferStatus {
    /// "stalled" (a read blocks) | "low" (< ready) | "ready" | "full"
    pub level: String,
    /// Read position in the file.
    pub pos: u64,
    pub ahead_bytes: u64,
    /// `ahead_bytes` in playback time at `rate`.
    pub ahead_ms: u64,
    /// Playback time ahead that counts as ready to start / resume.
    pub ready_ms: u64,
    /// Estimated time until ready at the current download rate.
    pub eta_ready_ms: Option<u64>,
    /// Consumption rate used, bytes/s; `rate_known` false = assumed.
    pub rate: u64,
    pub rate_known: bool,
    /// Current torrent download rate used to fill this reader's buffer.
    pub fill_rate: u64,
    /// Continuous duration with no verified data at the read position.
    pub stalled_ms: u64,
}

fn make_buffer_status(buffer: sbtl::BufferInfo) -> BufferStatus {
    BufferStatus {
        level: match buffer.level {
            sbtl::BufferLevel::Stalled => "stalled",
            sbtl::BufferLevel::Low => "low",
            sbtl::BufferLevel::Ready => "ready",
            sbtl::BufferLevel::Full => "full",
        }
        .to_string(),
        pos: buffer.pos,
        ahead_bytes: buffer.ahead_bytes,
        ahead_ms: buffer.ahead.as_millis() as u64,
        ready_ms: READY_MS,
        eta_ready_ms: buffer.eta_ready.map(|duration| duration.as_millis() as u64),
        rate: buffer.rate as u64,
        rate_known: buffer.rate_known,
        fill_rate: buffer.fill_rate as u64,
        stalled_ms: buffer.stalled_for.as_millis() as u64,
    }
}

struct Streams {
    /// Held only to stay open: keeps files nobody reads from downloading
    /// (see the module docs).
    _idle: sbtl::Stream,
    preload: Option<(usize, sbtl::Stream)>,
    /// Newest foreground reader per file (weak: doesn't keep it open).
    probes: HashMap<usize, ReaderProbe>,
}

#[derive(Clone)]
enum ReaderProbe {
    Async(sbtl::BufferProbe),
    Direct(std::sync::Weak<sbtl::Stream>),
}

impl ReaderProbe {
    fn buffer(&self) -> Option<sbtl::BufferInfo> {
        match self {
            Self::Async(probe) => probe.buffer(),
            Self::Direct(stream) => stream.upgrade().map(|stream| stream.buffer()),
        }
    }
}

pub struct Torrent {
    torrent: sbtl::Torrent,
    info_hash: String,
    save_dir: PathBuf,
    streams: Mutex<Streams>,
    /// Open `Reader`s; the engine never removes a torrent that has one.
    readers: Arc<AtomicUsize>,
    last_access: Mutex<Instant>,
}

impl Torrent {
    pub(crate) fn new(torrent: sbtl::Torrent, info_hash: String, save_dir: PathBuf) -> Result<Self> {
        let idle = torrent
            .open_stream(0, &sbtl::StreamOptions { sequential: false, idle: sbtl::Idle::None, tail_prefetch: false, ..Default::default() })
            .map_err(|e| anyhow!("sbtl idle stream: {e}"))?;
        Ok(Self {
            torrent,
            info_hash,
            save_dir,
            streams: Mutex::new(Streams { _idle: idle, preload: None, probes: HashMap::new() }),
            readers: Arc::new(AtomicUsize::new(0)),
            last_access: Mutex::new(Instant::now()),
        })
    }

    pub(crate) fn add_trackers(&self, trackers: &[String]) {
        for t in trackers {
            if let Err(e) = self.torrent.add_tracker(t) {
                tracing::warn!(info_hash = %self.info_hash, tracker = %t, %e, "sbtl: tracker rejected");
            }
        }
    }

    pub(crate) fn touch(&self) {
        *self.last_access.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    pub(crate) fn last_access(&self) -> Instant {
        *self.last_access.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn open_readers(&self) -> usize {
        self.readers.load(Ordering::SeqCst)
    }

    pub fn info_hash(&self) -> &str {
        &self.info_hash
    }

    pub fn name(&self) -> Option<String> {
        let n = self.torrent.status().name;
        (!n.is_empty()).then_some(n)
    }

    fn sbtl_files(&self) -> Vec<sbtl::FileInfo> {
        self.torrent.files().unwrap_or_default()
    }

    /// Torrent-relative path with native separators.
    fn relative(&self, f: &sbtl::FileInfo) -> String {
        Path::new(&f.path)
            .strip_prefix(&self.save_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| f.path.clone())
            .replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)
    }

    /// The torrent's files; empty until a magnet's metadata arrived.
    pub fn files(&self) -> Vec<FileInfo> {
        self.sbtl_files().iter().map(|f| FileInfo { name: self.relative(f), length: f.size }).collect()
    }

    pub fn stats(&self) -> TorrentStats {
        self.touch();
        let st = self.torrent.status();
        let files = self
            .sbtl_files()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let ranges = self.torrent.file_ranges(i as u32).unwrap_or_default();
                FileStats {
                    name: Path::new(&f.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    path: self.relative(f),
                    length: f.size,
                    downloaded: ranges.iter().map(|(a, b)| b - a).sum(),
                    ranges,
                }
            })
            .collect();
        let state = if st.pieces_checking > 0 {
            TorrentState::Checking
        } else if !st.has_metadata {
            TorrentState::Metadata
        } else if st.complete {
            TorrentState::Finished
        } else {
            TorrentState::Downloading
        };
        TorrentStats {
            name: st.name,
            state,
            has_metadata: st.has_metadata,
            finished: st.complete,
            download_rate: st.download_rate as f64,
            upload_rate: st.upload_rate as f64,
            downloaded: st.downloaded_bytes,
            uploaded: st.uploaded_bytes,
            peers: st.peers_connected as u64,
            peers_known: st.peers_known as u64,
            files,
        }
    }

    /// Opens a reader on `file_idx` at `offset`.
    pub fn open(&self, file_idx: usize, offset: u64, kind: ReadKind) -> Result<Reader> {
        self.touch();
        let files = self.sbtl_files();
        let file = files.get(file_idx).ok_or_else(|| anyhow!("no file {file_idx} in {}", self.info_hash))?;
        let foreground = kind == ReadKind::Foreground;
        let opts = sbtl::StreamOptions {
            sequential: true,
            idle: if foreground { sbtl::Idle::SeqAhead } else { sbtl::Idle::None },
            tail_prefetch: foreground,
            media_probe: false, // sbtl's own container index: nyaa probes media itself (media.rs)
            bitrate: None,
            read_timeout: Some(READ_TIMEOUT),
        };
        let mut stream = self.torrent.open_stream(file_idx as u32, &opts).map_err(|e| anyhow!("sbtl reader for file {file_idx}: {e}"))?;
        stream.seek(std::io::SeekFrom::Start(offset))?;
        stream.set_buffer_targets(Some(Duration::from_millis(READY_MS)), None);
        let stream = sbtl::AsyncStream::from(stream);
        if foreground {
            self.streams.lock().unwrap_or_else(|e| e.into_inner()).probes.insert(file_idx, ReaderProbe::Async(stream.buffer_probe()));
        }
        self.readers.fetch_add(1, Ordering::SeqCst);
        tracing::info!(info_hash = %self.info_hash, file_idx, offset, ?kind, "sbtl: reader opened");
        Ok(Reader {
            size: file.size,
            name: self.relative(file),
            stream,
            _open: OpenReader(self.readers.clone()),
        })
    }

    /// Opens a blocking, thread-safe reader for libmpv's stream callback API.
    /// This owns the sbtl stream until mpv closes its callback cookie, so the
    /// torrent lifecycle cannot remove it while demux is reading.
    pub fn open_mpv_stream(&self, file_idx: usize) -> Result<MpvStream> {
        self.touch();
        let files = self.sbtl_files();
        let file = files.get(file_idx).ok_or_else(|| anyhow!("no file {file_idx} in {}", self.info_hash))?;
        let opts = sbtl::StreamOptions {
            sequential: true,
            idle: sbtl::Idle::SeqAhead,
            tail_prefetch: true,
            media_probe: true,
            bitrate: None,
            read_timeout: None,
        };
        let stream = self.torrent.open_stream(file_idx as u32, &opts).map_err(|e| anyhow!("sbtl mpv stream for file {file_idx}: {e}"))?;
        stream.set_buffer_targets(Some(Duration::from_secs(3)), None);
        let stream = Arc::new(stream);
        self.streams.lock().unwrap_or_else(|e| e.into_inner()).probes.insert(file_idx, ReaderProbe::Direct(Arc::downgrade(&stream)));
        self.readers.fetch_add(1, Ordering::SeqCst);
        tracing::info!(info_hash = %self.info_hash, file_idx, length = file.size, "sbtl: direct mpv stream opened");
        Ok(MpvStream {
            stream,
            position: Mutex::new(0),
            cancelled: AtomicBool::new(false),
            adaptive_window: Mutex::new(AdaptiveWindow { playback_speed: 1.0, configured_seconds: None }),
            len: file.size,
            _open: OpenReader(self.readers.clone()),
        })
    }

    /// The newest foreground reader's buffer on `file_idx`, while it's open.
    pub fn buffer(&self, file_idx: usize) -> Option<BufferStatus> {
        let probe = self.streams.lock().unwrap_or_else(|e| e.into_inner()).probes.get(&file_idx).cloned()?;
        let b = probe.buffer()?;
        Some(make_buffer_status(b))
    }

    /// Downloads `file_idx` in order behind everything else (the next
    /// episode of a batch); `None` stops it.
    pub fn set_preload_file(&self, file_idx: Option<usize>) -> Result<()> {
        tracing::debug!(info_hash = %self.info_hash, ?file_idx, "sbtl: preload file");
        let stream = match file_idx {
            Some(i) => Some((
                i,
                self.torrent
                    .open_stream(i as u32, &sbtl::StreamOptions { sequential: false, idle: sbtl::Idle::SeqAhead, tail_prefetch: false, ..Default::default() })
                    .map_err(|e| anyhow!("sbtl preload stream for file {i}: {e}"))?,
            )),
            None => None,
        };
        self.streams.lock().unwrap_or_else(|e| e.into_inner()).preload = stream;
        Ok(())
    }

    /// `file_idx`'s path on disk, once the file exists.
    pub fn file_path(&self, file_idx: usize) -> Option<PathBuf> {
        let path = PathBuf::from(&self.sbtl_files().get(file_idx)?.path);
        path.is_file().then_some(path)
    }
}

/// Counts an open reader for as long as it lives.
struct OpenReader(Arc<AtomicUsize>);

impl Drop for OpenReader {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A file reader: `AsyncRead + AsyncSeek` over verified data, whose
/// position drives what downloads.
pub struct Reader {
    pub size: u64,
    /// Torrent-relative path, as in `FileInfo::name`.
    pub name: String,
    stream: sbtl::AsyncStream,
    _open: OpenReader,
}

/// Direct, seekable sbtl stream used by libmpv's synchronous callbacks.
pub struct MpvStream {
    stream: Arc<sbtl::Stream>,
    position: Mutex<u64>,
    cancelled: AtomicBool,
    adaptive_window: Mutex<AdaptiveWindow>,
    len: u64,
    _open: OpenReader,
}

struct AdaptiveWindow {
    playback_speed: f64,
    configured_seconds: Option<u64>,
}

impl MpvStream {
    /// Once sbtl's container index has a measured bitrate, size its
    /// sequential read-ahead window in playback time instead of bytes.
    /// Retrying while the index is probing is cheap and lets the stream
    /// adapt as soon as the index becomes available.
    fn update_adaptive_window(&self) {
        let mut adaptive = self.adaptive_window.lock().unwrap_or_else(|error| error.into_inner());
        let window_seconds = (45.0 * adaptive.playback_speed).round().clamp(15.0, 180.0) as u64;
        if adaptive.configured_seconds == Some(window_seconds) {
            return;
        }
        let media = self.stream.media_info();
        if media.state != sbtl::MediaState::Ready || media.avg_bitrate == 0 {
            return;
        }
        match self.stream.set_window_time(Duration::from_secs(window_seconds)) {
            Ok(()) => {
                adaptive.configured_seconds = Some(window_seconds);
                tracing::info!(
                    avg_bitrate = media.avg_bitrate,
                    window_seconds,
                    playback_speed = adaptive.playback_speed,
                    "sbtl: configured bitrate-adaptive mpv read-ahead window"
                );
            }
            Err(error) => tracing::debug!(%error, "sbtl: couldn't configure adaptive read-ahead window"),
        }
    }

    /// Updates the read-ahead budget when mpv's playback speed changes.
    pub fn set_playback_speed(&self, speed: f64) {
        if !speed.is_finite() || speed <= 0.0 {
            return;
        }
        let speed = speed.clamp(0.25, 4.0);
        self.adaptive_window.lock().unwrap_or_else(|error| error.into_inner()).playback_speed = speed;
        self.update_adaptive_window();
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn position(&self) -> u64 {
        *self.position.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Prioritizes the indexed keyframe for an upcoming player seek. This
    /// changes sbtl's download priority but leaves mpv's byte-read position
    /// alone; mpv still performs the actual seek through `seek` below.
    pub fn prefetch_seek_time(&self, time: Duration) -> Result<(u64, bool)> {
        let key = self.stream.seek_time(time)?;
        let already_available = self.stream.available(key.offset) > 0;
        Ok((key.offset, already_available))
    }

    pub fn buffer_status(&self) -> BufferStatus {
        make_buffer_status(self.stream.buffer())
    }

    pub fn set_idle_policy(&self, paused: bool) -> Result<()> {
        let idle = if paused { sbtl_sys::SBTL_IDLE_SEQ_AHEAD } else { sbtl_sys::SBTL_IDLE_NONE };
        // SAFETY: Stream::id returns the opaque handle for this live stream;
        // self.stream retains ownership until this call returns.
        let stream = self.stream.id() as *mut sbtl_sys::sbtl_stream;
        let result = unsafe { sbtl_sys::sbtl_set_idle_policy(stream, idle) };
        if result != sbtl_sys::SBTL_OK {
            anyhow::bail!("sbtl_set_idle_policy returned {result}");
        }
        tracing::debug!(paused, "sbtl mpv stream idle policy updated");
        Ok(())
    }

    /// Reads verified bytes, waiting in bounded slices so cancellation can
    /// wake the callback promptly. A zero return is reserved for true EOF.
    pub fn read(&self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.update_adaptive_window();
        let mut position = self.position.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "sbtl mpv stream cancelled"));
            }
            if *position >= self.len {
                return Ok(0);
            }
            match self.stream.read_at(*position, buffer) {
                Ok(n) if n > 0 => {
                    *position += n as u64;
                    return Ok(n);
                }
                Ok(_) if *position >= self.len => return Ok(0),
                Ok(_) | Err(sbtl::Error::WOULD_BLOCK) => {
                    match self.stream.wait(*position, 1, Duration::from_millis(250)) {
                        Ok(_) => {}
                        Err(sbtl::Error::CANCELLED) if self.cancelled.load(Ordering::SeqCst) => {
                            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "sbtl mpv stream cancelled"));
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub fn seek(&self, offset: u64) -> std::io::Result<u64> {
        if self.cancelled.load(Ordering::SeqCst) {
            tracing::debug!(offset, "sbtl mpv seek rejected because the stream was cancelled");
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "sbtl mpv stream cancelled"));
        }
        if offset > self.len {
            tracing::warn!(offset, size = self.len, "sbtl mpv seek exceeds the file length");
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek beyond sbtl stream length"));
        }
        let previous = self.position();
        let available_bytes = self.stream.available(offset);
        if offset < self.len {
            match self.stream.wait(offset, 1, Duration::ZERO) {
                Ok(_) | Err(sbtl::Error::WOULD_BLOCK) => {}
                Err(error) => {
                    tracing::warn!(previous, offset, %error, "sbtl mpv seek could not prepare the target");
                    return Err(error.into());
                }
            }
        }
        *self.position.lock().unwrap_or_else(|e| e.into_inner()) = offset;
        tracing::debug!(previous, offset, available_bytes, "sbtl mpv byte seek completed");
        Ok(offset)
    }

    pub fn cancel(&self) {
        let already_cancelled = self.cancelled.swap(true, Ordering::SeqCst);
        if !already_cancelled {
            tracing::debug!("sbtl mpv stream cancellation requested");
        }
        if let Err(error) = self.stream.wake() {
            tracing::debug!(%error, "sbtl mpv stream wake returned an error");
        }
    }
}

impl AsyncRead for Reader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncSeek for Reader {
    fn start_seek(mut self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        Pin::new(&mut self.stream).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Pin::new(&mut self.stream).poll_complete(cx)
    }
}
