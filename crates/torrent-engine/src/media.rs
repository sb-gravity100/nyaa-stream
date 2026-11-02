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
//! Thumbnail, storyboard and clip-export jobs read the torrent through
//! AVIO callbacks (`direct_input`).
//!
//! Tradeoff accepted when choosing in-process: a libav crash on a malformed
//! file takes the whole app down instead of one child process.

use std::path::Path;
use std::time::Duration;

use ez_ffmpeg::core::scheduler::ffmpeg_scheduler::Running;
use ez_ffmpeg::{FfmpegContext, FfmpegScheduler, Input, Output};
use tokio_util::sync::CancellationToken;

use crate::direct_input::TorrentReader;

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
    /// Cancels a direct torrent input's reads (see `direct_input`).
    cancel: Option<CancellationToken>,
}

impl MediaJob {
    /// Stops the job without waiting for it to drain (a seek restart
    /// doesn't want the tail of the old run).
    pub fn abort(&mut self) {
        // First, so a read parked on an undownloaded piece returns and the
        // scheduler's abort (which waits for its workers) doesn't hang on it.
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        if let Some(scheduler) = self.scheduler.take() {
            tracing::debug!(job = self.label, "aborting media job");
            scheduler.abort();
        }
    }

    /// Waits for a bounded one-off media job, cancelling blocked torrent
    /// reads before returning on timeout.
    fn wait_timeout(mut self, timeout: Duration) -> anyhow::Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let Some(scheduler) = self.scheduler.as_ref() else { anyhow::bail!("media job ended without a scheduler") };
            if scheduler.is_ended() {
                let scheduler = self.scheduler.take().expect("scheduler checked above");
                return scheduler.wait().map_err(|err| anyhow::anyhow!("{}: {err}", self.label));
            }
            if std::time::Instant::now() >= deadline {
                self.abort();
                anyhow::bail!("{} timed out after {timeout:?}", self.label);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for MediaJob {
    fn drop(&mut self) {
        self.abort();
    }
}

fn start(context: FfmpegContext, label: &'static str, cancel: Option<CancellationToken>) -> anyhow::Result<MediaJob> {
    let scheduler = FfmpegScheduler::new(context).start().map_err(|err| {
        // Nothing will read from the input any more.
        if let Some(cancel) = &cancel {
            cancel.cancel();
        }
        anyhow::anyhow!("{label}: failed to start: {err}")
    })?;
    Ok(MediaJob { scheduler: Some(scheduler), label, cancel })
}

/// Where a run reads the media file from.
pub enum InputSource {
    /// Direct torrent reads - what the app uses.
    Torrent(TorrentReader),
}

fn open_input(source: InputSource, start_seconds: f64) -> (Input, Option<CancellationToken>) {
    let (mut input, cancel) = match source {
        InputSource::Torrent(reader) => {
            let (input, cancel) = reader.into_input();
            (input, Some(cancel))
        }
    };
    // Sequential playback from the start deliberately passes no start time:
    // without a Matroska Cues index on a still-downloading torrent, any seek
    // can make the demuxer estimate a byte offset instead of reading forward.
    if start_seconds > 0.0 {
        input = input.set_start_time_us((start_seconds * 1_000_000.0) as i64);
    }
    (input, cancel)
}

pub fn keyframe_byte_offset(path: &Path, seconds: f64) -> anyhow::Result<Option<u64>> {
    /// Packets to look through after the seek for the video keyframe.
    const MAX_PACKETS: usize = 2000;
    init();
    let mut context = ffmpeg_next::format::input(&path).map_err(|err| anyhow::anyhow!("opening input: {err}"))?;
    let Some(video) = context.streams().best(ffmpeg_next::media::Type::Video).map(|s| s.index()) else {
        return Ok(None);
    };
    let target = (seconds.max(0.0) * f64::from(ffmpeg_next::ffi::AV_TIME_BASE)) as i64;
    // max_ts = target: the keyframe at or before it.
    context.seek(target, ..target).map_err(|err| anyhow::anyhow!("seeking to {seconds}s: {err}"))?;
    for (stream, packet) in context.packets().take(MAX_PACKETS) {
        if stream.index() == video && packet.is_key() {
            let position = packet.position();
            tracing::debug!(seconds, position, "keyframe located");
            return Ok(u64::try_from(position).ok());
        }
    }
    Ok(None)
}

/// Decode keyframes from a snapshot made exclusively from the verified
/// contiguous prefix of a torrent file. A snapshot is deliberately a
/// standalone file: FFmpeg never sees the original partially-written file.
/// At most one frame per 5-second bucket is written, as a 240x135 JPEG.
pub fn storyboard_prefix(snapshot: &Path, output_dir: &Path, cancel: &CancellationToken, paused: &std::sync::atomic::AtomicBool) -> anyhow::Result<usize> {
    use ffmpeg_next::{codec, format, frame, media, software};
    tracing::debug!(snapshot = %snapshot.display(), output_dir = %output_dir.display(), "decoding verified storyboard prefix");
    init();
    if cancel.is_cancelled() { return Ok(0); }
    std::fs::create_dir_all(output_dir)?;
    let mut input = format::input(snapshot).map_err(|e| anyhow::anyhow!("opening verified storyboard snapshot: {e}"))?;
    let video_index = input.streams().best(media::Type::Video).map(|s| s.index());
    let Some(video_index) = video_index else { return Ok(0); };
    let params = input.stream(video_index).ok_or_else(|| anyhow::anyhow!("storyboard video stream disappeared"))?.parameters();
    let context = codec::context::Context::from_parameters(params)?;
    let mut decoder = context.decoder().video()?;
    decoder.skip_frame(ffmpeg_next::Discard::NonKey);
    let mut scaler: Option<software::scaling::Context> = None;
    let mut decoded = frame::Video::empty();
    let mut produced = 0usize;
    let mut last_bucket: Option<u64> = None;
    let mut last_write = std::time::Instant::now();
    for (stream, packet) in input.packets() {
        if cancel.is_cancelled() { break; }
        while paused.load(std::sync::atomic::Ordering::Relaxed) && !cancel.is_cancelled() { std::thread::sleep(Duration::from_millis(100)); }
        if stream.index() != video_index { continue; }
        if decoder.send_packet(&packet).is_err() { continue; }
        while decoder.receive_frame(&mut decoded).is_ok() {
            if cancel.is_cancelled() { break; }
            while paused.load(std::sync::atomic::Ordering::Relaxed) && !cancel.is_cancelled() { std::thread::sleep(Duration::from_millis(100)); }
            if !decoded.is_key() { continue; }
            let Some(pts) = decoded.pts() else { continue; };
            let time_base = stream.time_base();
            let seconds = pts as f64 * f64::from(time_base.numerator()) / f64::from(time_base.denominator());
            if !seconds.is_finite() || seconds < 0.0 { continue; }
            let bucket = (seconds / 5.0).floor() as u64;
            if last_bucket == Some(bucket) { continue; }
            let path = output_dir.join(format!("{bucket}.jpg"));
            if path.exists() { last_bucket = Some(bucket); continue; }
            if let Some(scaler) = scaler.as_mut() {
                let mut scaled = frame::Video::empty();
                scaler.run(&decoded, &mut scaled)?;
                write_storyboard_jpeg(&scaled, &path)?;
            } else {
            let mut new_scaler = software::scaling::Context::get(
                decoded.format(), decoded.width(), decoded.height(), ffmpeg_next::format::Pixel::RGB24,
                decoded.width(), decoded.height(), software::scaling::Flags::BILINEAR,
            )?;
                let mut scaled = frame::Video::empty();
                new_scaler.run(&decoded, &mut scaled)?;
                write_storyboard_jpeg(&scaled, &path)?;
                scaler = Some(new_scaler);
            }
            last_bucket = Some(bucket);
            produced += 1;
            // Pace CPU and disk use; cancellation is checked between frames.
            let elapsed = last_write.elapsed();
            if elapsed < Duration::from_millis(250) { std::thread::sleep(Duration::from_millis(250) - elapsed); }
            last_write = std::time::Instant::now();
        }
    }
    tracing::debug!(produced, "verified-prefix storyboard pass completed");
    Ok(produced)
}

fn write_storyboard_jpeg(frame: &ffmpeg_next::frame::Video, path: &Path) -> anyhow::Result<()> {
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let (width, height, stride) = (frame.width() as usize, frame.height() as usize, frame.stride(0));
    let plane = frame.data(0);
    let mut packed = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let start = row * stride;
        packed.extend_from_slice(&plane[start..start + width * 3]);
    }
    let image = image::RgbImage::from_raw(width as u32, height as u32, packed)
        .ok_or_else(|| anyhow::anyhow!("FFmpeg returned invalid RGB storyboard data"))?;
    let mut top = 0;
    let mut bottom = height;
    let max_crop = height / 5;
    let black_row = |y: usize| {
        let row = image.as_raw();
        let start = y * width * 3;
        let black = (0..width).filter(|x| {
            let offset = start + x * 3;
            row[offset] <= 14 && row[offset + 1] <= 14 && row[offset + 2] <= 14
        }).count();
        black as f32 / width as f32 >= 0.985
    };
    while top < max_crop && top + 1 < bottom && black_row(top) { top += 1; }
    while height - bottom < max_crop && bottom > top + 1 && black_row(bottom - 1) { bottom -= 1; }
    let black_column = |x: usize| {
        let row = image.as_raw();
        let black = (top..bottom).filter(|y| {
            let offset = y * width * 3 + x * 3;
            row[offset] <= 14 && row[offset + 1] <= 14 && row[offset + 2] <= 14
        }).count();
        black as f32 / (bottom - top) as f32 >= 0.985
    };
    let mut left = 0;
    let mut right = width;
    let max_x_crop = width / 5;
    while left < max_x_crop && left + 1 < right && black_column(left) { left += 1; }
    while width - right < max_x_crop && right > left + 1 && black_column(right - 1) { right -= 1; }
    let cropped = image::imageops::crop_imm(&image, left as u32, top as u32, (right - left) as u32, (bottom - top) as u32).to_image();
    let resized = image::DynamicImage::ImageRgb8(cropped).resize_to_fill(240, 135, image::imageops::FilterType::Lanczos3).to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 82).encode_image(&image::DynamicImage::ImageRgb8(resized))?;
    let temp = path.with_extension(format!("{}.jpg.tmp", NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&temp, jpeg)?;
    std::fs::rename(temp, path)?;
    Ok(())
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

/// One A-B clip to cut out of a media file.
pub struct ClipExport<'a> {
    pub input: InputSource,
    pub out: &'a Path,
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// Which audio stream of the file to keep (`0:a:<n>`, 0 = first).
    pub audio_stream: usize,
    pub encoder: H264Encoder,
}

/// Cuts `[start, end)` into an MP4 with normalized codecs: 8-bit 4:2:0
/// H.264 from `encoder` (NVENC when the machine has it - see
/// `detect_h264_encoder`) and AAC audio, whatever the source used (10-bit
/// HEVC, FLAC, E-AC-3...), so the clip plays anywhere. Always re-encodes:
/// a stream copy could only start on the source's own keyframes. `faststart`
/// moves the index up front for sharing. Blocking until the file is written -
/// call from `spawn_blocking`.
pub fn export_clip(clip: ClipExport<'_>) -> anyhow::Result<()> {
    init();
    anyhow::ensure!(clip.end_seconds > clip.start_seconds, "clip end must be after its start");
    let (input, _cancel) = open_input(clip.input, clip.start_seconds);
    let input = input
        .set_format_opt("probesize", "2000000")
        .set_format_opt("analyzeduration", "2000000")
        .set_hwaccel("auto")
        .set_recording_time_us(((clip.end_seconds - clip.start_seconds) * 1_000_000.0) as i64);
    let mut output = Output::from(clip.out.to_string_lossy().to_string())
        .set_format("mp4")
        .set_format_opt("movflags", "+faststart")
        .add_stream_map("0:v:0")
        .set_video_codec(clip.encoder.ffmpeg_name())
        .set_pix_fmt(clip.encoder.pix_fmt().0);
    for (key, value) in clip.encoder.codec_opts() {
        output = output.set_video_codec_opt(key, value);
    }
    let output = output.add_stream_map(format!("0:a:{}", clip.audio_stream)).set_audio_codec("aac");
    let context = FfmpegContext::builder().input(input).output(output).build().map_err(|err| anyhow::anyhow!("clip export: {err}"))?;
    tracing::info!(out = %clip.out.display(), start = clip.start_seconds, end = clip.end_seconds, encoder = clip.encoder.ffmpeg_name(), "exporting clip");
    FfmpegScheduler::new(context)
        .start()
        .map_err(|err| anyhow::anyhow!("clip export: failed to start: {err}"))?
        .wait()
        .map_err(|err| anyhow::anyhow!("clip export: {err}"))
}

/// Seeks directly into an sbtl-backed reader and writes its first decoded
/// frame as a centered 16:9 JPEG. The output limit makes FFmpeg stop as soon
/// as one frame has been encoded.
pub fn capture_thumbnail_frame(input: InputSource, out: &Path, start_seconds: f64, timeout: Duration) -> anyhow::Result<()> {
    init();
    anyhow::ensure!(start_seconds.is_finite() && start_seconds >= 0.0, "invalid thumbnail seek time");
    let (input, cancel) = open_input(input, start_seconds);
    let input = input.set_format_opt("probesize", "2000000").set_format_opt("analyzeduration", "2000000");
    let output = Output::from(out.to_string_lossy().to_string())
        .set_format("image2")
        .add_stream_map("0:v:0")
        .set_video_codec("mjpeg")
        .set_video_filter("scale=640:360:force_original_aspect_ratio=increase,crop=640:360")
        .set_max_video_frames(1);
    let context = FfmpegContext::builder().input(input).output(output).build()
        .map_err(|err| anyhow::anyhow!("thumbnail frame: {err}"))?;
    tracing::info!(start_seconds, out = %out.display(), "capturing one torrent thumbnail frame with ffmpeg");
    start(context, "thumbnail frame", cancel)?.wait_timeout(timeout)?;
    anyhow::ensure!(out.is_file() && std::fs::metadata(out)?.len() > 0, "ffmpeg produced no thumbnail frame");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual: `NYAA_KEYFRAME_TEST_INPUT=<file> cargo test -p torrent-engine
    /// keyframe_offset_smoke -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn keyframe_offset_smoke() {
        let input = std::env::var("NYAA_KEYFRAME_TEST_INPUT").expect("NYAA_KEYFRAME_TEST_INPUT");
        let mut last = 0;
        for seconds in [0.0, 60.0, 300.0, 600.0] {
            let offset = keyframe_byte_offset(Path::new(&input), seconds).unwrap();
            println!("{seconds}s -> {offset:?}");
            let offset = offset.expect("an offset");
            assert!(offset >= last, "offsets grow with time");
            last = offset;
        }
    }

}
