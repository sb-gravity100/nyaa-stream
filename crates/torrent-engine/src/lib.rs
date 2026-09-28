use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::Router;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use axum_range::{KnownSize, Ranged};
use enginefs::backend::{BackendConfig, TorrentBackend, TorrentHandle, TorrentSource};
use enginefs::EngineFS;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

/// `enginefs`'s `BackendEngineFS` keys every torrent by its info-hash
/// string rather than a numeric session id (librqbit's model, which this
/// crate used before vendoring `enginefs`'s libtorrent backend - see
/// PLAN.md's streaming-server section for why). Kept as a type alias
/// rather than a bare `String` at call sites so the meaning is documented
/// where it's used.
pub type TorrentId = String;

mod media;
pub use media::H264Encoder;



/// HLS segment length. 6s is Apple's low-latency recommendation.
const SEGMENT_DURATION_SECONDS: f64 = 6.0;

/// A request ahead of a running job's progress waits for that job only if
/// the job is expected to produce it within this long (at its measured
/// production rate) - otherwise it's treated as a seek and the job is
/// restarted at the target. Replaced a fixed 20-segment (~2 min) lookahead:
/// any forward seek inside that window used to wait for the sequential job
/// to download and process everything in between, which on a
/// download-bound torrent meant tens of seconds per short seek.
const RESTART_WAIT_BUDGET: Duration = Duration::from_secs(4);

/// Before a job has produced anything there's no rate to go on - requests
/// this close to its start are assumed to be hls.js's own sequential
/// prefetch, anything further is a seek.
const RESTART_COLD_LOOKAHEAD_SEGMENTS: usize = 2;

/// How long a segment request waits for the transcode job to produce the
/// file before giving up - bounded mostly by torrent download speed, not
/// transcode speed, since `ffmpeg` blocks on `stream_handler` for
/// not-yet-downloaded bytes.
const SEGMENT_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const SEGMENT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long a stream probe (`ffprobe` over loopback) may block waiting for
/// the container header to download before the caller gives up - the HLS
/// job then starts without subtitle outputs rather than stalling playback.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a run waits for the media probe to decide copy vs. transcode.
/// Waiting is nearly free: the run itself needs the same container header
/// before it can produce anything (verified live - the first segment landed
/// ~1s after a slow probe finished). A shorter 12s wait fell back to copy
/// and cached it, which would have left an HEVC release undecodable for the
/// whole session.
const PLAN_PROBE_WAIT: Duration = Duration::from_secs(45);

/// How long `font_handler` waits for the background attachment dump to
/// land a requested font on disk.
const FONT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Largest video file - the sensible default for a single-episode torrent
/// that also carries extras (NCOP/NCED, samples, fonts), and the fallback
/// when a batch's files can't be matched to an episode.
pub fn largest_video_file(files: &[TorrentFile]) -> Option<usize> {
    files.iter().filter(|f| f.is_video).max_by_key(|f| f.length).map(|f| f.index)
}

pub struct TorrentEngine {
    efs: Arc<EngineFS>,
    stream_addr: SocketAddr,
    hls_jobs: HlsJobs,
    probes: MediaProbes,
}

/// What the frontend's WebView can decode natively through MSE, reported
/// by `set_decoder_support`. Conservative defaults (nothing beyond 8-bit
/// H.264) until the frontend says otherwise.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecoderSupport {
    pub hevc: bool,
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

/// How a run handles the video stream.
#[derive(Debug, Clone)]
enum VideoPlan {
    /// Stream-copy: the source is something the WebView decodes natively.
    Copy,
    /// Re-encode to 8-bit H.264 High. `reason` is for logs/UI.
    Transcode { encoder: H264Encoder, reason: String },
}

impl VideoPlan {
    fn label(&self) -> String {
        match self {
            Self::Copy => "direct".to_string(),
            Self::Transcode { encoder, reason } => format!("{reason} -> H.264 ({})", encoder.ffmpeg_name()),
        }
    }
}

/// Decides copy vs. transcode from the probed video stream. Browsers (and
/// hls.js's MPEG-TS demuxer) only reliably handle 8-bit 4:2:0 H.264;
/// 10-bit "Hi10P" H.264 - still common in anime fansub releases - fails to
/// decode even though the codec name matches, and AV1/VP9/MPEG-4 can't be
/// carried in MPEG-TS for hls.js at all. HEVC is copied only when the
/// frontend reported native HEVC decode.
async fn plan_video(video: Option<&VideoStreamInfo>, support: DecoderSupport) -> VideoPlan {
    let Some(video) = video else {
        // Unknown (probe failed): copying is what always worked before.
        return VideoPlan::Copy;
    };
    let eight_bit_420 = matches!(video.pix_fmt.as_deref(), None | Some("yuv420p" | "yuvj420p"));
    let reason = match video.codec.as_str() {
        "h264" if eight_bit_420 => return VideoPlan::Copy,
        "h264" => format!("H.264 {}", video.pix_fmt.as_deref().unwrap_or("high bit depth")),
        "hevc" if support.hevc && eight_bit_420 => return VideoPlan::Copy,
        "hevc" => "HEVC".to_string(),
        "av1" => "AV1".to_string(),
        other => other.to_uppercase(),
    };
    VideoPlan::Transcode { encoder: detect_h264_encoder().await, reason }
}

/// One in-progress `ffmpeg` HLS transcode for a specific torrent file -
/// see `HlsJobs` for why there's at most one of these per file and how
/// requests decide whether to reuse or restart it.
struct HlsJob {
    child: media::MediaJob,
    dir: PathBuf,
    /// The segment index this job was told to start at (via `-ss`/
    /// `-start_number`) - segment requests below this were generated by an
    /// *earlier* job run and, if missing, need a fresh restart to fill the
    /// gap rather than waiting on this job to ever produce them.
    start_index: usize,
    started_at: tokio::time::Instant,
    /// Whether this run is writing subtitle outputs - false when it started
    /// before the media probe finished; `attach_subtitles` then adds a
    /// subtitle-only process for the same start offset.
    has_subtitles: bool,
    subtitle_child: Option<media::MediaJob>,
    /// `VideoPlan::label` of this run, reported in `StreamStats`.
    video_mode: String,
}

impl HlsJob {
    fn kill(&mut self) {
        let was_running = !self.child.is_ended();
        // Returns once the run's workers have released (ez-ffmpeg's drop
        // waits for them), so its playlist below is final.
        self.child.abort();
        if let Some(child) = self.subtitle_child.as_mut() {
            child.abort();
        }
        if was_running {
            drop_aborted_tail_segment(&self.dir);
        }
    }
}

/// Deletes the last segment of the run whose `ffmpeg_internal.m3u8` is in
/// `dir`. Aborting a run still finalizes its in-progress segment and renames
/// it into place, cut short (0.0s-0.9s seen in the cache), and
/// `ensure_segment_available` treats any existing file as complete - so a
/// seek restart left a hole of up to a segment that was served forever.
/// Only called for a run aborted while still running, where the last entry
/// is by definition that truncated segment.
fn drop_aborted_tail_segment(dir: &FsPath) {
    let playlist = match std::fs::read_to_string(dir.join("ffmpeg_internal.m3u8")) {
        Ok(playlist) => playlist,
        Err(err) => {
            tracing::debug!(?dir, %err, "no playlist for aborted hls run");
            return;
        }
    };
    let Some(last) = playlist.lines().rev().map(str::trim).find(|line| line.ends_with(".ts")) else {
        tracing::debug!(?dir, "aborted hls run produced no segments");
        return;
    };
    match std::fs::remove_file(dir.join(last)) {
        Ok(()) => tracing::info!(?dir, segment = last, "dropped truncated segment of aborted hls run"),
        Err(err) => tracing::warn!(?dir, segment = last, %err, "failed to drop truncated segment of aborted hls run"),
    }
}

