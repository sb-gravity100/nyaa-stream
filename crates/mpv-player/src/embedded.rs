//! The real playback engine: libmpv running inside the app process, drawing
//! into the app's own window (`wid`) underneath a transparent webview and
//! driven by the frontend's HTML controls through IPC-shaped JSON commands
//! (see `libmpv::Mpv::command`).

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::libmpv::{apply_options, Mpv};

pub struct EmbeddedMpv {
    mpv: Arc<Mpv>,
}

impl EmbeddedMpv {
    /// Starts an idle player inside window `wid` (a native window handle),
    /// with `extra_options` (`--name=value`) appended to the built-in ones.
    /// Every mpv event is forwarded to `events` as JSON; the channel closing
    /// means mpv shut down.
    pub async fn spawn(wid: i64, extra_options: &[String], events: mpsc::UnboundedSender<Value>) -> anyhow::Result<Self> {
        tracing::debug!(wid, "starting embedded libmpv");
        let mut options = vec![
            format!("--wid={wid}"),
            "--idle=yes".to_string(),
            "--keep-open=yes".to_string(),
            "--force-window=yes".to_string(),
            // The webview on top owns all input and draws its own UI.
            "--no-osc".to_string(),
            "--osd-level=0".to_string(),
            "--no-input-default-bindings".to_string(),
            "--input-vo-keyboard=no".to_string(),
            "--no-input-cursor".to_string(),
            "--cursor-autohide=no".to_string(),
            "--hwdec=auto-safe".to_string(),
            "--vo=gpu-next,gpu,".to_string(),
            "--background-color=#000000".to_string(),
            // Keep more already-played video demuxed (default 50 MiB) so a
            // jump back stays instant and its range stays on the seek bar.
            "--demuxer-max-back-bytes=150MiB".to_string(),
            // Don't load the user's own mpv.conf/scripts (an OSC,
            // keybindings) into the app's player.
            "--no-config".to_string(),
        ];
        options.extend_from_slice(extra_options);

        let mpv = tokio::task::spawn_blocking(move || -> anyhow::Result<Mpv> {
            let mpv = Mpv::new()?;
            apply_options(&mpv, &options)?;
            mpv.initialize()?;
            mpv.start_events(events);
            Ok(mpv)
        })
        .await??;
        tracing::info!(wid, "embedded libmpv started");
        Ok(Self { mpv: Arc::new(mpv) })
    }

    /// Runs one IPC-style command (e.g. `["loadfile", url]`) and returns its
    /// reply's `data`. Off the async threads: some commands (screenshots)
    /// block until mpv is done.
    pub async fn command(&self, args: &[Value]) -> anyhow::Result<Value> {
        let mpv = self.mpv.clone();
        let args = args.to_vec();
        tokio::task::spawn_blocking(move || mpv.command(&args))
            .await?
            .inspect_err(|err| tracing::debug!(%err, "embedded mpv command failed"))
            .map_err(|err| anyhow::anyhow!("mpv command failed: {err}"))
    }

    /// Destroys the player (blocks briefly while mpv tears down).
    pub async fn quit(self) {
        tracing::debug!("quitting embedded libmpv");
        let mpv = self.mpv;
        let _ = tokio::task::spawn_blocking(move || drop(mpv)).await;
    }
}
