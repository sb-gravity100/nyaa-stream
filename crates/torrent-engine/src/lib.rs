use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::routing::get;
use axum::Router;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use axum_range::{KnownSize, Ranged};
use sbtl_engine::{Engine, MpvStream as SbtlMpvStream, ReadKind, Source};
pub use sbtl_engine::{BufferStatus, TorrentState};
use serde::Serialize;

/// The torrent engine keys every torrent by its info-hash string (a numeric
/// session id was librqbit's model, which this crate used long ago - see
/// PLAN.md's streaming-server section). Kept as a type alias
/// rather than a bare `String` at call sites so the meaning is documented
/// where it's used.
pub type TorrentId = String;

mod direct_input;
mod media;
pub mod read_stats;
pub mod resume_buffer;
use direct_input::TorrentSources;
use resume_buffer::{BufferedReader, LoadedBuffer, OpenLogHandle, OpenReads, RecordingReader, ResumeBuffers};
use read_stats::{ReadSnapshot, ReadStats, ReadSummary};
pub use media::H264Encoder;



/// How long `TorrentEngine::files` waits for a magnet's metadata (the file
/// list) to arrive from the swarm.
const METADATA_TIMEOUT: Duration = Duration::from_secs(90);
const METADATA_POLL_INTERVAL: Duration = Duration::from_millis(300);


const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "avi", "webm", "mov", "ts", "m2ts", "wmv", "flv"];

/// One file inside a torrent, as listed by its metadata.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFile {
    pub index: usize,
    pub name: String,
    pub length: u64,
    pub is_video: bool,
}

/// See `TorrentEngine::verified_file`.
#[derive(Debug, Clone)]
pub struct VerifiedFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub verified: Vec<(u64, u64)>,
}

/// Largest video file - the sensible default for a single-episode torrent
/// that also carries extras (NCOP/NCED, samples, fonts), and the fallback
/// when a batch's files can't be matched to an episode.
pub fn largest_video_file(files: &[TorrentFile]) -> Option<usize> {
    files.iter().filter(|f| f.is_video).max_by_key(|f| f.length).map(|f| f.index)
}

pub struct TorrentEngine {
    torrents: Arc<Engine>,
    stream_addr: SocketAddr,
    sources: TorrentSources,
    open_reads: OpenReads,
    resume_buffers: ResumeBuffers,
    read_stats: ReadStats,
    storyboards: Arc<StoryboardJobs>,
}

/// Direct, blocking reader passed to libmpv's custom stream callbacks.
pub struct DirectMpvStream {
    inner: SbtlMpvStream,
    resume: Option<Arc<LoadedBuffer>>,
    operation: Mutex<()>,
    log: OpenLogHandle,
    reads: Arc<read_stats::TorrentReads>,
    torrent_id: TorrentId,
    file_idx: usize,
    opened_at: Instant,
    first_byte_logged: std::sync::atomic::AtomicBool,
}

impl DirectMpvStream {
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    pub fn read(&self, buffer: &mut [u8]) -> io::Result<usize> {
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        let position = self.inner.position();
        let started = Instant::now();
        let result = if let Some(bytes) = self.resume.as_ref().and_then(|resume| resume.slice_at(position)) {
            let n = buffer.len().min(bytes.len());
            buffer[..n].copy_from_slice(&bytes[..n]);
            self.inner.seek(position + n as u64).map(|_| n)
        } else {
            self.inner.read(buffer)
        };
        let elapsed = started.elapsed();
        if elapsed >= Duration::from_millis(10) {
            self.reads.waited(elapsed);
        }
        match result {
            Ok(n) => {
                if n > 0 {
                    self.log.add(position, position + n as u64);
                    self.reads.read(n as u64);
                    if !self.first_byte_logged.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        tracing::info!(torrent_id = %self.torrent_id, file_idx = self.file_idx, first_byte_ms = self.opened_at.elapsed().as_millis(), bytes = n, "direct mpv stream delivered first byte");
                    }
                }
                if elapsed >= Duration::from_millis(100) {
                    tracing::debug!(torrent_id = %self.torrent_id, file_idx = self.file_idx, pos = position, bytes = n, wait_ms = elapsed.as_millis(), "direct mpv stream read completed");
                }
                Ok(n)
            }
            Err(error) => {
                if error.kind() == io::ErrorKind::Interrupted {
                    tracing::debug!(torrent_id = %self.torrent_id, file_idx = self.file_idx, pos = position, "direct mpv stream read cancelled");
                } else {
                    tracing::warn!(torrent_id = %self.torrent_id, file_idx = self.file_idx, pos = position, wait_ms = elapsed.as_millis(), %error, "direct mpv stream read failed");
                }
                Err(error)
            }
        }
    }

    pub fn seek(&self, offset: u64) -> io::Result<u64> {
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        self.inner.seek(offset)
    }

    /// Starts indexed torrent prefetch for an absolute mpv seek without
    /// waiting for an in-flight read or changing the reader's byte position.
    pub fn prefetch_seek_time(&self, time: Duration) -> anyhow::Result<(u64, bool)> {
        self.inner.prefetch_seek_time(time)
    }

    pub fn buffer_status(&self) -> BufferStatus {
        self.inner.buffer_status()
    }

    pub fn set_idle_policy(&self, paused: bool) -> anyhow::Result<()> {
        self.inner.set_idle_policy(paused)
    }

    /// Scales sbtl's read-ahead budget with the playback rate.
    pub fn set_playback_speed(&self, speed: f64) {
        self.inner.set_playback_speed(speed);
    }

    pub fn cancel(&self) {
        tracing::debug!(torrent_id = %self.torrent_id, file_idx = self.file_idx, "direct mpv stream cancellation forwarded");
        self.inner.cancel();
    }
}