/// Tracks the single running (or most recently run) `ffmpeg` transcode job
/// per torrent file, keyed so unrelated files never block each other's
/// decisions while same-file requests serialize on one lock.
///
/// Replaces an earlier design that spawned an independent `ffmpeg -ss
/// ... -t 6 ...` process per *segment*: each one reinitialized its own AAC
/// encoder and re-based timestamps from zero, which produced audible
/// discontinuities at every segment boundary and, since `-ss` before `-i`
/// only approximately seeks without a Matroska Cues index (not available
/// on a still-downloading torrent), sometimes fed the muxer inconsistent
/// timestamps outright ("Error submitting a packet to the muxer: Invalid
/// argument" - verified live). A single continuous `ffmpeg` process per
/// file reading sequentially from byte 0 has one continuous encoder/
/// timestamp timeline for the whole episode, and its sequential HTTP reads
/// line up naturally with librqbit's own sequential piece-priority
/// download strategy instead of fighting it with scattered probe reads.
#[derive(Clone)]
struct HlsJobs {
    jobs: Arc<AsyncMutex<HashMap<(TorrentId, usize), Arc<AsyncMutex<Option<HlsJob>>>>>>,
    cache_root: PathBuf,
    decoder_support: Arc<std::sync::RwLock<DecoderSupport>>,
    /// Video plan decided once per file per process (see `plan_for`).
    plans: Arc<AsyncMutex<HashMap<(TorrentId, usize), VideoPlan>>>,
}

impl HlsJobs {
    fn new(cache_root: PathBuf) -> Self {
        Self { jobs: Arc::new(AsyncMutex::new(HashMap::new())), cache_root, decoder_support: Arc::new(std::sync::RwLock::new(DecoderSupport::default())), plans: Arc::new(AsyncMutex::new(HashMap::new())) }
    }

    fn job_dir(&self, torrent_id: &TorrentId, file_idx: usize) -> PathBuf {
        self.cache_root.join(format!("{torrent_id}_{file_idx}"))
    }

    fn segment_path(&self, torrent_id: &TorrentId, file_idx: usize, segment_index: usize) -> PathBuf {
        self.job_dir(torrent_id, file_idx).join(format!("{segment_index}.ts"))
    }

    async fn key_lock(&self, torrent_id: &TorrentId, file_idx: usize) -> Arc<AsyncMutex<Option<HlsJob>>> {
        let mut jobs = self.jobs.lock().await;
        jobs.entry((torrent_id.clone(), file_idx)).or_insert_with(|| Arc::new(AsyncMutex::new(None))).clone()
    }

    /// How far into the file playback can seek and get an instant response
    /// (backed by an already-produced segment file), as a timestamp in
    /// seconds - not to be confused with raw torrent download progress.
    /// Those two track different things and can diverge a lot: a byte
    /// range can be fully downloaded while its segment still needs ffmpeg
    /// to actually process it, and a restart-at-a-seek-target can leave
    /// large already-transcoded stretches behind at a lower index than the
    /// current job's own start_index (still instantly seekable, just not
    /// "in progress" by the sense HlsJob::start_index tracks) - so this
    /// deliberately scans for the highest segment file across the whole
    /// directory, unlike the restart-decision logic elsewhere in this
    /// impl, which only looks at the *current* job's own progress.
    fn ready_ranges(&self, torrent_id: &TorrentId, file_idx: usize) -> Vec<(f64, f64)> {
        let dir = self.job_dir(torrent_id, file_idx);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut indices: Vec<usize> = entries
            .flatten()
            .filter_map(|entry| {
                entry.file_name().to_str().and_then(|name| name.strip_suffix(".ts")).and_then(|stem| stem.parse::<usize>().ok())
            })
            .collect();
        indices.sort_unstable();
        // Contiguous runs of segment indices -> [start, end) in seconds.
        // Seek-restarts leave several disjoint runs; reporting them all
        // (rather than just the highest index, as this used to) keeps the
        // seek bar from claiming a gap is instantly seekable.
        let mut ranges: Vec<(f64, f64)> = Vec::new();
        let mut run: Option<(usize, usize)> = None;
        for index in indices {
            run = match run {
                Some((start, end)) if index == end + 1 => Some((start, index)),
                Some((start, end)) => {
                    ranges.push((start as f64 * SEGMENT_DURATION_SECONDS, (end + 1) as f64 * SEGMENT_DURATION_SECONDS));
                    Some((index, index))
                }
                None => Some((index, index)),
            };
        }
        if let Some((start, end)) = run {
            ranges.push((start as f64 * SEGMENT_DURATION_SECONDS, (end + 1) as f64 * SEGMENT_DURATION_SECONDS));
        }
        ranges
    }

    /// Ensures a background transcode job is producing (or has already
    /// produced) the segment at `segment_index`, restarting it at that
    /// offset first if needed, then waits for the resulting file to land
    /// on disk.
    async fn ensure_segment_available(
        &self,
        torrent_id: &TorrentId,
        file_idx: usize,
        segment_index: usize,
        stream_addr: SocketAddr,
        probes: &MediaProbes,
    ) -> anyhow::Result<PathBuf> {
        // Decided (and the on-disk cache validated against it) before any
        // cached segment is served - see plan_for.
        let plan = self.plan_for(torrent_id, file_idx, stream_addr, probes).await;
        let path = self.segment_path(torrent_id, file_idx, segment_index);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }

        let job_lock = self.key_lock(torrent_id, file_idx).await;
        {
            let mut job_slot = job_lock.lock().await;
            let job_exited = job_slot.as_ref().is_some_and(|job| job.child.is_ended());
            let needs_restart = match job_slot.as_mut() {
                None => true,
                // A job whose ffmpeg already exited (read error, crash)
                // will never produce anything again. Without this check
                // the next request fell into the "will reach soon" path,
                // waited, bailed with 503, and hls.js retried the same
                // segment forever - playback stalled permanently at the
                // point the transcode died (verified live at ~3:18).
                Some(_) if job_exited => {
                    tracing::warn!(torrent_id = %torrent_id, file_idx, segment_index, "hls transcode job had exited, restarting");
                    true
                }
                // Started under a provisional plan that turned out wrong
                // (e.g. copying what the probe now says is HEVC).
                Some(job) if job.video_mode != plan.label() => {
                    tracing::info!(torrent_id = %torrent_id, file_idx, running = %job.video_mode, planned = %plan.label(), "hls job video plan changed, restarting");
                    true
                }
                Some(job) => {
                    // Only counts files at or after this job's own
                    // start_index - the directory isn't cleared on
                    // restart, so lower-numbered segments from an earlier,
                    // already-superseded run can still be sitting there.
                    // Counting those as "progress" made a request for a
                    // fresh restart's own (not-yet-produced) target look
                    // like it was still far ahead of "current progress",
                    // triggering another pointless restart to the exact
                    // same offset - verified live: a single seek restarted
                    // the job 5+ times in a row before it was ever left
                    // alone long enough to produce a single segment.
                    let highest_existing = highest_existing_segment_from(&job.dir, job.start_index).await;
                    // Below the current job's start: it belongs to an
                    // earlier, already-superseded run and was never
                    // produced (a backward seek into a gap a forward seek
                    // left behind) - always a restart. Ahead of progress:
                    // restart unless the job will get there shortly.
                    segment_index < job.start_index || !job_will_reach_soon(&*job, highest_existing, segment_index)
                }
            };

            if needs_restart {
                if let Some(mut old) = job_slot.take() {
                    old.kill();
                    tracing::info!(torrent_id = %torrent_id, file_idx, "killed hls transcode job to restart at a new offset");
                }
                let dir = self.job_dir(torrent_id, file_idx);
                tokio::fs::create_dir_all(&dir).await?;
                // The probe decides copy vs. transcode, so the first run
                // waits for it - bounded by PLAN_PROBE_WAIT, since ffmpeg
                // needs the same container header anyway. If it isn't back
                // in time the run copies video (what always worked before)
                // and, once the probe lands, attach_subtitles adds the
                // subtitle outputs. An earlier version waited up to 2x30s
                // here and delayed the first frame by ~55s (verified live).
                let probe = probes.peek(torrent_id, file_idx).await;
                let has_subtitles = probe.is_some();
                let subtitles = probe.map(|probe| probe.subtitles).unwrap_or_default();
                tracing::info!(torrent_id = %torrent_id, file_idx, segment_index, video = %plan.label(), "hls run video plan");
                let child = spawn_hls_transcode(torrent_id, file_idx, segment_index, stream_addr, &dir, &subtitles, &plan).await?;
                *job_slot = Some(HlsJob {
                    child,
                    dir: dir.clone(),
                    start_index: segment_index,
                    started_at: tokio::time::Instant::now(),
                    has_subtitles,
                    subtitle_child: None,
                    video_mode: plan.label(),
                });
                if !has_subtitles {
                    let (jobs, probes, torrent_id) = (self.clone(), probes.clone(), torrent_id.clone());
                    tokio::spawn(async move {
                        match probes.get(&torrent_id, file_idx, stream_addr, dir).await {
                            Ok(probe) => jobs.attach_subtitles(&torrent_id, file_idx, &probe.subtitles, stream_addr).await,
                            Err(err) => tracing::warn!(torrent_id = %torrent_id, file_idx, %err, "background media probe failed; no subtitles for this run"),
                        }
                    });
                }
                tracing::info!(torrent_id = %torrent_id, file_idx, segment_index, "started hls transcode job");
            }
        }

