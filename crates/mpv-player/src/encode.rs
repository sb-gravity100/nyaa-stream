//! Clip export with subtitles burned in: libmpv's encode mode (`--o`) writes
//! the section to an MP4 through the same libass renderer, fonts and style
//! options the player shows, which the in-process FFmpeg export can't do
//! (the vcpkg FFmpeg has no libass).

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::libmpv::Mpv;

/// H.264 encoders to try, best first: (mpv `--ovc`, `--ovcopts`).
const ENCODERS: &[(&str, &str)] = &[
    ("libx264", "crf=20,preset=veryfast"),
    ("h264_nvenc", "cq=21,preset=p4"),
    ("h264_qsv", "global_quality=21"),
    ("h264_amf", "quality=balanced"),
    ("libopenh264", "b=8M"),
];

/// Longest a clip export may take before it's given up on.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub struct EncodeClip {
    /// What to read: the torrent file's stream URL (or a path).
    pub input: String,
    pub out: PathBuf,
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// mpv track ids; `None` leaves the file's default audio, no subtitle
    /// id shows no subtitles.
    pub audio_id: Option<i64>,
    pub subtitle_id: Option<i64>,
    /// Extra `name=value` mpv options (subtitle style, fonts dir...).
    pub options: Vec<(String, String)>,
}

/// Encodes the clip, trying each available H.264 encoder until one works.
pub async fn encode_clip(clip: EncodeClip) -> anyhow::Result<()> {
    let mut last_error = anyhow::anyhow!("no H.264 encoder available");
    for (encoder, encoder_options) in ENCODERS {
        tracing::info!(encoder, out = %clip.out.display(), start = clip.start_seconds, end = clip.end_seconds, "encoding clip with libmpv");
        match encode_once(&clip, encoder, encoder_options).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                tracing::warn!(encoder, %err, "clip encode failed");
                let _ = tokio::fs::remove_file(&clip.out).await;
                last_error = err;
            }
        }
    }
    Err(last_error)
}

async fn encode_once(clip: &EncodeClip, encoder: &str, encoder_options: &str) -> anyhow::Result<()> {
    let mut options: Vec<(String, String)> = vec![
        ("o".into(), clip.out.to_string_lossy().into_owned()),
        ("of".into(), "mp4".into()),
        ("ovc".into(), encoder.into()),
        ("ovcopts".into(), encoder_options.into()),
        ("oac".into(), "aac".into()),
        ("oacopts".into(), "b=192k".into()),
        // Encoders take 8-bit 4:2:0; 10-bit sources are converted.
        ("vf".into(), "format=yuv420p".into()),
        ("start".into(), clip.start_seconds.to_string()),
        ("end".into(), clip.end_seconds.to_string()),
        ("config".into(), "no".into()),
        ("sub-visibility".into(), "yes".into()),
        ("sid".into(), clip.subtitle_id.map_or("no".to_string(), |id| id.to_string())),
    ];
    if let Some(id) = clip.audio_id {
        options.push(("aid".into(), id.to_string()));
    }
    options.extend(clip.options.iter().cloned());

    let (events_tx, mut events) = mpsc::unbounded_channel::<Value>();
    let input = clip.input.clone();
    let mpv = tokio::task::spawn_blocking(move || -> anyhow::Result<Mpv> {
        let mpv = Mpv::new()?;
        for (name, value) in &options {
            mpv.set_option(name, value)?;
        }
        mpv.initialize()?;
        mpv.start_events(events_tx);
        mpv.command(&["loadfile".into(), input.into()])?;
        Ok(mpv)
    })
    .await??;

    let outcome = tokio::time::timeout(EXPORT_TIMEOUT, async {
        while let Some(event) = events.recv().await {
            if event.get("event").and_then(Value::as_str) != Some("end-file") {
                continue;
            }
            return match event.get("reason").and_then(Value::as_str) {
                Some("error") => Err(anyhow::anyhow!("{}", event.get("file_error").and_then(Value::as_str).unwrap_or("encoding failed"))),
                Some("eof") | Some("stop") => Ok(()),
                other => Err(anyhow::anyhow!("encoding ended early ({})", other.unwrap_or("unknown"))),
            };
        }
        Err(anyhow::anyhow!("mpv shut down before the clip finished"))
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("clip export timed out")));

    // Destroying the player is what finalizes the MP4.
    let _ = tokio::task::spawn_blocking(move || drop(mpv)).await;
    outcome
}