#[derive(Default)]
struct StoryboardJobs {
    jobs: std::sync::Mutex<HashMap<(TorrentId, usize), StoryboardTask>>,
    root: std::sync::Mutex<PathBuf>,
}

struct StoryboardTask {
    cancel: tokio_util::sync::CancellationToken,
    paused: Arc<std::sync::atomic::AtomicBool>,
}

static H264_ENCODER: tokio::sync::OnceCell<H264Encoder> = tokio::sync::OnceCell::const_new();

/// First H.264 encoder that actually opens on this machine (see
/// `media::detect_h264_encoder`), detected once per process.
pub async fn detect_h264_encoder() -> H264Encoder {
    *H264_ENCODER
        .get_or_init(|| async {
            tokio::task::spawn_blocking(media::detect_h264_encoder).await.unwrap_or_else(|err| {
                tracing::error!(%err, "encoder detection panicked, falling back to libopenh264");
                H264Encoder::OpenH264
            })
        })
        .await
}

pub struct AddedTorrent {
    pub id: TorrentId,
}

#[derive(Clone)]
struct StreamRouterState {
    torrents: Arc<Engine>,
    open_reads: OpenReads,
    resume_buffers: ResumeBuffers,
    read_stats: ReadStats,
}

/// mpv's start/resume threshold (`--cache-pause-wait`, seconds). sbtl only
/// serves verified data and reports when its ready threshold is buffered
/// ahead of the reader, so mpv needn't hold back for 10 s of its own cache.
pub const MPV_CACHE_PAUSE_WAIT_SECS: u32 = (sbtl_engine::READY_MS / 1000) as u32;

/// Trimmed-down mirror of librqbit's `TorrentStats` - mirrors Stremio's own
/// streaming-server statistics endpoint (`GET /:infoHash/stats.json`,
/// see `reference/stremio-core`'s `models::streaming_server`), which the
/// player UI polls to show download progress/speed/peers while a stream is
/// buffering, rather than leaving the user staring at a blank window with
/// no feedback while mpv waits for enough data to start decoding.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamStats {
    /// "initializing" | "live" | "paused" | "error"
    pub state: String,
    pub progress_percent: f64,
    /// Despite the name, librqbit's underlying `Speed::mbps` field is
    /// actually MiB/s (see its `Display` impl) - mirrored as-is here.
    pub download_speed_mbps: f64,
    pub connected_peers: u32,
    pub finished: bool,
    /// Raw byte counts (alongside `progress_percent`) so the frontend can
    /// port stremio-web's own weighted buffering-readiness score
    /// (`useStatistics.ts`'s `getLoadingProgress`: peers/downloaded-vs-
    /// threshold/speed, not just raw completion percent) rather than a
    /// cruder "% of the whole file" estimate.
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    /// The file's verified bytes as merged `[start, end)` runs, relative to
    /// the file start. The mpv player maps them onto its timeline for the
    /// seek bar's "downloaded" layer (mpv's own buffer forgets ranges after
    /// a seek). Empty when the engine can't tell.
    pub downloaded_byte_ranges: Vec<(u64, u64)>,
    // --- Verbose statistics popup ---
    pub torrent_name: String,
    pub file_name: String,
    pub upload_speed_mbps: f64,
    pub uploaded_bytes: u64,
    /// Peers sending us data / waiting to connect / the whole known swarm.
    pub unchoked_peers: u64,
    pub queued_peers: u64,
    pub swarm_size: u64,
    /// Peer sources (trackers, DHT...) queried.
    pub sources: usize,
    /// Verified data ahead of the player's newest read of this file, when
    /// sbtl has seen a foreground reader of it (`None` before). PLAN.md "sbtl
    /// backend": the buffering UI shows real readiness from it.
    pub buffer: Option<BufferStatus>,
}

