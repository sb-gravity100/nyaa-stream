use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
#[cfg(unix)]
use tokio::net::UnixStream;

#[derive(Serialize)]
struct MpvCommand<'a> {
    command: &'a [Value],
}

#[derive(Deserialize)]
struct MpvResponse {
    error: String,
    #[serde(default)]
    data: Value,
}

/// Spawns a headless `mpv` pointed at a URL (e.g. a torrent-engine stream
/// URL) with a JSON IPC socket enabled, purely to pull a single frame out of
/// a torrent's stream as a thumbnail - see `spawn_headless`/
/// `screenshot_to_file`. Real playback is an HTML5 `<video>` element in the
/// frontend (see PLAN.md's Known gaps for why mpv isn't the player itself).
pub struct MpvPlayer {
    child: tokio::process::Child,
    #[cfg(windows)]
    pipe: Mutex<NamedPipeClient>,
    #[cfg(unix)]
    pipe: Mutex<UnixStream>,
}

impl MpvPlayer {
    /// Spawns `mpv` with no video/audio output window (`--vo=null --ao=null
    /// --no-terminal`), for driving it purely over IPC without ever showing
    /// a player UI - used for pulling a single frame out of a torrent's
    /// stream as a thumbnail (see `screenshot_to_file`) rather than actual
    /// playback. `start_seconds`, when given, has mpv attempt to start
    /// already positioned there (`--start=`) instead of at 0 - callers
    /// still need to poll `get_time_position` afterward since this is a
    /// best-effort request, not a guarantee (see its own doc comment for
    /// why: without a Matroska Cues index on a still-downloading torrent,
    /// this is the same kind of imprecise/potentially slow seek as
    /// ffmpeg's `-ss` in torrent-engine - it can land short of the target,
    /// or occasionally not resolve in time at all, gracefully falling back
    /// to no thumbnail at the call site rather than failing outright).
    pub async fn spawn_headless(stream_url: &str, start_seconds: Option<f64>) -> anyhow::Result<Self> {
        let mut args = vec![
            "--vo=null".to_string(),
            "--ao=null".to_string(),
            "--no-terminal".to_string(),
            // Without a real video/audio output consuming frames, mpv
            // otherwise decides almost immediately that there's
            // "nothing to do" and quits as if it hit EOF - verified
            // live (see mpv-ipc's screenshot_test example): playback
            // position stayed at 0 and the process exited within ~2s
            // without this flag, even against a normal, fully seekable
            // remote file.
            "--keep-open=yes".to_string(),
        ];
        if let Some(seconds) = start_seconds {
            args.push(format!("--start={seconds}"));
        }
        Self::spawn_with_args(stream_url, &args, "headless").await
    }

    async fn spawn_with_args(stream_url: &str, extra_args: &[String], label: &str) -> anyhow::Result<Self> {
        let socket_path = ipc_path();
        tracing::debug!(label, stream_url, socket_path, "spawning mpv");

        let mut command = Command::new("mpv");
        command
            .arg(format!("--input-ipc-server={socket_path}"))
            .args(extra_args)
            .arg(stream_url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let child = command.spawn().map_err(|e| {
            tracing::error!(label, %e, "failed to spawn mpv process");
            anyhow::anyhow!("failed to spawn mpv (is it installed and on PATH?): {e}")
        })?;
        tracing::debug!(label, "mpv process spawned, connecting to IPC socket");

        let pipe = match connect_with_retry(&socket_path).await {
            Ok(pipe) => pipe,
            Err(err) => {
                tracing::error!(label, %err, "failed to connect to mpv IPC socket");
                return Err(err);
            }
        };
        tracing::info!(label, "connected to mpv IPC socket");

        Ok(Self {
            child,
            pipe: Mutex::new(pipe),
        })
    }

    pub async fn get_time_position(&self) -> anyhow::Result<f64> {
        let data = self.get_property("time-pos").await?;
        Ok(data.as_f64().unwrap_or(0.0))
    }

    /// Writes the currently-decoded video frame to `path`. Verified live
    /// against a real headless (`--vo=null --ao=null`) instance streaming a
    /// remote MP4: produces a correct, non-blank JPEG of the actual frame
    /// at the current playback position.
    pub async fn screenshot_to_file(&self, path: &str) -> anyhow::Result<()> {
        self.command(&[
            Value::String("screenshot-to-file".into()),
            path.into(),
            "video".into(),
        ])
        .await
    }

    /// Kills the mpv process outright rather than asking it to quit over
    /// IPC - for aborting a headless capture that's stuck (e.g. the swarm
    /// never delivered enough data to start decoding) without waiting for a
    /// graceful IPC round trip that may never come.
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        tracing::debug!("killing mpv process");
        let _ = self.child.kill().await;
        Ok(())
    }

    pub async fn quit(&mut self) -> anyhow::Result<()> {
        tracing::debug!("quitting mpv");
        let _ = self
            .command(&[Value::String("quit".into())])
            .await;
        let _ = self.child.wait().await;
        tracing::info!("mpv process exited");
        Ok(())
    }

    async fn get_property(&self, name: &str) -> anyhow::Result<Value> {
        self.command_with_reply(&[Value::String("get_property".into()), name.into()])
            .await
    }

    async fn command(&self, args: &[Value]) -> anyhow::Result<()> {
        self.command_with_reply(args).await.map(|_| ())
    }

    async fn command_with_reply(&self, args: &[Value]) -> anyhow::Result<Value> {
        let payload = serde_json::to_vec(&MpvCommand { command: args })?;
        let mut pipe = self.pipe.lock().await;

        pipe.write_all(&payload).await?;
        pipe.write_all(b"\n").await?;
        pipe.flush().await?;

        let mut reader = BufReader::new(&mut *pipe);
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                anyhow::bail!("mpv IPC connection closed");
            }
            // mpv also emits unsolicited event lines on this socket; skip
            // anything that isn't a command reply.
            let Ok(resp) = serde_json::from_str::<MpvResponse>(&line) else {
                continue;
            };
            if resp.error != "success" {
                // Debug, not warn: the thumbnail path polls properties
                // like time-pos that are "unavailable" until the file
                // loads - expected, and it was flooding the log every
                // 500ms. Callers log real failures from the bail below.
                tracing::debug!(error = resp.error, "mpv command failed");
                anyhow::bail!("mpv command failed: {}", resp.error);
            }
            return Ok(resp.data);
        }
    }
}

// Per-instance counter, not just the process id: multiple concurrent
// `spawn_headless` thumbnail captures can run in the same process, and each
// needs its own IPC pipe/socket.
static NEXT_INSTANCE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(windows)]
fn ipc_path() -> String {
    let n = NEXT_INSTANCE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(r"\\.\pipe\nyaa-stream-mpv-{}-{n}", std::process::id())
}

#[cfg(unix)]
fn ipc_path() -> String {
    let n = NEXT_INSTANCE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("/tmp/nyaa-stream-mpv-{}-{n}.sock", std::process::id())
}

#[cfg(windows)]
async fn connect_with_retry(path: &str) -> anyhow::Result<NamedPipeClient> {
    for _ in 0..50 {
        match ClientOptions::new().open(path) {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    anyhow::bail!("timed out waiting for mpv IPC pipe")
}

#[cfg(unix)]
async fn connect_with_retry(path: &str) -> anyhow::Result<UnixStream> {
    for _ in 0..50 {
        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    anyhow::bail!("timed out waiting for mpv IPC socket")
}
