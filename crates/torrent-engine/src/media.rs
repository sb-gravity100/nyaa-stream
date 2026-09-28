//! In-process FFmpeg (ez-ffmpeg for pipelines, ffmpeg-next for probing),
//! replacing the `ffmpeg`/`ffprobe` CLI processes this crate used to spawn.
//!
//! Why: the release build is a GUI-subsystem app and every CLI child opened
//! a console window; the CLI also needed ffmpeg on PATH, and its only
//! diagnostics were scraped stderr lines. In-process, FFmpeg's own log goes
//! through the `log` facade into our tracing subscriber, jobs are cancelled
//! with `abort()` instead of killing processes, and FFmpeg is statically
//! linked from the project's vcpkg manifest (LGPL build - no x264; see
//! `H264Encoder`).
//!
//! Inputs still read the torrent through `stream_handler` over loopback
//! HTTP (with FFmpeg's reconnect options), exactly as the CLI did - swapping
//! that for direct read callbacks is a separate, later step.
//!
//! Tradeoff accepted when choosing in-process: a libav crash on a malformed
//! file takes the whole app down instead of one child process.

use std::collections::HashMap;
use std::path::Path;

use ez_ffmpeg::core::scheduler::ffmpeg_scheduler::Running;
use ez_ffmpeg::{FfmpegContext, FfmpegScheduler, Input, Output};

/// Demuxer/protocol options for every read of `stream_handler`. Its body can
/// end short of Content-Length when the torrent engine gives up on a piece;
/// FFmpeg's http client re-requests from the byte it stopped at instead of
/// treating that as a fatal read error. `rw_timeout` bounds a read that
/// never completes (µs).
pub(crate) const HTTP_INPUT_OPTS: &[(&str, &str)] = &[
    ("reconnect", "1"),
    ("reconnect_on_network_error", "1"),
    ("reconnect_delay_max", "5"),
    ("rw_timeout", "60000000"),
];

/// One-time FFmpeg setup: registers codecs/formats and routes FFmpeg's log
/// through the `log` facade at warning level (tracing picks it up).
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if let Err(err) = ffmpeg_next::init() {
            tracing::error!(%err, "ffmpeg init failed");
        }
        ez_ffmpeg::set_ffmpeg_log_level(ez_ffmpeg::FfmpegLogLevel::Warning);
        tracing::info!("in-process ffmpeg initialized");
    });
}

/// A running in-process FFmpeg job (replaces a `tokio::process::Child`).
pub struct MediaJob {
    scheduler: Option<FfmpegScheduler<Running>>,
    label: &'static str,
}

impl MediaJob {
    /// Whether the job has finished, failed or been aborted.
    pub fn is_ended(&self) -> bool {
        self.scheduler.as_ref().is_none_or(|s| s.is_ended())
    }

    /// Stops the job without waiting for it to drain (a seek restart
    /// doesn't want the tail of the old run).
    pub fn abort(&mut self) {
        if let Some(scheduler) = self.scheduler.take() {
            tracing::debug!(job = self.label, "aborting media job");
            scheduler.abort();
        }
    }
}

impl Drop for MediaJob {
    fn drop(&mut self) {
        self.abort();
    }
}

fn start(context: FfmpegContext, label: &'static str) -> anyhow::Result<MediaJob> {
    let scheduler = FfmpegScheduler::new(context).start().map_err(|err| anyhow::anyhow!("{label}: failed to start: {err}"))?;
    Ok(MediaJob { scheduler: Some(scheduler), label })
}

fn http_input(url: &str, start_seconds: f64) -> Input {
    let mut input = Input::from(url).set_format_opts(HTTP_INPUT_OPTS.to_vec());
    // Sequential playback from the start deliberately passes no start time:
    // without a Matroska Cues index on a still-downloading torrent, any seek
    // can make the demuxer estimate a byte offset instead of reading forward.
    if start_seconds > 0.0 {
        input = input.set_start_time_us((start_seconds * 1_000_000.0) as i64);
    }
    input
}

/// One subtitle stream to extract.
#[derive(Debug, Clone, Copy)]
pub struct SubtitleStream {
    pub index: usize,
    /// Source is already ASS/SSA: stream-copy it into the .ass output
    /// (lossless, no decoder). Everything else (SRT, WebVTT...) is decoded
    /// and re-encoded as ASS.
    pub copy: bool,
}