        let wait_started = tokio::time::Instant::now();
        let deadline = wait_started + SEGMENT_WAIT_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                // Diagnostic for "seeking is slow" reports: distinguishes a
                // slow torrent download (this job just started, most of
                // the wait was here) from a slow restart/ffmpeg-startup
                // path (this fires almost immediately after the "started
                // hls transcode job" log line above).
                tracing::debug!(
                    torrent_id = %torrent_id,
                    file_idx,
                    segment_index,
                    wait_ms = wait_started.elapsed().as_millis() as u64,
                    "hls segment became available"
                );
                return Ok(path);
            }
            {
                // Bail out early if the job already died rather than
                // waiting out the full timeout for a file it will never
                // produce (e.g. the underlying torrent stream errored).
                let mut job_slot = job_lock.lock().await;
                if let Some(job) = job_slot.as_mut() {
                    if job.child.is_ended() {
                        anyhow::bail!("hls transcode job for segment {segment_index} ended before producing it");
                    }
                }
            }
            tokio::time::sleep(SEGMENT_POLL_INTERVAL).await;
        }
        anyhow::bail!("timed out waiting for hls segment {segment_index} to be generated")
    }

    /// The video plan for `(torrent_id, file_idx)`, decided on first use in
    /// this process: waits (bounded by PLAN_PROBE_WAIT) for the media
    /// probe, then compares the result with the `video_mode` marker in the
    /// file's cache directory. Segments cached by a different mode (e.g.
    /// HEVC stream-copied before transcoding existed, or before the
    /// frontend reported decoder support) would be undecodable, so they're
    /// deleted before anything is served from that directory.
    async fn plan_for(&self, torrent_id: &TorrentId, file_idx: usize, stream_addr: SocketAddr, probes: &MediaProbes) -> VideoPlan {
        let key = (torrent_id.clone(), file_idx);
        let mut plans = self.plans.lock().await;
        if let Some(plan) = plans.get(&key) {
            return plan.clone();
        }
        let dir = self.job_dir(torrent_id, file_idx);
        let probe = match probes.peek(torrent_id, file_idx).await {
            Some(probe) => Some(probe),
            None => tokio::time::timeout(PLAN_PROBE_WAIT, probes.get(torrent_id, file_idx, stream_addr, dir.clone()))
                .await
                .ok()
                .and_then(|result| result.ok()),
        };
        let marker = dir.join("video_mode");
        let previous = tokio::fs::read_to_string(&marker).await.ok();
        let Some(probe) = probe else {
            // No probe yet: use a provisional plan - whatever an earlier
            // session decided for this file (its marker), else copy - and
            // don't cache it or touch the cache, so the next request
            // re-plans once the probe lands. A running job whose mode
            // differs from the real plan is restarted (see
            // ensure_segment_available).
            let provisional = match previous.as_deref() {
                None | Some("direct") => VideoPlan::Copy,
                Some(label) => VideoPlan::Transcode {
                    encoder: detect_h264_encoder().await,
                    reason: label.split(" -> ").next().unwrap_or(label).to_string(),
                },
            };
            tracing::warn!(torrent_id = %torrent_id, file_idx, video = %provisional.label(), "media probe not ready, provisional video plan");
            return provisional;
        };
        let support = *self.decoder_support.read().unwrap_or_else(|e| e.into_inner());
        let plan = plan_video(probe.video.as_ref(), support).await;
        let label = plan.label();
        if previous.as_deref() != Some(label.as_str()) {
            let mut removed = 0usize;
            if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.file_name().to_string_lossy().ends_with(".ts") && tokio::fs::remove_file(entry.path()).await.is_ok() {
                        removed += 1;
                    }
                }
            }
            if let Err(err) = tokio::fs::create_dir_all(&dir).await.and(tokio::fs::write(&marker, &label).await) {
                tracing::warn!(torrent_id = %torrent_id, file_idx, %err, "failed to write video mode marker");
            }
            tracing::info!(torrent_id = %torrent_id, file_idx, previous = ?previous, current = %label, removed, "hls cache video mode changed, dropped stale segments");
        }
        tracing::info!(torrent_id = %torrent_id, file_idx, video = %label, "hls video plan decided");
        plans.insert(key, plan.clone());
        plan
    }

    async fn video_mode(&self, torrent_id: &TorrentId, file_idx: usize) -> Option<String> {
        let job_lock = self.key_lock(torrent_id, file_idx).await;
        let slot = job_lock.lock().await;
        slot.as_ref().map(|job| job.video_mode.clone())
    }

    /// Adds a subtitle-only extraction process to the current run of
    /// `(torrent_id, file_idx)` if that run started without subtitle
    /// outputs (probe not ready yet). Same start offset and `-copyts`
    /// timeline as the run, so its output merges like any other run's.
    async fn attach_subtitles(&self, torrent_id: &TorrentId, file_idx: usize, tracks: &[SubtitleTrack], stream_addr: SocketAddr) {
        if tracks.is_empty() {
            return;
        }
        let job_lock = self.key_lock(torrent_id, file_idx).await;
        let mut job_slot = job_lock.lock().await;
        let Some(job) = job_slot.as_mut() else {
            return;
        };
        if job.has_subtitles {
            return;
        }
        match spawn_subtitle_extraction(torrent_id, file_idx, job.start_index, stream_addr, &job.dir, tracks).await {
            Ok(child) => {
                job.subtitle_child = Some(child);
                job.has_subtitles = true;
                tracing::info!(torrent_id = %torrent_id, file_idx, start_index = job.start_index, tracks = tracks.len(), "attached subtitle extraction to running hls job");
            }
            Err(err) => tracing::error!(torrent_id = %torrent_id, file_idx, %err, "failed to start subtitle extraction"),
        }
    }

    /// Kills and forgets any transcode job for `torrent_id` (any file
    /// index) and deletes its cached segments - called when the torrent
    /// itself is removed so nothing keeps writing into a now-orphaned
    /// cache directory.
    async fn remove_torrent(&self, torrent_id: &TorrentId) {
        self.plans.lock().await.retain(|(id, _), _| id != torrent_id);
        let mut jobs = self.jobs.lock().await;
        let keys: Vec<_> = jobs.keys().filter(|(id, _)| id == torrent_id).cloned().collect();
        for key in keys {
            if let Some(job_lock) = jobs.remove(&key) {
                let mut job_slot = job_lock.lock().await;
                if let Some(mut job) = job_slot.take() {
                    job.kill();
                    let _ = tokio::fs::remove_dir_all(&job.dir).await;
                }
            }
        }
    }
}


