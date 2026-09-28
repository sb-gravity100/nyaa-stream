//! Direct torrent reads for in-process FFmpeg inputs: FFmpeg's AVIO read/
//! seek callbacks served straight from `enginefs`'s file stream, instead of
//! FFmpeg's HTTP client reading `stream_handler` over loopback.
//!
//! Why: no HTTP framing/copying per read, and the case the HTTP path could
//! only paper over with FFmpeg's `-reconnect` - `enginefs`'s playback
//! coordinator ending a body early (permit cancellation / lease expiry) -
//! is handled here by reopening the stream at the current byte offset.
//! Every read is also cancellable: `MediaJob::abort` fires the reader's
//! token first, so a read parked on a not-yet-downloaded piece can't hold up
//! a seek restart (ez-ffmpeg's abort waits for its worker threads).
//!
//! The callbacks run on FFmpeg's demux thread (or the `spawn_blocking`
//! thread that opens the input), never on an async worker, so blocking on
//! the runtime there is fine.

use std::io::SeekFrom;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use enginefs::EngineFS;
use ez_ffmpeg::Input;
use ffmpeg_next::ffi;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};
use tokio_util::sync::CancellationToken;

use crate::TorrentId;

/// A single read that makes no progress for this long fails (FFmpeg's
/// `rw_timeout` on the HTTP path).
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Consecutive reopen attempts without reading a byte before the input
/// reports an I/O error.
const MAX_REOPENS: u32 = 8;
/// Backoff between reopen attempts, doubling from 250ms up to this.
const REOPEN_DELAY_MAX: Duration = Duration::from_secs(5);
/// AVIO buffer: FFmpeg asks for one buffer-sized chunk per read callback.
const IO_BUFFER_SIZE: usize = 256 * 1024;

trait ReadSeek: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> ReadSeek for T {}

#[derive(Clone)]
enum Backend {
    Engine(Arc<EngineFS>),
    /// A local file standing in for the torrent (tests).
    #[cfg(test)]
    File(std::path::PathBuf),
}

/// Opens direct readers onto the engine's torrents.
#[derive(Clone)]
pub(crate) struct TorrentSources {
    backend: Backend,
    runtime: tokio::runtime::Handle,
}

impl TorrentSources {
    /// Must be called inside the runtime the readers will block on.
    pub(crate) fn new(efs: Arc<EngineFS>) -> Self {
        Self { backend: Backend::Engine(efs), runtime: tokio::runtime::Handle::current() }
    }

    #[cfg(test)]
    pub(crate) fn local_file(path: std::path::PathBuf) -> Self {
        Self { backend: Backend::File(path), runtime: tokio::runtime::Handle::current() }
    }

    /// A reader for one file. `background` readers never compete with
    /// playback: no playback-lease refresh and `enginefs`'s Background
    /// intent (the same distinction `stream_handler`'s `?intent=background`
    /// makes).
    pub(crate) fn reader(&self, torrent_id: &TorrentId, file_idx: usize, background: bool) -> TorrentReader {
        TorrentReader {
            sources: self.clone(),
            torrent_id: torrent_id.clone(),
            file_idx,
            background,
            size: None,
            pos: 0,
            stream: None,
            cancel: CancellationToken::new(),
        }
    }
}

pub(crate) struct TorrentReader {
    sources: TorrentSources,
    torrent_id: TorrentId,
    file_idx: usize,
    background: bool,
    size: Option<u64>,
    pos: u64,
    stream: Option<Box<dyn ReadSeek>>,
    cancel: CancellationToken,
}

enum Failure {
    Cancelled,
    Retry(String),
}

impl TorrentReader {
    /// Turns the reader into an ez-ffmpeg callback input, plus the token
    /// that cancels its reads.
    pub(crate) fn into_input(self) -> (Input, CancellationToken) {
        let cancel = self.cancel.clone();
        let reader = Arc::new(Mutex::new(self));
        let seeker = reader.clone();
        let input = Input::new_by_read_callback(move |buf| reader.lock().unwrap_or_else(|e| e.into_inner()).read(buf))
            .set_seek_callback(move |offset, whence| seeker.lock().unwrap_or_else(|e| e.into_inner()).seek(offset, whence))
            .set_io_buffer_size(IO_BUFFER_SIZE);
        (input, cancel)
    }

    /// Runs `future` on the runtime unless the reader is cancelled first.
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> Option<T> {
        let cancel = self.cancel.clone();
        self.sources.runtime.block_on(async move {
            tokio::select! {
                _ = cancel.cancelled() => None,
                value = future => Some(value),
            }
        })
    }