/// ASS output for one subtitle stream, flushed per event so a growing file
/// can be served while the job runs.
///
/// ASS tracks are stream-copied rather than decoded: in-process, a Kaleido-
/// subs release failed to decode every packet ("Invalid UTF-8 in decoded
/// subtitles", "Cannot allocate memory" - verified live), and since FFmpeg
/// only writes an output's header after the first successfully decoded
/// frame, no subtitle file was produced at all.
fn subtitle_output(dir: &Path, stream: SubtitleStream, name_suffix: &str) -> Output {
    let output = Output::from(dir.join(format!("sub_{}_{name_suffix}.ass", stream.index)).to_string_lossy().to_string())
        .set_format("ass")
        .set_format_opt("flush_packets", "1");
    if stream.copy {
        output.add_stream_map_with_copy(format!("0:{}", stream.index))
    } else {
        output.add_stream_map(format!("0:{}", stream.index)).set_subtitle_codec("ass")
    }
}

// ---------------------------------------------------------------- probing

/// Raw probe result, before torrent-engine's own filtering.
pub struct ProbedStream {
    pub index: usize,
    pub kind: ffmpeg_next::media::Type,
    pub codec: String,
    pub pix_fmt: Option<String>,
    pub tags: HashMap<String, String>,
    pub default: bool,
    /// Attachment payload (fonts), straight from the container header.
    pub attachment: Option<Vec<u8>>,
}

/// Opens `url` with small probe limits (only header-level information is
/// needed: codecs, pixel format, tags, attachments) and lists every stream.
/// Blocking - call from `spawn_blocking`.
pub fn probe_streams(url: &str) -> anyhow::Result<Vec<ProbedStream>> {
    let mut options = ffmpeg_next::Dictionary::new();
    for (key, value) in HTTP_INPUT_OPTS {
        options.set(key, value);
    }
    options.set("probesize", "1000000");
    options.set("analyzeduration", "0");
    let context = ffmpeg_next::format::input_with_dictionary(&url, options).map_err(|err| anyhow::anyhow!("opening input: {err}"))?;
    let mut streams = Vec::new();
    for stream in context.streams() {
        let parameters = stream.parameters();
        let tags = stream.metadata().iter().map(|(k, v)| (k.to_lowercase(), v.to_string())).collect();
        // SAFETY: `parameters` borrows the stream's live AVCodecParameters
        // for the duration of this loop body; we only read plain fields.
        let (pix_fmt, attachment) = unsafe {
            let raw = &*parameters.as_ptr();
            let pix_fmt = if raw.codec_type == ffmpeg_next::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO && raw.format >= 0 {
                let pixel: ffmpeg_next::format::Pixel = std::mem::transmute::<i32, ffmpeg_next::ffi::AVPixelFormat>(raw.format).into();
                pixel.descriptor().map(|d| d.name().to_string())
            } else {
                None
            };
            let attachment = if raw.codec_type == ffmpeg_next::ffi::AVMediaType::AVMEDIA_TYPE_ATTACHMENT && !raw.extradata.is_null() && raw.extradata_size > 0 {
                Some(std::slice::from_raw_parts(raw.extradata, raw.extradata_size as usize).to_vec())
            } else {
                None
            };
            (pix_fmt, attachment)
        };
        streams.push(ProbedStream {
            index: stream.index(),
            kind: parameters.medium(),
            codec: parameters.id().name().to_string(),
            pix_fmt,
            tags,
            default: stream.disposition().contains(ffmpeg_next::format::stream::Disposition::DEFAULT),
            attachment,
        });
    }
    Ok(streams)
}

// ---------------------------------------------------------------- encoders

/// The H.264 encoder a transcode uses. LGPL build: hardware encoders, with
/// OpenH264 (BSD) as the CPU fallback instead of x264 (GPL - statically
/// linking it would make the app GPL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Encoder {
    Nvenc,
    Qsv,
    Amf,
    OpenH264,
}

