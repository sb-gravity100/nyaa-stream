mod embedded;
mod encode;
mod libmpv;

pub use embedded::EmbeddedMpv;
pub use encode::{encode_clip, EncodeClip};
pub use libmpv::{is_available, set_library_path, split_option, Mpv};

use std::sync::Arc;

use serde_json::Value;

use libmpv::apply_options;

/// Width headless captures are scaled to - matches the player's last-frame
/// capture (`LAST_FRAME_WIDTH` in PlayerView.tsx / src-tauri's
/// `player::THUMBNAIL_WIDTH`), all feeding 16:9 cards.
const THUMBNAIL_WIDTH: u32 = 640;

/// A windowless libmpv instance pointed at a URL (e.g. a torrent-engine
/// stream URL), purely to pull a single frame out of a torrent's stream as a
/// thumbnail - see `spawn_headless`/`screenshot_to_file`. Real playback is
/// `EmbeddedMpv` (embedded.rs), a separate instance drawing into the app
/// window.
pub struct MpvPlayer {
    mpv: Option<Arc<Mpv>>,
}

impl MpvPlayer {
    /// Starts libmpv with no video/audio output window (`--vo=null
    /// --ao=null`) and loads `stream_url`. `start_seconds`, when given, has
    /// mpv attempt to start already positioned there (`--start=`) instead of
    /// at 0 - callers still need to poll `get_time_position` afterward since
    /// this is a best-effort request, not a guarantee (see its own doc
    /// comment for why: without a Matroska Cues index on a still-downloading
    /// torrent, this is the same kind of imprecise/potentially slow seek as
    /// ffmpeg's `-ss` in torrent-engine - it can land short of the target,
    /// or occasionally not resolve in time at all, gracefully falling back
    /// to no thumbnail at the call site rather than failing outright).
    pub async fn spawn_headless(stream_url: &str, start_seconds: Option<f64>) -> anyhow::Result<Self> {
        tracing::debug!(stream_url, ?start_seconds, "starting headless libmpv");
        let mut options = vec![
            "--vo=null".to_string(),
            "--ao=null".to_string(),
            // Without a real video/audio output consuming frames, mpv
            // otherwise decides almost immediately that there's
            // "nothing to do" and quits as if it hit EOF - verified live
            // against the old spawned-mpv design: playback position stayed
            // at 0 and the process exited within ~2s without this flag,
            // even against a normal, fully seekable remote file.
            "--keep-open=yes".to_string(),
            // Thumbnails only need pictures: skip audio/subtitle decoding
            // entirely (`--ao=null` alone still decodes audio).
            "--aid=no".to_string(),
            "--sid=no".to_string(),
            // Card-sized frames: a full-res 1080p screenshot was several
            // times the bytes of what any card ever displays. With
            // `--vo=null`, the screenshot is the last frame handed to the
            // VO, i.e. after this filter.
            format!("--vf=lavfi=[scale={THUMBNAIL_WIDTH}:-2]"),
            "--screenshot-jpeg-quality=82".to_string(),
            "--no-config".to_string(),
        ];
        if let Some(seconds) = start_seconds {
            options.push(format!("--start={seconds}"));
        }
        let url = stream_url.to_string();
        let mpv = tokio::task::spawn_blocking(move || -> anyhow::Result<Mpv> {
            let mpv = Mpv::new()?;
            apply_options(&mpv, &options)?;
            mpv.initialize()?;
            mpv.command(&["loadfile".into(), url.into()])?;
            Ok(mpv)
        })
        .await??;
        tracing::info!("headless libmpv started");
        Ok(Self { mpv: Some(Arc::new(mpv)) })
    }

    pub async fn get_time_position(&self) -> anyhow::Result<f64> {
        let data = self.command(vec!["get_property".into(), "time-pos".into()]).await?;
        Ok(data.as_f64().unwrap_or(0.0))
    }

    /// Writes the currently-decoded video frame to `path`: a correct,
    /// non-blank JPEG of the actual frame at the current playback position.
    pub async fn screenshot_to_file(&self, path: &str) -> anyhow::Result<()> {
        self.command(vec!["screenshot-to-file".into(), path.into(), "video".into()]).await.map(|_| ())
    }

    /// Destroys the player - for aborting a headless capture that's stuck
    /// (e.g. the swarm never delivered enough data to start decoding).
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        tracing::debug!("destroying headless libmpv");
        self.quit().await
    }

    pub async fn quit(&mut self) -> anyhow::Result<()> {
        if let Some(mpv) = self.mpv.take() {
            let _ = tokio::task::spawn_blocking(move || drop(mpv)).await;
            tracing::info!("headless libmpv destroyed");
        }
        Ok(())
    }

    async fn command(&self, args: Vec<Value>) -> anyhow::Result<Value> {
        let mpv = self.mpv.clone().ok_or_else(|| anyhow::anyhow!("mpv already quit"))?;
        // Debug, not warn: the thumbnail path polls properties like time-pos
        // that are "unavailable" until the file loads - expected. Callers
        // log real failures.
        tokio::task::spawn_blocking(move || mpv.command(&args))
            .await?
            .inspect_err(|err| tracing::debug!(%err, "mpv command failed"))
            .map_err(|err| anyhow::anyhow!("mpv command failed: {err}"))
    }
}