    /// Opens (or reopens) the file stream at `self.pos`.
    fn open(&mut self) -> Result<(), Failure> {
        let (backend, torrent_id, file_idx, pos, background) =
            (self.sources.backend.clone(), self.torrent_id.clone(), self.file_idx, self.pos, self.background);
        tracing::debug!(torrent_id = %torrent_id, file_idx, pos, background, "opening direct torrent read");
        let opened = self.block_on(async move {
            let efs = match backend {
                Backend::Engine(efs) => efs,
                #[cfg(test)]
                Backend::File(path) => {
                    let mut file = tokio::fs::File::open(&path).await.map_err(|err| err.to_string())?;
                    let size = file.metadata().await.map_err(|err| err.to_string())?.len();
                    file.seek(SeekFrom::Start(pos)).await.map_err(|err| err.to_string())?;
                    return Ok((Box::new(file) as Box<dyn ReadSeek>, size));
                }
            };
            let engine = efs.get_engine(&torrent_id).await.ok_or_else(|| "unknown torrent".to_string())?;
            if !background {
                efs.refresh_hls_playback(&torrent_id, file_idx, "direct-read").await;
            }
            // Same priorities stream_handler uses: 128 = foreground read,
            // 0 = Background intent.
            let mut handle = engine.get_file(file_idx, pos, if background { 0 } else { 128 }).await.ok_or_else(|| "unknown file index".to_string())?;
            let size = handle.size;
            if pos > 0 && pos < size {
                handle.seek(SeekFrom::Start(pos)).await.map_err(|err| format!("seek to {pos}: {err}"))?;
            }
            Ok::<_, String>((Box::new(handle) as Box<dyn ReadSeek>, size))
        });
        match opened {
            None => Err(Failure::Cancelled),
            Some(Err(err)) => Err(Failure::Retry(err)),
            Some(Ok((stream, size))) => {
                self.stream = Some(stream);
                self.size = Some(size);
                Ok(())
            }
        }
    }

    /// One read attempt on the open stream: bytes read, or why to reopen.
    fn read_once(&mut self, buf: &mut [u8]) -> Result<usize, Failure> {
        let Some(mut stream) = self.stream.take() else {
            self.open()?;
            return self.read_once(buf);
        };
        let result = self.block_on(async { tokio::time::timeout(READ_TIMEOUT, stream.read(buf)).await });
        match result {
            None => Err(Failure::Cancelled),
            Some(Err(_)) => Err(Failure::Retry(format!("no data for {}s", READ_TIMEOUT.as_secs()))),
            Some(Ok(Err(err))) => Err(Failure::Retry(err.to_string())),
            // Short of the file's end: the coordinator ended the stream early.
            Some(Ok(Ok(0))) => Err(Failure::Retry("stream ended early".to_string())),
            Some(Ok(Ok(n))) => {
                self.stream = Some(stream);
                Ok(n)
            }
        }
    }

    /// AVIO read callback: bytes read, `AVERROR_EOF` at the file's end,
    /// `AVERROR_EXIT` once cancelled, `AVERROR(EIO)` after `MAX_REOPENS`.
    fn read(&mut self, buf: &mut [u8]) -> i32 {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..=MAX_REOPENS {
            if self.cancel.is_cancelled() {
                return ffi::AVERROR_EXIT;
            }
            if self.size.is_some_and(|size| self.pos >= size) {
                return ffi::AVERROR_EOF;
            }
            match self.read_once(buf) {
                Ok(n) => {
                    self.pos += n as u64;
                    return n as i32;
                }
                Err(Failure::Cancelled) => {
                    tracing::debug!(torrent_id = %self.torrent_id, file_idx = self.file_idx, "direct torrent read cancelled");
                    return ffi::AVERROR_EXIT;
                }
                Err(Failure::Retry(reason)) => {
                    tracing::warn!(torrent_id = %self.torrent_id, file_idx = self.file_idx, pos = self.pos, attempt, %reason, "direct torrent read failed, reopening");
                    self.stream = None;
                    if self.block_on(tokio::time::sleep(delay)).is_none() {
                        return ffi::AVERROR_EXIT;
                    }
                    delay = (delay * 2).min(REOPEN_DELAY_MAX);
                }
            }
        }
        tracing::error!(torrent_id = %self.torrent_id, file_idx = self.file_idx, pos = self.pos, "direct torrent read giving up");
        ffi::AVERROR(ffi::EIO)
    }

    /// AVIO seek callback (`whence` may carry `AVSEEK_SIZE`/`AVSEEK_FORCE`).
    fn seek(&mut self, offset: i64, whence: i32) -> i64 {
        if self.size.is_none() {
            match self.open() {
                Ok(()) => {}
                Err(Failure::Cancelled) => return ffi::AVERROR_EXIT as i64,
                Err(Failure::Retry(reason)) => {
                    tracing::warn!(torrent_id = %self.torrent_id, file_idx = self.file_idx, %reason, "direct torrent seek could not open the file");
                    return ffi::AVERROR(ffi::EIO) as i64;
                }
            }
        }
        let size = self.size.unwrap_or(0) as i64;
        if whence & ffi::AVSEEK_SIZE != 0 {
            return size;
        }
        let target = match whence & !ffi::AVSEEK_FORCE {
            ffi::SEEK_SET => offset,
            ffi::SEEK_CUR => self.pos as i64 + offset,
            ffi::SEEK_END => size + offset,
            other => {
                tracing::warn!(whence = other, "unsupported direct torrent seek mode");
                return ffi::AVERROR(ffi::ESPIPE) as i64;
            }
        };
        if target < 0 {
            return ffi::AVERROR(ffi::EINVAL) as i64;
        }
        let target_u = target as u64;
        if target_u != self.pos {
            if let Some(mut stream) = self.stream.take() {
                match self.block_on(async { stream.seek(SeekFrom::Start(target_u)).await }) {
                    None => return ffi::AVERROR_EXIT as i64,
                    Some(Ok(_)) => self.stream = Some(stream),
                    // Reopened at the new position by the next read.
                    Some(Err(err)) => tracing::debug!(%err, target, "direct torrent stream seek failed, will reopen"),
                }
            }
            self.pos = target_u;
        }
        target
    }
}