impl H264Encoder {
    pub fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Amf => "h264_amf",
            Self::OpenH264 => "libopenh264",
        }
    }

    fn pix_fmt(self) -> (&'static str, ffmpeg_next::format::Pixel) {
        match self {
            Self::Qsv | Self::Amf => ("nv12", ffmpeg_next::format::Pixel::NV12),
            Self::Nvenc | Self::OpenH264 => ("yuv420p", ffmpeg_next::format::Pixel::YUV420P),
        }
    }

    /// Quality-targeted settings tuned for anime with a bitrate ceiling so
    /// a torrent-fed stream never spikes past what the player buffers
    /// comfortably. OpenH264 has no quality mode worth using, so it gets a
    /// plain bitrate target.
    fn codec_opts(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Nvenc => vec![("preset", "p4"), ("tune", "hq"), ("rc", "vbr"), ("cq", "21"), ("b", "0"), ("maxrate", "12M"), ("bufsize", "24M"), ("profile", "high"), ("forced-idr", "1")],
            Self::Qsv => vec![("preset", "medium"), ("global_quality", "22"), ("maxrate", "12M"), ("bufsize", "24M"), ("profile", "high")],
            Self::Amf => vec![("quality", "balanced"), ("rc", "qvbr"), ("qvbr_quality_level", "22"), ("maxrate", "12M"), ("bufsize", "24M"), ("profile", "high")],
            Self::OpenH264 => vec![("b", "8M"), ("maxrate", "12M"), ("profile", "high")],
        }
    }

    /// Tries to open the encoder on a tiny frame size - an encoder compiled
    /// in can still fail to open without the matching GPU/driver (verified
    /// on this machine: QSV and AMF built in, only NVENC opens).
    fn opens(self) -> bool {
        let Some(codec) = ffmpeg_next::encoder::find_by_name(self.ffmpeg_name()) else {
            return false;
        };
        let context = ffmpeg_next::codec::context::Context::new_with_codec(codec);
        let Ok(mut encoder) = context.encoder().video() else {
            return false;
        };
        encoder.set_width(640);
        encoder.set_height(360);
        encoder.set_format(self.pix_fmt().1);
        encoder.set_time_base((1, 24));
        encoder.set_frame_rate(Some((24, 1)));
        encoder.open_as(codec).is_ok()
    }
}

/// First encoder that actually opens, NVENC > QSV > AMF > OpenH264. Blocking
/// (opening a hardware encoder initializes the driver) - call once from
/// `spawn_blocking`.
pub fn detect_h264_encoder() -> H264Encoder {
    init();
    for encoder in [H264Encoder::Nvenc, H264Encoder::Qsv, H264Encoder::Amf] {
        let works = encoder.opens();
        tracing::debug!(encoder = encoder.ffmpeg_name(), works, "probed hardware h264 encoder");
        if works {
            tracing::info!(encoder = encoder.ffmpeg_name(), "using hardware h264 encoder for transcodes");
            return encoder;
        }
    }
    tracing::info!("no hardware h264 encoder available, transcodes use libopenh264");
    H264Encoder::OpenH264
}

// ---------------------------------------------------------------- jobs

/// Segment file extension and init segment name of every HLS run. fMP4
/// rather than MPEG-TS: hls.js appends fMP4 to MSE as-is instead of
/// transmuxing every TS segment in JavaScript, and it's the container MSE
/// decodes HEVC from.
pub const SEGMENT_EXTENSION: &str = "m4s";
pub const INIT_SEGMENT: &str = "init.mp4";

/// Video handling for one HLS run.
pub enum VideoCodec {
    /// `hvc1`: the source is HEVC - tag it `hvc1` (parameter sets in the
    /// sample description) rather than the muxer's default `hev1`, which
    /// MSE implementations reject.
    Copy { hvc1: bool },
    Transcode(H264Encoder),
}

pub struct HlsRun<'a> {
    pub input_url: &'a str,
    pub dir: &'a Path,
    pub start_segment_index: usize,
    pub segment_seconds: f64,
    pub video: VideoCodec,
    pub subtitle_streams: &'a [SubtitleStream],
}