/// Whether `job` is expected to produce `target` within
/// `RESTART_WAIT_BUDGET`, from its measured production rate so far.
fn job_will_reach_soon(job: &HlsJob, highest_existing: Option<usize>, target: usize) -> bool {
    let Some(highest) = highest_existing else {
        let soon = target <= job.start_index + RESTART_COLD_LOOKAHEAD_SEGMENTS;
        tracing::debug!(target, start_index = job.start_index, soon, "hls job has no output yet");
        return soon;
    };
    if target <= highest + 1 {
        return true;
    }
    let produced = (highest + 1 - job.start_index) as f64;
    let elapsed = job.started_at.elapsed().as_secs_f64().max(0.001);
    let rate = produced / elapsed;
    let expected_wait = (target - highest) as f64 / rate;
    let soon = expected_wait <= RESTART_WAIT_BUDGET.as_secs_f64();
    tracing::debug!(target, highest, rate_segments_per_sec = rate, expected_wait, soon, "hls restart decision");
    soon
}

/// Highest `.ts` segment index present in `dir` at or after `min_index` -
/// callers pass the current job's own `start_index` so a lower-numbered
/// leftover from an earlier, already-superseded job run (the directory is
/// never cleared on restart) can't be mistaken for that job's own progress.
async fn highest_existing_segment_from(dir: &FsPath, min_index: usize) -> Option<usize> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    let mut highest = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Some(index) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".ts"))
            .and_then(|stem| stem.parse::<usize>().ok())
        {
            if index >= min_index {
                highest = Some(highest.map_or(index, |h: usize| h.max(index)));
            }
        }
    }
    highest
}

/// Media-layer subtitle stream list: ASS/SSA sources are stream-copied,
/// others converted (see `media::SubtitleStream`).
fn subtitle_streams(tracks: &[SubtitleTrack]) -> Vec<media::SubtitleStream> {
    tracks.iter().map(|t| media::SubtitleStream { index: t.index, copy: matches!(t.codec.as_str(), "ass" | "ssa") }).collect()
}

/// Starts the single continuous in-process HLS run for a torrent file at
/// `start_segment_index` (0 for normal playback, non-zero only for a seek
/// restart) - see `media::start_hls_run` for the pipeline itself. Reads
/// from `stream_handler` over loopback, reusing its Range-aware,
/// piece-priority-aware reads; segments land as real files (`temp_file`
/// renames each into place only once complete).
async fn spawn_hls_transcode(
    torrent_id: &TorrentId,
    file_idx: usize,
    start_segment_index: usize,
    stream_addr: SocketAddr,
    dir: &FsPath,
    subtitles: &[SubtitleTrack],
    plan: &VideoPlan,
) -> anyhow::Result<media::MediaJob> {
    let input_url = format!("http://{stream_addr}/stream/{torrent_id}/{file_idx}");
    let dir = dir.to_path_buf();
    let subtitle_streams = subtitle_streams(subtitles);
    let video = match plan {
        VideoPlan::Copy => media::VideoCodec::Copy,
        VideoPlan::Transcode { encoder, .. } => media::VideoCodec::Transcode(*encoder),
    };
    tracing::debug!(torrent_id = %torrent_id, file_idx, start_segment_index, subtitles = subtitle_streams.len(), "starting in-process hls run");
    tokio::task::spawn_blocking(move || {
        media::start_hls_run(&media::HlsRun {
            input_url: &input_url,
            dir: &dir,
            start_segment_index,
            segment_seconds: SEGMENT_DURATION_SECONDS,
            video,
            subtitle_streams: &subtitle_streams,
        })
    })
    .await
    .map_err(|err| anyhow::anyhow!("hls run start panicked: {err}"))?
}

/// Starts one low-priority in-process pass over the *whole* file that
/// extracts every text subtitle track to `sub_<index>_bg.ass`, so subtitles
/// for parts the transcode hasn't reached yet (a forward seek, a skipped
/// OP) are usually already there. Subtitle packets are interleaved with
/// video across every cluster, so this can only complete as the torrent
/// itself finishes downloading - it reads with the Background intent (see
/// `StreamQuery`) so it trails the download rather than pulling piece
/// priority away from the playhead.
async fn spawn_background_subtitle_pass(
    torrent_id: &TorrentId,
    file_idx: usize,
    stream_addr: SocketAddr,
    dir: &FsPath,
    tracks: &[SubtitleTrack],
) -> anyhow::Result<Option<media::MediaJob>> {
    if tracks.is_empty() {
        return Ok(None);
    }
    tokio::fs::create_dir_all(dir).await?;
    let input_url = format!("http://{stream_addr}/stream/{torrent_id}/{file_idx}?intent=background");
    let dir = dir.to_path_buf();
    let streams = subtitle_streams(tracks);
    let job = tokio::task::spawn_blocking(move || media::start_subtitle_run(&input_url, &dir, 0.0, &streams, "bg"))
        .await
        .map_err(|err| anyhow::anyhow!("background subtitle pass start panicked: {err}"))??;
    tracing::info!(torrent_id = %torrent_id, file_idx, tracks = tracks.len(), "started background full-file subtitle pass");
    Ok(Some(job))
}

/// Subtitle-only counterpart of the HLS run's ASS outputs, for a run that
/// started before the media probe finished (see
/// `HlsJobs::attach_subtitles`). Same start offset, same `-copyts`
/// timeline and `sub_<index>_<start>.ass` naming, so `subtitle_handler`
/// merges it like any other run.
async fn spawn_subtitle_extraction(
    torrent_id: &TorrentId,
    file_idx: usize,
    start_segment_index: usize,
    stream_addr: SocketAddr,
    dir: &FsPath,
    tracks: &[SubtitleTrack],
) -> anyhow::Result<media::MediaJob> {
    let input_url = format!("http://{stream_addr}/stream/{torrent_id}/{file_idx}");
    let start_seconds = start_segment_index as f64 * SEGMENT_DURATION_SECONDS;
    let dir = dir.to_path_buf();
    let streams = subtitle_streams(tracks);
    tracing::debug!(torrent_id = %torrent_id, file_idx, start_segment_index, "starting subtitle catch-up run");
    tokio::task::spawn_blocking(move || media::start_subtitle_run(&input_url, &dir, start_seconds, &streams, &start_segment_index.to_string()))
        .await
        .map_err(|err| anyhow::anyhow!("subtitle run start panicked: {err}"))?
}

/// Subtitle codecs ffmpeg can convert to ASS text. Everything else a real
/// release carries (PGS/VobSub/DVB - bitmap formats) can't become ASS
/// without OCR, and asking ffmpeg to try fails the *whole* process it's
/// part of - which, since subtitle extraction now runs inside the HLS
/// transcode (see `spawn_hls_transcode`), would take video down with it.
const TEXT_SUBTITLE_CODECS: &[&str] = &["ass", "ssa", "subrip", "srt", "webvtt", "mov_text", "text"];

/// One embedded, text-based subtitle track discovered in a torrent file, as
/// reported by `ffprobe` - `index` is the absolute demuxer stream index,
/// which doubles as the exact `-map 0:<index>` argument
/// `spawn_hls_transcode` uses and as the URL segment `subtitle_handler`
/// serves it under, so it's never renumbered anywhere along the way.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub index: usize,
    pub language: Option<String>,
    pub title: Option<String>,
    /// Source codec as ffprobe names it (`ass`, `subrip`, ...). Everything
    /// is served as ASS regardless; this only tells the frontend whether
    /// the track carries its own real styling (`ass`/`ssa`) or is plain
    /// text ffmpeg gave a generic `Default` style, which is what the
    /// user's default-subtitle-style setting is allowed to restyle.
    pub codec: String,
    /// Matroska's per-track default flag - fansub releases set it on the
    /// main dialogue track, which is a better auto-pick than "first
    /// English one" when a release carries several.
    pub default: bool,
}

/// One embedded font attachment (Matroska `Attachments`), needed for
/// typeset ASS signs/dialogue to render with the fonts the fansubber
/// actually used rather than a generic fallback.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontAttachment {
    pub index: usize,
    pub filename: String,
}

/// What `ffprobe` found in a torrent file's container header - probed once
/// per file and cached (see `MediaProbes`).
#[derive(Debug, Clone, Default)]
pub struct MediaProbe {
    pub subtitles: Vec<SubtitleTrack>,
    pub fonts: Vec<FontAttachment>,
    pub video: Option<VideoStreamInfo>,
}