async fn storyboard_worker(
    torrents: Arc<Engine>, id: TorrentId, file_idx: usize, root: PathBuf, job_id: u64,
    cancel: tokio_util::sync::CancellationToken,
    paused: Arc<std::sync::atomic::AtomicBool>,
) {
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
    const CHUNK: u64 = 8 * 1024 * 1024;
    const POLL: Duration = Duration::from_secs(2);
    let dir = root.join(&id).join(file_idx.to_string());
    let snapshot = dir.join(format!("verified-prefix-{job_id}.snapshot"));
    let temp = dir.join(format!("verified-prefix-{job_id}.tmp"));
    let mut processed_prefix = 0u64;
    let mut attempted_prefix = 0u64;
    tracing::info!(torrent_id = %id, file_idx, "verified-prefix storyboard worker started");
    loop {
        if cancel.is_cancelled() { break; }
        if paused.load(std::sync::atomic::Ordering::Relaxed) {
            tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis(200)) => continue }
        }
        let Some(torrent) = torrents.get(&id) else { break; };
        let Some(file) = torrent.stats().files.get(file_idx).cloned() else { break; };
        let mut ranges = file.ranges;
        ranges.sort_unstable_by_key(|r| r.0);
        // Only the fully verified run beginning at byte zero is safe to
        // present to a container demuxer. Never seek across a hole.
        let mut prefix = 0u64;
        for (start, end) in ranges {
            if start > prefix { break; }
            prefix = prefix.max(end.min(file.length));
            if prefix >= file.length { break; }
        }
        if prefix >= attempted_prefix.saturating_add(CHUNK) || (prefix == file.length && prefix > attempted_prefix) {
            let result = async {
                tokio::fs::create_dir_all(&dir).await?;
                let mut out = tokio::fs::File::create(&temp).await?;
                let mut reader = torrent.open(file_idx, 0, ReadKind::Background)?;
                let mut start = 0u64;
                while start < prefix {
                    if cancel.is_cancelled() { anyhow::bail!("storyboard cancelled"); }
                    let end = (start + CHUNK).min(prefix);
                    reader.seek(std::io::SeekFrom::Start(start)).await?;
                    let mut bytes = vec![0u8; usize::try_from(end - start)?];
                    reader.read_exact(&mut bytes).await?;
                    out.write_all(&bytes).await?;
                    start = end;
                }
                out.flush().await?;
                drop(out);
                let _ = tokio::fs::remove_file(&snapshot).await;
                tokio::fs::rename(&temp, &snapshot).await?;
                anyhow::Ok(())
            }.await;
            match result {
                Ok(()) => {
                    let snapshot = snapshot.clone();
                    let output_dir = dir.clone();
                    let cancel_for_decode = cancel.clone();
                    let paused_for_decode = paused.clone();
                    match tokio::task::spawn_blocking(move || media::storyboard_prefix(&snapshot, &output_dir, &cancel_for_decode, &paused_for_decode)).await {
                        Ok(Ok(_)) => processed_prefix = prefix,
                        Ok(Err(err)) => tracing::debug!(torrent_id = %id, file_idx, %err, "verified-prefix storyboard pass could not decode yet"),
                        Err(err) => tracing::warn!(torrent_id = %id, file_idx, %err, "storyboard decoder task failed"),
                    }
                    attempted_prefix = prefix;
                }
                Err(_err) if cancel.is_cancelled() => break,
                Err(err) => tracing::debug!(torrent_id = %id, file_idx, %err, verified_prefix = prefix, "storyboard snapshot read failed"),
            }
        }
        if prefix == file.length && (processed_prefix == file.length || attempted_prefix == prefix) { break; }
        tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(POLL) => {} }
    }
    let _ = tokio::fs::remove_file(&temp).await;
    let _ = tokio::fs::remove_file(&snapshot).await;
    tracing::info!(torrent_id = %id, file_idx, "verified-prefix storyboard worker ended");
}