/// A path for the HLS muxer, with forward slashes: it places the fMP4 init
/// segment next to the playlist by cutting the playlist path at its last
/// `/` only, so a Windows path wrote `init.mp4` into the working directory
/// (or a parent of the job dir, with mixed separators - verified by the
/// smoke test below).
fn muxer_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Longest run the forced-keyframe list covers (in-process
/// `force_key_frames` takes explicit times, not the CLI's `expr:` form).
const MAX_RUN_SECONDS: f64 = 4.0 * 3600.0;

/// Starts one continuous HLS run: video (copied, or re-encoded to 8-bit
/// H.264 with IDR keyframes exactly every `segment_seconds` so segments sit
/// on the playlist grid), audio to AAC, plus one ASS output per subtitle
/// stream. `-copyts` keeps source timestamps so every run - sequential or
/// seek-restarted - shares one timeline. Segments are fMP4
/// (`<index>.m4s`) behind one `init.mp4` per run - identical across runs of
/// the same file and plan, so the one hls.js loaded stays valid after a seek
/// restart. Blocking (opens the input) - call from `spawn_blocking`.
pub fn start_hls_run(run: &HlsRun<'_>) -> anyhow::Result<MediaJob> {
    init();
    let start_seconds = run.start_segment_index as f64 * run.segment_seconds;
    let transcoding = matches!(run.video, VideoCodec::Transcode(_));
    let mut input = http_input(run.input_url, start_seconds)
        .set_format_opt("probesize", "2000000")
        .set_format_opt("analyzeduration", "2000000");
    if transcoding {
        // GPU decode when available; FFmpeg falls back to software itself.
        input = input.set_hwaccel("auto");
    }

    let mut hls = Output::from(muxer_path(&run.dir.join("ffmpeg_internal.m3u8")))
        .set_format("hls")
        .set_format_opt("hls_time", run.segment_seconds.to_string())
        .set_format_opt("hls_list_size", "0")
        .set_format_opt("hls_flags", "temp_file+independent_segments")
        .set_format_opt("hls_segment_type", "fmp4")
        .set_format_opt("hls_fmp4_init_filename", INIT_SEGMENT)
        .set_format_opt("start_number", run.start_segment_index.to_string())
        .set_format_opt("hls_segment_filename", muxer_path(&run.dir.join(format!("%d.{SEGMENT_EXTENSION}"))));
    match run.video {
        VideoCodec::Copy { hvc1 } => {
            hls = hls.add_stream_map_with_copy("0:v:0");
            if hvc1 {
                hls = hls.set_video_codec_tag("hvc1");
            }
        }
        VideoCodec::Transcode(encoder) => {
            let keyframes: Vec<String> = (0..)
                .map(|n| start_seconds + n as f64 * run.segment_seconds)
                .take_while(|t| *t <= start_seconds + MAX_RUN_SECONDS)
                .map(|t| format!("{t:.3}"))
                .collect();
            hls = hls
                .add_stream_map("0:v:0")
                .set_video_codec(encoder.ffmpeg_name())
                .set_pix_fmt(encoder.pix_fmt().0)
                .set_force_key_frames(keyframes.join(","));
            for (key, value) in encoder.codec_opts() {
                hls = hls.set_video_codec_opt(key, value);
            }
        }
    }
    // Audio is always AAC: several codecs real releases use (E-AC-3...)
    // aren't reliably playable by browsers through MSE.
    let hls = hls.add_stream_map("0:a:0").set_audio_codec("aac");

    let mut builder = FfmpegContext::builder().copyts().input(input).output(hls);
    for stream in run.subtitle_streams {
        builder = builder.output(subtitle_output(run.dir, *stream, &run.start_segment_index.to_string()));
    }
    let context = builder.build().map_err(|err| anyhow::anyhow!("hls run: {err}"))?;
    start(context, "hls run")
}

/// Subtitle-only extraction for `subtitle_streams`, named
/// `sub_<index>_<name_suffix>.ass` - the catch-up for a run that started
/// before the probe finished (suffix = its start segment) or the full-file
/// background pass (suffix `bg`, `start_seconds` 0). Blocking.
pub fn start_subtitle_run(input_url: &str, dir: &Path, start_seconds: f64, subtitle_streams: &[SubtitleStream], name_suffix: &str) -> anyhow::Result<MediaJob> {
    init();
    anyhow::ensure!(!subtitle_streams.is_empty(), "no subtitle streams");
    let mut builder = FfmpegContext::builder().copyts().input(http_input(input_url, start_seconds));
    for stream in subtitle_streams {
        builder = builder.output(subtitle_output(dir, *stream, name_suffix));
    }
    let context = builder.build().map_err(|err| anyhow::anyhow!("subtitle run: {err}"))?;
    start(context, "subtitle run")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual smoke test of a real HLS run against a local file:
    /// `NYAA_HLS_TEST_INPUT=<file> [NYAA_HLS_TEST_HVC1=1] cargo test -p
    /// torrent-engine hls_run_smoke -- --ignored --nocapture`. Writes to
    /// `NYAA_HLS_TEST_OUT` (default: a temp dir) for inspection with ffprobe.
    #[test]
    #[ignore]
    fn hls_run_smoke() {
        let input = std::env::var("NYAA_HLS_TEST_INPUT").expect("NYAA_HLS_TEST_INPUT");
        let hvc1 = std::env::var("NYAA_HLS_TEST_HVC1").is_ok();
        let start_segment_index: usize = std::env::var("NYAA_HLS_TEST_START").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
        let dir = std::env::var("NYAA_HLS_TEST_OUT").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("nyaa_hls_smoke"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut job = start_hls_run(&HlsRun {
            input_url: &input,
            dir: &dir,
            start_segment_index,
            segment_seconds: 6.0,
            video: VideoCodec::Copy { hvc1 },
            subtitle_streams: &[],
        })
        .unwrap();
        let third = dir.join(format!("{}.{SEGMENT_EXTENSION}", start_segment_index + 2));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !third.exists() && !job.is_ended() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        job.abort();
        assert!(dir.join(INIT_SEGMENT).exists(), "no init segment");
        assert!(third.exists(), "no third segment");
        println!("output in {}", dir.display());
    }
}