/// The main video stream's codec/pixel format - what `plan_video` decides
/// copy vs. transcode from.
#[derive(Debug, Clone)]
pub struct VideoStreamInfo {
    pub codec: String,
    pub pix_fmt: Option<String>,
}

fn is_font_attachment(codec: Option<&str>, mimetype: Option<&str>, filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    matches!(codec, Some("ttf" | "otf"))
        || mimetype.is_some_and(|m| m.contains("font") || m.contains("truetype") || m.contains("opentype"))
        || lower.ends_with(".ttf")
        || lower.ends_with(".otf")
        || lower.ends_with(".ttc")
}

/// Reads a torrent file's stream list in-process (ffmpeg-next, header-level
/// probe limits) over the same loopback `stream_handler` URL the transcode
/// uses, and writes font attachments straight into `fonts_dir` from the
/// container header - replacing both the `ffprobe` call and the separate
/// `ffmpeg -dump_attachment` pass. Blocks until the header has downloaded;
/// callers bound it with `PROBE_TIMEOUT`.
async fn probe_media(stream_addr: SocketAddr, torrent_id: &TorrentId, file_idx: usize, fonts_dir: PathBuf) -> anyhow::Result<MediaProbe> {
    tracing::debug!(torrent_id = %torrent_id, file_idx, "probing media streams");
    let input_url = format!("http://{stream_addr}/stream/{torrent_id}/{file_idx}");
    let streams = tokio::task::spawn_blocking(move || media::probe_streams(&input_url))
        .await
        .map_err(|err| anyhow::anyhow!("probe panicked: {err}"))??;

    let mut probe = MediaProbe::default();
    let mut font_bytes = Vec::new();
    for stream in streams {
        let mut tags = stream.tags;
        match stream.kind {
            ffmpeg_next::media::Type::Subtitle => {
                if !TEXT_SUBTITLE_CODECS.contains(&stream.codec.as_str()) {
                    tracing::info!(torrent_id = %torrent_id, file_idx, index = stream.index, codec = %stream.codec, "skipping non-text subtitle track");
                    continue;
                }
                probe.subtitles.push(SubtitleTrack {
                    index: stream.index,
                    language: tags.remove("language"),
                    title: tags.remove("title"),
                    codec: stream.codec,
                    default: stream.default,
                });
            }
            // First real video stream - cover art attachments also show up
            // as video (mjpeg/png with the attached_pic disposition).
            ffmpeg_next::media::Type::Video if probe.video.is_none() && !matches!(stream.codec.as_str(), "mjpeg" | "png" | "bmp") => {
                probe.video = Some(VideoStreamInfo { codec: stream.codec, pix_fmt: stream.pix_fmt });
            }
            ffmpeg_next::media::Type::Attachment => {
                let filename = tags.remove("filename").unwrap_or_default();
                let mimetype = tags.remove("mimetype");
                if is_font_attachment(Some(stream.codec.as_str()), mimetype.as_deref(), &filename) {
                    if let Some(bytes) = stream.attachment {
                        font_bytes.push((stream.index, bytes));
                    }
                    probe.fonts.push(FontAttachment { index: stream.index, filename });
                }
            }
            _ => {}
        }
    }
    write_fonts(torrent_id, file_idx, &fonts_dir, font_bytes).await;
    tracing::info!(
        torrent_id = %torrent_id,
        file_idx,
        subtitle_count = probe.subtitles.len(),
        font_count = probe.fonts.len(),
        video = ?probe.video.as_ref().map(|v| (&v.codec, &v.pix_fmt)),
        "media probe succeeded"
    );
    Ok(probe)
}

/// Writes font attachments to `<fonts_dir>/<stream_index>` (under a `.part`
/// name, renamed once complete so `font_handler` never serves a
/// half-written font).
async fn write_fonts(torrent_id: &TorrentId, file_idx: usize, fonts_dir: &FsPath, fonts: Vec<(usize, Vec<u8>)>) {
    if fonts.is_empty() {
        return;
    }
    if let Err(err) = tokio::fs::create_dir_all(fonts_dir).await {
        tracing::error!(torrent_id = %torrent_id, file_idx, %err, "failed to create font directory");
        return;
    }
    let expected = fonts.len();
    let mut written = 0;
    for (index, bytes) in fonts {
        let part = fonts_dir.join(format!("{index}.part"));
        if tokio::fs::write(&part, &bytes).await.is_ok() && tokio::fs::rename(&part, fonts_dir.join(index.to_string())).await.is_ok() {
            written += 1;
        }
    }
    tracing::info!(torrent_id = %torrent_id, file_idx, written, expected, "font attachments written");
}

/// Per-file cache of `probe_media` results. Only successes are cached
/// (`OnceCell::get_or_try_init`), so a probe that ran before the header had
/// downloaded (timed out) is simply retried by the next caller. The first
/// success also kicks off a one-shot font-attachment dump into the file's
/// HLS job directory (`fonts/<stream_index>`), served by `font_handler`.
#[derive(Clone)]
struct MediaProbes {
    cells: Arc<AsyncMutex<HashMap<(TorrentId, usize), Arc<tokio::sync::OnceCell<MediaProbe>>>>>,
    /// One full-file subtitle pass per file (see
    /// `spawn_background_subtitle_pass`), killed with the torrent.
    background_passes: Arc<AsyncMutex<HashMap<(TorrentId, usize), media::MediaJob>>>,
}

impl MediaProbes {
    fn new() -> Self {
        Self { cells: Arc::new(AsyncMutex::new(HashMap::new())), background_passes: Arc::new(AsyncMutex::new(HashMap::new())) }
    }

    async fn get(&self, torrent_id: &TorrentId, file_idx: usize, stream_addr: SocketAddr, job_dir: PathBuf) -> anyhow::Result<MediaProbe> {
        let cell = {
            let mut cells = self.cells.lock().await;
            cells.entry((torrent_id.clone(), file_idx)).or_insert_with(|| Arc::new(tokio::sync::OnceCell::new())).clone()
        };
        let probe = cell
            .get_or_try_init(|| async {
                let probe = tokio::time::timeout(PROBE_TIMEOUT, probe_media(stream_addr, torrent_id, file_idx, job_dir.join("fonts")))
                    .await
                    .map_err(|_| anyhow::anyhow!("timed out probing media streams"))??;
                match spawn_background_subtitle_pass(torrent_id, file_idx, stream_addr, &job_dir, &probe.subtitles).await {
                    Ok(Some(child)) => {
                        self.background_passes.lock().await.insert((torrent_id.clone(), file_idx), child);
                    }
                    Ok(None) => {}
                    Err(err) => tracing::error!(torrent_id = %torrent_id, file_idx, %err, "failed to start background subtitle pass"),
                }
                Ok::<_, anyhow::Error>(probe)
            })
            .await?;
        Ok(probe.clone())
    }

    /// The cached probe if one has already succeeded - never waits.
    async fn peek(&self, torrent_id: &TorrentId, file_idx: usize) -> Option<MediaProbe> {
        let cells = self.cells.lock().await;
        cells.get(&(torrent_id.clone(), file_idx)).and_then(|cell| cell.get().cloned())
    }

    async fn remove_torrent(&self, torrent_id: &TorrentId) {
        self.cells.lock().await.retain(|(id, _), _| id != torrent_id);
        // Dropping a MediaJob aborts it.
        self.background_passes.lock().await.retain(|(id, _), _| id != torrent_id);
    }
}