impl TorrentEngine {
    /// Starts the sbtl engine (`sbtl_engine`), rooted at
    /// `download_dir` and a local HTTP server that serves torrent file
    /// bytes with Range support, mirroring Stremio's local streaming
    /// server. Disk-backed, so partial downloads survive an app restart -
    /// see PLAN.md's streaming-server section.
    pub async fn start(download_dir: PathBuf) -> anyhow::Result<Self> {
        tracing::debug!(?download_dir, "starting the sbtl engine");
        let storyboard_root = download_dir
            .parent()
            .map(|parent| parent.join("storyboard"))
            .unwrap_or_else(|| download_dir.join("storyboard"));
        let torrents = Engine::start(download_dir)?;
        let sources = TorrentSources::new(torrents.clone());
        let storyboards = Arc::new(StoryboardJobs::default());
        *storyboards.root.lock().unwrap_or_else(|e| e.into_inner()) = storyboard_root;
        let open_reads = OpenReads::default();
        let resume_buffers = ResumeBuffers::default();
        let read_stats = ReadStats::default();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let stream_addr = listener.local_addr()?;
        tracing::info!(%stream_addr, "streaming server listening");

        let router_state =
            StreamRouterState {
            torrents: torrents.clone(),
            open_reads: open_reads.clone(),
            resume_buffers: resume_buffers.clone(),
            read_stats: read_stats.clone(),
        };
        let app = Router::new()
            .route("/stream/{torrent_id}/{file_idx}", get(stream_handler))
            .with_state(router_state);
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!(?err, "streaming server stopped");
            }
        });

        Ok(Self { torrents, stream_addr, sources, open_reads, resume_buffers, read_stats, storyboards })
    }

    /// Adds a torrent from a magnet link or .torrent URL and starts
    /// downloading it. sbtl downloads whatever its readers read (the read
    /// position is the playback cursor), so no extra "sequential mode" flag
    /// is needed here.
    pub async fn add(&self, magnet_or_url: &str) -> anyhow::Result<AddedTorrent> {
        tracing::debug!("adding torrent");
        let torrent = self.torrents.add(Source::Url(magnet_or_url.to_string())).await.map_err(|err| {
            tracing::error!(%err, "failed to add torrent");
            err
        })?;
        let id = torrent.info_hash().to_string();
        tracing::info!(torrent_id = %id, "torrent added");
        Ok(AddedTorrent { id })
    }

    /// Removes a torrent from the engine. Used to clean up after a scratch download that only
    /// existed to pull a thumbnail frame out of the first few seconds of an
    /// episode - unlike a torrent the user actually chose to watch, there's
    /// no reason to keep seeding or keep the partial file around afterward.
    ///
    /// Note: the engine's `remove` keeps the downloaded files
    /// (so a re-opened episode resumes from disk).
    pub async fn remove(&self, id: TorrentId) -> anyhow::Result<()> {
        tracing::debug!(torrent_id = %id, "removing torrent");
        self.stop_storyboards_for_torrent(&id);
        self.open_reads.remove_torrent(&id);
        self.resume_buffers.remove_torrent(&id);
        self.read_stats.remove_torrent(&id);
        self.torrents.remove(&id);
        Ok(())
    }

    /// Starts paced storyboard generation for a video file. The worker only
    /// snapshots the contiguous verified prefix (starting at byte zero), so
    /// FFmpeg never reads the live partial file or waits for missing pieces.
    pub fn start_storyboard(&self, id: &TorrentId, file_idx: usize) -> anyhow::Result<()> {
        tracing::info!(torrent_id = %id, file_idx, "storyboard generation requested");
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        anyhow::ensure!(torrent.stats().files.get(file_idx).is_some(), "unknown file {file_idx} in torrent {id}");
        let key = (id.clone(), file_idx);
        let cancel = tokio_util::sync::CancellationToken::new();
        // Player polling starts the worker; its buffering state immediately
        // pauses or resumes this initial state through `pause_storyboard`.
        let paused = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut jobs = self.storyboards.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if jobs.contains_key(&key) { return Ok(()); }
        jobs.insert(key, StoryboardTask { cancel: cancel.clone(), paused: paused.clone() });
        drop(jobs);
        let root = self.storyboards.root.lock().unwrap_or_else(|e| e.into_inner()).clone();
        static NEXT_JOB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        tokio::spawn(storyboard_worker(self.torrents.clone(), id.clone(), file_idx, root, NEXT_JOB.fetch_add(1, std::sync::atomic::Ordering::Relaxed), cancel, paused));
        Ok(())
    }

    /// Pauses/resumes storyboard decode while playback is buffering.
    pub fn pause_storyboard(&self, id: &TorrentId, file_idx: usize, paused: bool) -> bool {
        tracing::debug!(torrent_id = %id, file_idx, paused, "storyboard pause state changed");
        let jobs = self.storyboards.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.get(&(id.clone(), file_idx)) {
            job.paused.store(paused, std::sync::atomic::Ordering::Relaxed);
            true
        } else { false }
    }

    /// Cancels storyboard generation for a file. Completed frames remain in cache.
    pub fn stop_storyboard(&self, id: &TorrentId, file_idx: usize) -> bool {
        tracing::info!(torrent_id = %id, file_idx, "storyboard generation stopped");
        self.storyboards.jobs.lock().unwrap_or_else(|e| e.into_inner()).remove(&(id.clone(), file_idx)).is_some_and(|job| { job.cancel.cancel(); true })
    }

    /// Looks up the cached frame for `floor(seconds / 5)` without starting work.
    pub fn storyboard_frame(&self, id: &TorrentId, file_idx: usize, seconds: f64) -> Option<PathBuf> {
        tracing::debug!(torrent_id = %id, file_idx, seconds, "storyboard frame lookup");
        if !seconds.is_finite() || seconds < 0.0 { return None; }
        let path = self.storyboards.root.lock().unwrap_or_else(|e| e.into_inner()).join(id).join(file_idx.to_string()).join(format!("{}.jpg", (seconds / 5.0).floor() as u64));
        path.is_file().then_some(path)
    }

    /// Removes all cached storyboard frames for a torrent after the player
    /// closes, unless a resume buffer keeps that episode's cache alive.
    pub async fn clear_storyboard_cache(&self, id: &TorrentId) -> anyhow::Result<()> {
        self.stop_storyboards_for_torrent(id);
        let dir = self.storyboards.root.lock().unwrap_or_else(|e| e.into_inner()).join(id);
        for attempt in 0..10 {
            match tokio::fs::remove_dir_all(&dir).await {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_error) if attempt < 9 => tokio::time::sleep(Duration::from_millis(50)).await,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn stop_storyboards_for_torrent(&self, id: &str) {
        let mut jobs = self.storyboards.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.retain(|(torrent_id, _), job| {
            if torrent_id == id { job.cancel.cancel(); false } else { true }
        });
    }

    /// Fetches `file_idx` (e.g. the next episode of a batch) at the lowest
    /// priority alongside the file being played; `None` stops. Only spare
    /// bandwidth goes to it - the playing file keeps its own priorities.
    pub async fn preload_file(&self, id: &TorrentId, file_idx: Option<usize>) -> anyhow::Result<()> {
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        tracing::debug!(torrent_id = %id, ?file_idx, "preload file requested");
        torrent.set_preload_file(file_idx)
    }

    /// The player finished opening `file_idx` (mpv's `file-loaded`): stops
    /// recording its open reads (see `resume_buffer`).
    pub fn finish_open_reads(&self, id: &TorrentId, file_idx: usize) {
        self.open_reads.finish(id, file_idx);
    }

    /// Serves `file_idx` of `id` from `buffer` where it covers a read (see
    /// `resume_buffer::BufferedReader`) until the torrent is removed.
    pub fn attach_resume_buffer(&self, id: &TorrentId, file_idx: usize, buffer: Arc<LoadedBuffer>) {
        self.resume_buffers.attach(id, file_idx, buffer);
    }

    /// File-relative `[start, end)` ranges the player read to open the file.
    pub fn open_read_ranges(&self, id: &TorrentId, file_idx: usize) -> Vec<(u64, u64)> {
        self.open_reads.ranges(id, file_idx)
    }

    /// What a resume buffer of `file_idx` can copy: the file on disk, its
    /// size, and its verified `[start, end)` byte runs (whole pieces clipped
    /// to the file - hash-checked, so safe to serve as-is).
    pub async fn verified_file(&self, id: &TorrentId, file_idx: usize) -> anyhow::Result<VerifiedFile> {
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let path = torrent.file_path(file_idx).ok_or_else(|| anyhow::anyhow!("no file on disk for {id}/{file_idx}"))?;
        let stats = torrent.stats();
        let file = stats.files.get(file_idx).ok_or_else(|| anyhow::anyhow!("no file {file_idx} in {id}"))?;
        Ok(VerifiedFile { path, name: file.name.clone(), size: file.length, verified: file.ranges.clone() })
    }

    /// Load profiler: `id`'s stream byte counters now (a trace's baseline).
    pub fn read_snapshot(&self, id: &TorrentId) -> ReadSnapshot {
        self.read_stats.snapshot(id)
    }

    /// Load profiler: what `id`'s streams did since `since` / `base`.
    pub fn read_summary(&self, id: &TorrentId, since: std::time::Instant, base: ReadSnapshot) -> ReadSummary {
        self.read_stats.summary_since(id, since, base)
    }

    /// `id`'s state (checking data on disk, fetching metadata,
    /// downloading, finished), `None` if unknown.
    pub fn torrent_state(&self, id: &TorrentId) -> Option<TorrentState> {
        Some(self.torrents.get(id)?.stats().state)
    }

    /// Reads `ranges` of `file_idx` through the engine's own reader (as a
    /// background read: no playback lease, lowest priority). Only for
    /// verified ranges - an unverified one would wait on the swarm, bounded
    /// by a timeout. The engine serves a just-verified piece from sbtl's
    /// copy, which the file on disk may not have yet.
    pub async fn read_ranges(&self, id: &TorrentId, file_idx: usize, ranges: &[(u64, u64)]) -> anyhow::Result<Vec<(u64, Vec<u8>)>> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        const TIMEOUT: Duration = Duration::from_secs(30);
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let read = async {
            let mut handle = torrent.open(file_idx, 0, ReadKind::Background)?;
            let mut runs = Vec::with_capacity(ranges.len());
            for &(start, end) in ranges {
                handle.seek(std::io::SeekFrom::Start(start)).await?;
                let mut bytes = vec![0; usize::try_from(end - start)?];
                handle.read_exact(&mut bytes).await?;
                runs.push((start, bytes));
            }
            anyhow::Ok(runs)
        };
        let runs = tokio::time::timeout(TIMEOUT, read).await.map_err(|_| anyhow::anyhow!("reading {id}/{file_idx} timed out after {TIMEOUT:?}"))??;
        tracing::debug!(torrent_id = %id, file_idx, ranges = ranges.len(), "verified ranges read through the engine");
        Ok(runs)
    }

    /// Byte offset of the keyframe at or before `seconds` in the file at
    /// `path` (see `media::keyframe_byte_offset`), bounded by a timeout.
    pub async fn keyframe_byte_offset(path: PathBuf, seconds: f64) -> anyhow::Result<Option<u64>> {
        const TIMEOUT: Duration = Duration::from_secs(10);
        let lookup = tokio::task::spawn_blocking(move || media::keyframe_byte_offset(&path, seconds));
        match tokio::time::timeout(TIMEOUT, lookup).await {
            Ok(joined) => joined?,
            Err(_) => anyhow::bail!("keyframe lookup timed out after {TIMEOUT:?}"),
        }
    }

    /// Waits (up to `METADATA_TIMEOUT`) for `id`'s metadata and returns its
    /// file list. A magnet link carries no file list, so nothing about
    /// which file to stream can be decided before this resolves.
    pub async fn files(&self, id: &TorrentId) -> anyhow::Result<Vec<TorrentFile>> {
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let started = tokio::time::Instant::now();
        loop {
            let files = torrent.files();
            if !files.is_empty() {
                let files: Vec<TorrentFile> = files
                    .into_iter()
                    .enumerate()
                    .map(|(index, file)| {
                        let extension = file.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                        TorrentFile { index, is_video: VIDEO_EXTENSIONS.contains(&extension.as_str()), name: file.name, length: file.length }
                    })
                    .collect();
                tracing::info!(torrent_id = %id, count = files.len(), wait_ms = started.elapsed().as_millis() as u64, "torrent metadata available");
                return Ok(files);
            }
            if started.elapsed() >= METADATA_TIMEOUT {
                tracing::warn!(torrent_id = %id, "timed out waiting for torrent metadata");
                anyhow::bail!("timed out waiting for torrent metadata (no peers?)");
            }
            tokio::time::sleep(METADATA_POLL_INTERVAL).await;
        }
    }

    /// URL that streams a specific file within an already-added torrent.
    /// The local HTTP server serves it with Range support so playback can
    /// start before the whole torrent has downloaded. Raw container bytes
    /// are opened by embedded mpv.
    pub fn stream_url(&self, torrent_id: &TorrentId, file_idx: usize) -> String {
        format!("http://{}/stream/{}/{}", self.stream_addr, torrent_id, file_idx)
    }

    /// Opens a torrent-backed stream for libmpv's `sbtl://` callbacks.
    pub fn open_mpv_stream(&self, torrent_id: &str, file_idx: usize) -> anyhow::Result<DirectMpvStream> {
        let torrent = self.torrents.get(torrent_id).ok_or_else(|| anyhow::anyhow!("unknown torrent {torrent_id}"))?;
        self.torrents.touch_playback(torrent_id, file_idx, "mpv-direct");
        let inner = torrent.open_mpv_stream(file_idx)?;
        let torrent_id = torrent_id.to_string();
        Ok(DirectMpvStream {
            inner,
            resume: self.resume_buffers.get(&torrent_id, file_idx),
            operation: Mutex::new(()),
            log: self.open_reads.log_for(&torrent_id, file_idx),
            reads: self.read_stats.for_torrent(&torrent_id),
            torrent_id: torrent_id.clone(),
            file_idx,
            opened_at: Instant::now(),
            first_byte_logged: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Cuts `[start_seconds, end_seconds)` of a torrent file into an MP4 at
    /// `out` with normalized codecs (hardware H.264 when available + AAC) -
    /// see `media::export_clip`. Reads the torrent at foreground priority, so
    /// it waits on pieces the player hasn't downloaded yet. Returns once the
    /// file is written.
    pub async fn export_clip(&self, torrent_id: &TorrentId, file_idx: usize, start_seconds: f64, end_seconds: f64, audio_stream: usize, out: PathBuf) -> anyhow::Result<()> {
        let encoder = detect_h264_encoder().await;
        let input = media::InputSource::Torrent(self.sources.reader(torrent_id, file_idx, false));
        tracing::info!(torrent_id = %torrent_id, file_idx, start_seconds, end_seconds, audio_stream, out = %out.display(), "clip export requested");
        tokio::task::spawn_blocking(move || media::export_clip(media::ClipExport { input, out: &out, start_seconds, end_seconds, audio_stream, encoder }))
            .await
            .map_err(|err| anyhow::anyhow!("clip export panicked: {err}"))?
    }

    /// Captures one frame using FFmpeg's direct, seekable sbtl reader.
    pub async fn capture_thumbnail_frame(&self, torrent_id: &TorrentId, file_idx: usize, start_seconds: f64, out: PathBuf, timeout: Duration) -> anyhow::Result<()> {
        let input = media::InputSource::Torrent(self.sources.reader(torrent_id, file_idx, true));
        tracing::info!(torrent_id = %torrent_id, file_idx, start_seconds, out = %out.display(), "torrent thumbnail frame requested");
        tokio::task::spawn_blocking(move || media::capture_thumbnail_frame(input, &out, start_seconds, timeout))
            .await
            .map_err(|err| anyhow::anyhow!("thumbnail frame capture panicked: {err}"))?
    }

    /// Download progress/speed/peer-count snapshot for an in-progress
    /// torrent, polled by the frontend to show buffering feedback while
    /// mpv waits for enough data to start decoding.
    pub async fn stats(&self, id: &TorrentId, file_idx: usize) -> anyhow::Result<StreamStats> {
        let torrent = self.torrents.get(id).ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let stats = torrent.stats();

        // The streamed file's own progress, not the whole torrent's - for
        // a batch those differ by an order of magnitude. Falls back to the
        // torrent total if the index is somehow out of range.
        let downloaded_byte_ranges = stats.files.get(file_idx).map(|file| file.ranges.clone()).unwrap_or_default();
        let (total_bytes, downloaded_bytes) = match stats.files.get(file_idx) {
            Some(file) => (file.length, file.downloaded),
            None => (stats.files.iter().map(|f| f.length).sum(), stats.files.iter().map(|f| f.downloaded).sum()),
        };
        let progress_percent = if total_bytes > 0 { downloaded_bytes as f64 / total_bytes as f64 * 100.0 } else { 0.0 };
        // The engine reports bytes/sec; `StreamStats` documents MiB/s
        // (consumed by the frontend).
        let download_speed_mbps = stats.download_rate / (1024.0 * 1024.0);
        let state = if !stats.has_metadata { "initializing" } else { "live" };

        Ok(StreamStats {
            state: state.to_string(),
            progress_percent,
            download_speed_mbps,
            connected_peers: stats.peers as u32,
            finished: stats.finished,
            downloaded_bytes,
            total_bytes,
            downloaded_byte_ranges,
            torrent_name: stats.name.clone(),
            file_name: stats.files.get(file_idx).map(|f| f.name.clone()).unwrap_or_default(),
            upload_speed_mbps: stats.upload_rate / (1024.0 * 1024.0),
            uploaded_bytes: stats.uploaded,
            unchoked_peers: stats.peers,
            queued_peers: 0,
            swarm_size: stats.peers_known,
            sources: 0,
            buffer: torrent.buffer(file_idx),
        })
    }
}

/// Content type for raw file streams and clip export, mapped from extension
/// because sbtl doesn't expose a MIME type.
fn mime_for_filename(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "ts" | "m2ts" => "video/mp2t",
        _ => "application/octet-stream",
    }
}

async fn stream_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx)): Path<(TorrentId, usize)>,
    range: Option<TypedHeader<Range>>,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    use axum::response::IntoResponse;
    tracing::debug!(torrent_id = %torrent_id, file_idx, "stream request received");
    let torrent = state.torrents.get(&torrent_id).ok_or_else(|| {
        tracing::warn!(torrent_id = %torrent_id, file_idx, "stream request for unknown torrent");
        axum::http::StatusCode::NOT_FOUND
    })?;

    // Refreshes this file's playback lease, which keeps the engine from
    // removing the torrent as inactive - a cheap early return on the
    // common case of a repeat request for the file already being watched.
    // Called on every request rather than once when the stream starts,
    // since the lease expires (sbtl's own readers decide what downloads).
    state.torrents.touch_playback(&torrent_id, file_idx, "raw-stream");

    let range = range.map(|TypedHeader(range)| range);
    let file = torrent.files().into_iter().nth(file_idx);
    let file_size = file.as_ref().map(|file| file.length).unwrap_or_default();
    let content_type = file
        .as_ref()
        .map(|file| mime_for_filename(&file.name))
        .unwrap_or("application/octet-stream");
    let headers = [(axum::http::header::CONTENT_TYPE, content_type)];
    // Foreground reads are what mpv opens the file with - recorded for the
    // resume buffer until it reports file-loaded.
    let log = Some(state.open_reads.log_for(&torrent_id, file_idx));
    let reads = Some(state.read_stats.for_torrent(&torrent_id));
    if let Some(reads) = &reads {
        reads.request(range.as_ref(), file_size);
    }
    let kind = ReadKind::Foreground;

    // A Continue-watching resume: its buffer answers what it covers, and
    // the torrent file is only opened (lazily) where it stops - so playback
    // starts while sbtl is still re-checking or reconnecting.
    if let Some(buffer) = state.resume_buffers.get(&torrent_id, file_idx) {
        tracing::debug!(torrent_id = %torrent_id, file_idx, "stream served through the resume buffer");
        let byte_size = buffer.file_size;
        let torrent = torrent.clone();
        let open: resume_buffer::OpenFuture<_> = Box::pin(async move { torrent.open(file_idx, 0, kind).ok() });
        let body = KnownSize::sized(RecordingReader::new(BufferedReader::new(buffer, open, reads.clone()), log, reads), byte_size);
        return Ok((headers, Ranged::new(range, body)).into_response());
    }

    let file_handle = torrent.open(file_idx, 0, kind).map_err(|err| {
        tracing::warn!(torrent_id = %torrent_id, file_idx, %err, "stream request for an unopenable file");
        axum::http::StatusCode::NOT_FOUND
    })?;
    let byte_size = file_handle.size;
    let body = KnownSize::sized(RecordingReader::new(file_handle, log, reads), byte_size);
    Ok((headers, Ranged::new(range, body)).into_response())
}
