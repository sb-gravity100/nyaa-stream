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

/// Spawns `mpv` pointed at a URL (e.g. a torrent-engine stream URL) with a
/// JSON IPC socket enabled, and lets the caller send playback commands
/// (pause, seek, set volume, etc.) to the running instance.
pub struct MpvPlayer {
    child: tokio::process::Child,
    #[cfg(windows)]
    pipe: Mutex<NamedPipeClient>,
    #[cfg(unix)]
    pipe: Mutex<UnixStream>,
}

impl MpvPlayer {
    pub async fn spawn(stream_url: &str, title: &str) -> anyhow::Result<Self> {
        let socket_path = ipc_path();

        let mut command = Command::new("mpv");
        command
            .arg(format!("--input-ipc-server={socket_path}"))
            .arg(format!("--force-media-title={title}"))
            .arg("--keep-open=yes")
            .arg(stream_url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let child = command
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn mpv (is it installed and on PATH?): {e}"))?;

        let pipe = connect_with_retry(&socket_path).await?;

        Ok(Self {
            child,
            pipe: Mutex::new(pipe),
        })
    }

    pub async fn set_pause(&self, paused: bool) -> anyhow::Result<()> {
        self.set_property("pause", Value::Bool(paused)).await
    }

    pub async fn seek_seconds(&self, seconds: f64) -> anyhow::Result<()> {
        self.command(&[Value::String("seek".into()), seconds.into(), "absolute".into()])
            .await
    }

    pub async fn set_volume(&self, volume_percent: f64) -> anyhow::Result<()> {
        self.set_property("volume", volume_percent.into()).await
    }

    pub async fn get_time_position(&self) -> anyhow::Result<f64> {
        let data = self.get_property("time-pos").await?;
        Ok(data.as_f64().unwrap_or(0.0))
    }

    pub async fn quit(&mut self) -> anyhow::Result<()> {
        let _ = self
            .command(&[Value::String("quit".into())])
            .await;
        let _ = self.child.wait().await;
        Ok(())
    }

    async fn set_property(&self, name: &str, value: Value) -> anyhow::Result<()> {
        self.command(&[Value::String("set_property".into()), name.into(), value])
            .await
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
                anyhow::bail!("mpv command failed: {}", resp.error);
            }
            return Ok(resp.data);
        }
    }
}

#[cfg(windows)]
fn ipc_path() -> String {
    format!(r"\\.\pipe\nyaa-stream-mpv-{}", std::process::id())
}

#[cfg(unix)]
fn ipc_path() -> String {
    format!("/tmp/nyaa-stream-mpv-{}.sock", std::process::id())
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