/// Merges every per-run ASS file the HLS transcode has written for one
/// subtitle track (`sub_<stream_index>_<start_segment>.ass` - one per
/// transcode run, since a seek restarts the job at a new offset) into a
/// single script: header from the earliest run, then the union of all
/// `Dialogue:` lines, de-duplicated. Every run shares one timeline thanks
/// to `-copyts`, so overlapping runs produce byte-identical event lines.
/// The last line of a file still being written may be partial, so only
/// newline-terminated lines are taken.
fn merge_ass_runs(mut runs: Vec<(usize, String)>) -> Option<String> {
    runs.sort_by_key(|(start, _)| *start);
    let mut header: Option<String> = None;
    let mut seen = std::collections::HashSet::new();
    let mut events = Vec::new();
    for (_, content) in &runs {
        let complete = match content.rfind('\n') {
            Some(end) => &content[..=end],
            None => continue,
        };
        if header.is_none() {
            if let Some(events_pos) = complete.find("[Events]") {
                // Header runs through the `Format:` line that follows
                // `[Events]`.
                let after = &complete[events_pos..];
                if let Some(format_pos) = after.find("Format:") {
                    if let Some(eol) = after[format_pos..].find('\n') {
                        header = Some(complete[..events_pos + format_pos + eol + 1].to_string());
                    }
                }
            }
        }
        for line in complete.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with("Dialogue:") && seen.insert(line.to_string()) {
                events.push(line.to_string());
            }
        }
    }
    let mut merged = header?;
    for event in events {
        merged.push_str(&event);
        merged.push('\n');
    }
    Some(merged)
}

pub struct AddedTorrent {
    pub id: TorrentId,
}

#[derive(Clone)]
struct StreamRouterState {
    efs: Arc<EngineFS>,
    stream_addr: SocketAddr,
    hls_jobs: HlsJobs,
    probes: MediaProbes,
}

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
    /// How far into the file (in seconds) HLS segments have actually been
    /// produced and are ready for an instant seek - see
    /// `HlsJobs::ready_ranges`' doc comment for why this isn't the same
    /// thing as `progress_percent`. Frontend seek-bar highlight should use
    /// this, not `progress_percent`, for "can I seek here instantly".
    pub ready_seconds: f64,
    /// Every stretch (`[start, end)` seconds) with produced segments - the
    /// seek bar draws these; `ready_seconds` is the end of the stretch
    /// starting at 0, kept for callers that only want one number.
    pub ready_ranges: Vec<(f64, f64)>,
    /// How the current run handles video: "direct" (stream copy) or e.g.
    /// "HEVC -> H.264 (h264_nvenc)". None before the first segment request.
    pub video_mode: Option<String>,
}

impl TorrentEngine {
    /// Starts an `enginefs` libtorrent-backend engine rooted at
    /// `download_dir` and a local HTTP server that serves torrent file
    /// bytes with Range support, mirroring Stremio's local streaming
    /// server. Disk-backed (not `enginefs`'s memory-only mode) so partial
    /// downloads survive an app restart, matching this crate's previous
    /// librqbit-based behavior - see PLAN.md's streaming-server section.
    pub async fn start(download_dir: PathBuf) -> anyhow::Result<Self> {
        tracing::debug!(?download_dir, "starting enginefs libtorrent backend");
        // Sibling of `download_dir` (itself `<cache_dir>/nyaa-stream/downloads`)
        // rather than nested under it, so a blanket wipe of one doesn't
        // have to know about the other's existence. Computed before
        // `download_dir` moves into `LibtorrentBackend::new_disk_backed` below.
        let hls_cache_root = download_dir
            .parent()
            .map(|parent| parent.join("hls_cache"))
            .unwrap_or_else(|| download_dir.join("hls_cache"));
        let hls_jobs = HlsJobs::new(hls_cache_root);
        // Warm the encoder probe so the first transcode doesn't pay for it.
        tokio::spawn(detect_h264_encoder());
        let probes = MediaProbes::new();
        let cache_dir = download_dir
            .parent()
            .map(|parent| parent.join("engine_cache"))
            .unwrap_or_else(|| download_dir.join("engine_cache"));

        let config = BackendConfig::default();
        let backend = enginefs::backend::libtorrent::LibtorrentBackend::new_disk_backed(download_dir.clone(), config)?;
        let efs: Arc<EngineFS> = Arc::new(EngineFS::new_with_backend(backend, HashMap::new(), cache_dir, download_dir));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let stream_addr = listener.local_addr()?;
        tracing::info!(%stream_addr, "streaming server listening");

        let router_state =
            StreamRouterState { efs: efs.clone(), stream_addr, hls_jobs: hls_jobs.clone(), probes: probes.clone() };
        let app = Router::new()
            .route("/stream/{torrent_id}/{file_idx}", get(stream_handler))
            .route("/hls/{torrent_id}/{file_idx}/playlist.m3u8", get(hls_playlist_handler))
            // No `.ts` suffix here: axum's router rejects a literal suffix
            // mixed with a param in the same path segment ("Only one
            // parameter is allowed per path segment") - verified live, this
            // panics at startup, not just a lint. The extension is only a
            // convention anyway; Content-Type is set explicitly below and
            // hls.js doesn't care what the segment URI looks like.
            .route("/hls/{torrent_id}/{file_idx}/{segment_index}", get(hls_segment_handler))
            .route("/subtitles/{torrent_id}/{file_idx}/{stream_index}", get(subtitle_handler))
            .route("/fonts/{torrent_id}/{file_idx}/{stream_index}", get(font_handler))
            .with_state(router_state)
            // hls.js loads the playlist/segments via fetch/XHR rather than
            // a plain <video src> element, which - unlike media-element
            // loading - is subject to CORS. Verified live: hls.js reported
            // a "manifestLoadError" even though the same URL loaded fine
            // via curl, since the app's origin (localhost:1420/tauri://...)
            // differs from this local server's (127.0.0.1:<port>) and we
            // sent no CORS headers at all.
            .layer(tower_http::cors::CorsLayer::permissive());
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!(?err, "streaming server stopped");
            }
        });

        Ok(Self { efs, stream_addr, hls_jobs, probes })
    }

    /// Adds a torrent from a magnet link or .torrent URL and starts
    /// downloading it. `enginefs`'s libtorrent backend prioritizes pieces
    /// for whichever file is actually being streamed once a stream is
    /// opened on it (its `LibtorrentPlaybackCoordinator`, driven by
    /// `refresh_hls_playback` in `stream_handler`/`hls_segment_handler`
    /// below), so no extra "sequential mode" flag is needed here.
    pub async fn add(&self, magnet_or_url: &str) -> anyhow::Result<AddedTorrent> {
        tracing::debug!("adding torrent");
        let engine = self.efs.add_torrent(TorrentSource::Url(magnet_or_url.to_string()), None).await.map_err(|err| {
            tracing::error!(%err, "failed to add torrent");
            err
        })?;
        let id = engine.info_hash.clone();
        tracing::info!(torrent_id = %id, "torrent added");
        Ok(AddedTorrent { id })
    }

    /// Removes a torrent from the engine and deletes our own HLS transcode
    /// cache for it. Used to clean up after a scratch download that only
    /// existed to pull a thumbnail frame out of the first few seconds of an
    /// episode - unlike a torrent the user actually chose to watch, there's
    /// no reason to keep seeding or keep the partial file around afterward.
    ///
    /// Note: unlike this crate's previous librqbit-based `remove`,
    /// `enginefs`'s libtorrent backend's own `remove_torrent` does not
    /// delete the torrent's downloaded files from disk (it calls
    /// libtorrent's `remove_torrent` with `delete_files = false` -
    /// verified in `enginefs`'s vendored source) - only our own HLS cache
    /// is guaranteed cleaned up here.
    pub async fn remove(&self, id: TorrentId) -> anyhow::Result<()> {
        tracing::debug!(torrent_id = %id, "removing torrent");
        self.hls_jobs.remove_torrent(&id).await;
        self.probes.remove_torrent(&id).await;
        self.efs.remove_engine(&id).await;
        self.efs.get_backend().remove_torrent(&id).await
    }

    /// Waits (up to `METADATA_TIMEOUT`) for `id`'s metadata and returns its
    /// file list. A magnet link carries no file list, so nothing about
    /// which file to stream can be decided before this resolves.
    pub async fn files(&self, id: &TorrentId) -> anyhow::Result<Vec<TorrentFile>> {
        let engine = self.efs.get_engine(id).await.ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let started = tokio::time::Instant::now();
        loop {
            let files = engine.handle.get_files().await;
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
    /// start before the whole torrent has downloaded. Raw container bytes -
    /// use `hls_playlist_url` for browser playback (see `hls_playlist_handler`'s
    /// doc comment for why).
    pub fn stream_url(&self, torrent_id: &TorrentId, file_idx: usize) -> String {
        format!("http://{}/stream/{}/{}", self.stream_addr, torrent_id, file_idx)
    }

    /// URL for an HLS playlist covering a specific file within an
    /// already-added torrent - see `hls_playlist_handler`'s doc comment for
    /// why this is what the frontend's `<video>` element (via hls.js)
    /// should actually point at instead of `stream_url`. Requires a
    /// `?duration=<seconds>` query param (the frontend supplies its own
    /// estimate) to know how many segments to declare.
    pub fn hls_playlist_url(&self, torrent_id: &TorrentId, file_idx: usize) -> String {
        format!("http://{}/hls/{}/{}/playlist.m3u8", self.stream_addr, torrent_id, file_idx)
    }

    /// Probes (or returns the cached probe of) a torrent file's embedded
    /// text subtitle tracks and font attachments. Extraction itself isn't
    /// started here - it runs inside the HLS transcode job (see
    /// `spawn_hls_transcode`). Errors (header not downloaded yet within
    /// `PROBE_TIMEOUT`) aren't cached, so the frontend can just retry.
    pub async fn media_probe(&self, torrent_id: &TorrentId, file_idx: usize) -> anyhow::Result<MediaProbe> {
        self.probes.get(torrent_id, file_idx, self.stream_addr, self.hls_jobs.job_dir(torrent_id, file_idx)).await
    }

    /// Records what the frontend's WebView decodes natively (see
    /// `DecoderSupport`), used by every later transcode-run decision.
    pub fn set_decoder_support(&self, support: DecoderSupport) {
        tracing::info!(?support, "decoder support reported");
        *self.hls_jobs.decoder_support.write().unwrap_or_else(|e| e.into_inner()) = support;
    }

    /// URL a font attachment (see `FontAttachment`) is served from.
    pub fn font_url(&self, torrent_id: &TorrentId, file_idx: usize, stream_index: usize) -> String {
        format!("http://{}/fonts/{}/{}/{}", self.stream_addr, torrent_id, file_idx, stream_index)
    }

    /// URL serving a subtitle track as one merged ASS script, growing as
    /// the transcode job progresses - the frontend polls it (see
    /// `subtitle_handler`). `stream_index` is `SubtitleTrack::index`.
    pub fn subtitle_url(&self, torrent_id: &TorrentId, file_idx: usize, stream_index: usize) -> String {
        format!("http://{}/subtitles/{}/{}/{}", self.stream_addr, torrent_id, file_idx, stream_index)
    }

    /// Download progress/speed/peer-count snapshot for an in-progress
    /// torrent, polled by the frontend to show buffering feedback while
    /// mpv waits for enough data to start decoding.
    pub async fn stats(&self, id: &TorrentId, file_idx: usize) -> anyhow::Result<StreamStats> {
        let engine = self.efs.get_engine(id).await.ok_or_else(|| anyhow::anyhow!("unknown torrent {id}"))?;
        let stats = engine.get_statistics().await;

        // The streamed file's own progress, not the whole torrent's - for
        // a batch those differ by an order of magnitude. Falls back to the
        // torrent total if the index is somehow out of range.
        let (total_bytes, downloaded_bytes) = match stats.files.get(file_idx) {
            Some(file) => (file.length, file.downloaded),
            None => (stats.files.iter().map(|f| f.length).sum(), stats.files.iter().map(|f| f.downloaded).sum()),
        };
        let progress_percent = if total_bytes > 0 { downloaded_bytes as f64 / total_bytes as f64 * 100.0 } else { 0.0 };
        // `enginefs::backend::EngineStats::download_speed` is bytes/sec,
        // unlike librqbit's `Speed::mbps` (MiB/s) this field previously
        // mirrored - converted here so `StreamStats`'s documented unit
        // (MiB/s, consumed by the frontend) doesn't silently change.
        let download_speed_mbps = stats.download_speed / (1024.0 * 1024.0);
        let ready_ranges = self.hls_jobs.ready_ranges(id, file_idx);
        let state = if !stats.has_metadata {
            "initializing"
        } else if stats.swarm_paused {
            "paused"
        } else {
            "live"
        };

        Ok(StreamStats {
            state: state.to_string(),
            progress_percent,
            download_speed_mbps,
            connected_peers: stats.peers as u32,
            finished: stats.is_finished,
            downloaded_bytes,
            total_bytes,
            ready_seconds: ready_ranges.first().filter(|(start, _)| *start == 0.0).map_or(0.0, |(_, end)| *end),
            ready_ranges,
            video_mode: self.hls_jobs.video_mode(id, file_idx).await,
        })
    }
}

/// Browsers refuse to play `<video src>` at all without a `Content-Type`
/// they recognize as a media type, even when the underlying codec would
/// otherwise decode fine - verified live: omitting this entirely produced
/// an immediate `MEDIA_ERR_SRC_NOT_SUPPORTED` in the frontend's `<video>`
/// regardless of the actual file. Mapped from the file's own extension
/// since librqbit doesn't otherwise expose a MIME type.
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

#[derive(Deserialize)]
struct StreamQuery {
    /// `background` marks a reader that must never compete with playback
    /// for bandwidth (the full-file subtitle pass, see
    /// `spawn_background_subtitle_pass`): no playback-lease refresh, and
    /// `enginefs`'s Background intent (libtorrent piece priority 1).
    intent: Option<String>,
}

async fn stream_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx)): Path<(TorrentId, usize)>,
    Query(query): Query<StreamQuery>,
    range: Option<TypedHeader<Range>>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    let background = query.intent.as_deref() == Some("background");
    tracing::debug!(torrent_id = %torrent_id, file_idx, background, "stream request received");
    let engine = state.efs.get_engine(&torrent_id).await.ok_or_else(|| {
        tracing::warn!(torrent_id = %torrent_id, file_idx, "stream request for unknown torrent");
        axum::http::StatusCode::NOT_FOUND
    })?;

    // Refreshes this file's HLS playback lease (resumes the torrent if
    // paused, reprioritizes pieces toward it) - a no-op-cheap early return
    // on the common case of a repeat request for the file already being
    // watched. See PLAN.md's streaming-server section / `playback.rs`'s
    // `LibtorrentPlaybackCoordinator` for why this needs to be called on
    // every request rather than once when the stream starts.
    if !background {
        state.efs.refresh_hls_playback(&torrent_id, file_idx, "raw-stream").await;
    }

    // priority 128: a normal foreground direct-playback read (not the
    // internal-probe/background sentinels `Engine::get_file` treats 255/0
    // as - see `enginefs::engine::Engine::get_file`'s doc comment).
    let file_handle = engine.get_file(file_idx, 0, if background { 0 } else { 128 }).await.ok_or_else(|| {
        tracing::warn!(torrent_id = %torrent_id, file_idx, "stream request for unknown file index");
        axum::http::StatusCode::NOT_FOUND
    })?;
    let byte_size = file_handle.size;
    let body = KnownSize::sized(file_handle, byte_size);
    let range = range.map(|TypedHeader(range)| range);

    let content_type = engine
        .handle
        .get_files()
        .await
        .into_iter()
        .nth(file_idx)
        .map(|file| mime_for_filename(&file.name))
        .unwrap_or("application/octet-stream");

    Ok(([(axum::http::header::CONTENT_TYPE, content_type)], Ranged::new(range, body)))
}

#[derive(Deserialize)]
struct PlaylistQuery {
    /// Total duration in seconds, used to compute how many segments to
    /// declare. The backend can't reliably determine this itself for a
    /// still-downloading torrent (many real releases don't declare
    /// duration until their Matroska Cues near the *end* of the file,
    /// which isn't available yet) - the frontend supplies its own estimate
    /// instead (AniList's per-episode runtime). Falls back to a generic
    /// guess if the frontend has none.
    duration: Option<f64>,
}

/// A show's typical episode length, used only when the frontend has no
/// AniList-derived estimate at all - better than refusing to build a
/// playlist, and self-corrects next time the page has real metadata.
const DEFAULT_DURATION_SECONDS: f64 = 24.0 * 60.0;

/// Serves an HLS VOD playlist for a torrent's file, built from a
/// frontend-supplied duration estimate (see `PlaylistQuery`) rather than
/// anything read from the file itself.
///
/// This - HLS via `hls.js`, replacing an earlier "remux the whole episode
/// through ffmpeg on every seek" approach - is necessary, not cosmetic:
/// verified live that a real anime release (MKV, H.264 video, E-AC-3
/// audio) fails in WebView2's `<video>` with `MEDIA_ERR_SRC_NOT_SUPPORTED`
/// even though `canPlayType` reports "probably" for every codec involved -
/// the actual blocker is that Matroska's seek index (Cues/SeekHead) is
/// commonly placed at/near the *end* of the file, which an
/// incrementally-downloading torrent can't provide up front. Segments
/// themselves are produced by `HlsJobs` (see its doc comment) rather than
/// this handler, which only ever needs to know the total segment count.
async fn hls_playlist_handler(
    Path((_torrent_id, _file_idx)): Path<(TorrentId, usize)>,
    Query(query): Query<PlaylistQuery>,
) -> impl axum::response::IntoResponse {
    let total_duration = query.duration.filter(|d| *d > 0.0).unwrap_or(DEFAULT_DURATION_SECONDS);
    let segment_count = (total_duration / SEGMENT_DURATION_SECONDS).ceil().max(1.0) as usize;

    let mut playlist = String::new();
    playlist.push_str("#EXTM3U\n");
    playlist.push_str("#EXT-X-VERSION:3\n");
    playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", SEGMENT_DURATION_SECONDS.ceil() as u64));
    playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");

    let mut remaining = total_duration;
    for index in 0..segment_count {
        let segment_len = remaining.min(SEGMENT_DURATION_SECONDS);
        playlist.push_str(&format!("#EXTINF:{segment_len:.3},\n{index}\n"));
        remaining -= segment_len;
    }
    playlist.push_str("#EXT-X-ENDLIST\n");

    ([(axum::http::header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], playlist)
}

/// Serves one HLS segment from the on-disk cache produced by the
/// per-file `ffmpeg` transcode job (`HlsJobs`), starting or restarting
/// that job first if needed. See `HlsJobs`' doc comment for why this is a
/// single continuous transcode per file rather than one `ffmpeg` process
/// per segment (the old design here produced audible per-segment audio
/// discontinuities and, occasionally, outright muxer errors that dropped
/// whole segments).
async fn hls_segment_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx, segment_index)): Path<(TorrentId, usize, usize)>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    tracing::debug!(torrent_id = %torrent_id, file_idx, segment_index, "hls segment request received");

    // Keeps this file's playback lease alive and its pieces prioritized
    // (`LibtorrentPlaybackCoordinator::refresh_hls`) - hls.js polls this
    // endpoint roughly every `SEGMENT_DURATION_SECONDS`, which is what
    // actually keeps the torrent resumed/prioritized while playing; the
    // one-time ffmpeg request to `stream_handler` when a transcode job
    // starts (see `spawn_hls_transcode`) isn't enough on its own since
    // that request is made once per continuous ffmpeg process, not once
    // per segment poll.
    state.efs.refresh_hls_playback(&torrent_id, file_idx, "hls-segment").await;

    let path = state
        .hls_jobs
        .ensure_segment_available(&torrent_id, file_idx, segment_index, state.stream_addr, &state.probes)
        .await
        .map_err(|err| {
            tracing::warn!(torrent_id = %torrent_id, file_idx, segment_index, %err, "hls segment unavailable");
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        })?;

    let bytes = tokio::fs::read(&path).await.map_err(|err| {
        tracing::error!(torrent_id = %torrent_id, file_idx, segment_index, %err, "failed to read generated hls segment file");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(([(axum::http::header::CONTENT_TYPE, "video/mp2t")], bytes))
}

/// Serves one subtitle track as a single ASS script merged from every
/// transcode run's output so far (see `merge_ass_runs`). Never waits: 404
/// until the first run has written its header, after which the frontend's
/// poll just gets a longer script each time as the job advances.
async fn subtitle_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx, stream_index)): Path<(TorrentId, usize, usize)>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    let dir = state.hls_jobs.job_dir(&torrent_id, file_idx);
    let prefix = format!("sub_{stream_index}_");
    let mut runs = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            // `<start>` for a transcode run, `bg` for the full-file pass
            // (sorted first: it starts at byte 0).
            let Some(start) = name.strip_prefix(&prefix).and_then(|rest| rest.strip_suffix(".ass")).and_then(|n| {
                if n == "bg" {
                    Some(0)
                } else {
                    n.parse::<usize>().ok()
                }
            }) else {
                continue;
            };
            match tokio::fs::read(entry.path()).await {
                Ok(bytes) => runs.push((start, String::from_utf8_lossy(&bytes).into_owned())),
                Err(err) => tracing::warn!(torrent_id = %torrent_id, file_idx, stream_index, %err, "failed to read subtitle run file"),
            }
        }
    }
    let run_count = runs.len();
    let merged = merge_ass_runs(runs).ok_or_else(|| {
        tracing::debug!(torrent_id = %torrent_id, file_idx, stream_index, run_count, "subtitle track not available yet");
        axum::http::StatusCode::NOT_FOUND
    })?;
    tracing::debug!(torrent_id = %torrent_id, file_idx, stream_index, run_count, bytes = merged.len(), "serving merged subtitle track");
    Ok((
        [(axum::http::header::CONTENT_TYPE, "text/x-ssa; charset=utf-8"), (axum::http::header::CACHE_CONTROL, "no-store")],
        merged,
    ))
}

/// Serves one font attachment written by the media probe (see `write_fonts`), waiting up
/// to `FONT_WAIT_TIMEOUT` for the dump to finish if it's still running.
async fn font_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx, stream_index)): Path<(TorrentId, usize, usize)>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    let path = state.hls_jobs.job_dir(&torrent_id, file_idx).join("fonts").join(stream_index.to_string());
    let deadline = tokio::time::Instant::now() + FONT_WAIT_TIMEOUT;
    loop {
        if let Ok(bytes) = tokio::fs::read(&path).await {
            tracing::debug!(torrent_id = %torrent_id, file_idx, stream_index, bytes = bytes.len(), "serving font attachment");
            return Ok(([(axum::http::header::CONTENT_TYPE, "application/octet-stream")], bytes));
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!(torrent_id = %torrent_id, file_idx, stream_index, "font attachment unavailable");
            return Err(axum::http::StatusCode::NOT_FOUND);
        }
        tokio::time::sleep(SEGMENT_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::merge_ass_runs;

    const HEADER: &str = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nStyle: Default,Arial,16\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

    #[test]
    fn merges_runs_dedupes_and_drops_partial_lines() {
        let run0 = format!("{HEADER}Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,a\nDialogue: 0,0:00:35.00,0:00:38.00,Default,,0,0,0,,b\n");
        // A later seek-restart run overlapping run0, with a partially-written last line.
        let run5 = format!("{HEADER}Dialogue: 0,0:00:35.00,0:00:38.00,Default,,0,0,0,,b\nDialogue: 0,0:00:50.00,0:00:53.00,Default,,0,0,0,,c\nDialogue: 0,0:00:5");
        let merged = merge_ass_runs(vec![(5, run5), (0, run0)]).unwrap();
        assert!(merged.starts_with("[Script Info]"));
        assert_eq!(merged.matches("Dialogue:").count(), 3);
        assert!(merged.ends_with(",,c\n"));
    }

    #[test]
    fn no_header_yet_is_none() {
        assert!(merge_ass_runs(vec![(0, "[Script Info]\n".to_string())]).is_none());
        assert!(merge_ass_runs(Vec::new()).is_none());
    }
}
