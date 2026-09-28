//! The real playback engine: a system `mpv` drawing into the app's own
//! window (`--wid`) underneath a transparent webview, driven over JSON IPC
//! by the frontend's HTML controls. Unlike `MpvPlayer` (one reply per
//! request, events discarded), this keeps a reader task running so
//! command replies (matched by `request_id`) and unsolicited events
//! (`property-change`, `end-file`...) can interleave freely.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, WriteHalf};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{connect_with_retry, ipc_path};

#[cfg(windows)]
type Pipe = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
type Pipe = tokio::net::UnixStream;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct EmbeddedMpv {
    child: tokio::process::Child,
    writer: Mutex<WriteHalf<Pipe>>,
    pending: Pending,
    next_request: AtomicU64,
}

impl EmbeddedMpv {
    /// Spawns `mpv` idle inside window `wid` (a native window handle), with
    /// `extra_args` appended to the built-in options. Every event line mpv
    /// emits is forwarded to `events` as raw JSON; the channel closing
    /// means mpv exited or its pipe broke.
    pub async fn spawn(wid: i64, extra_args: &[String], events: mpsc::UnboundedSender<Value>) -> anyhow::Result<Self> {
        let socket_path = ipc_path();
        tracing::debug!(wid, socket_path, "spawning embedded mpv");

        let mut command = Command::new("mpv");
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        command
            .arg(format!("--input-ipc-server={socket_path}"))
            .arg(format!("--wid={wid}"))
            .args([
                "--idle=yes",
                "--keep-open=yes",
                "--force-window=yes",
                "--no-terminal",
                // The webview on top owns all input and draws its own UI.
                "--no-osc",
                "--osd-level=0",
                "--no-input-default-bindings",
                "--input-vo-keyboard=no",
                "--no-input-cursor",
                "--cursor-autohide=no",
                "--hwdec=auto-safe",
                "--vo=gpu-next,gpu,",
                "--background-color=#000000",
                // Don't load the user's own mpv.conf/scripts (an OSC,
                // keybindings) into the app's player.
                "--no-config",
            ])
            .args(extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let child = command.spawn().map_err(|e| {
            tracing::error!(%e, "failed to spawn embedded mpv");
            anyhow::anyhow!("failed to spawn mpv (is it installed and on PATH?): {e}")
        })?;
        let pipe: Pipe = connect_with_retry(&socket_path).await.inspect_err(|err| {
            tracing::error!(%err, "failed to connect to embedded mpv IPC");
        })?;
        tracing::info!("connected to embedded mpv IPC");

        let (read, writer) = tokio::io::split(pipe);
        let pending: Pending = Arc::default();
        tokio::spawn(read_loop(BufReader::new(read), pending.clone(), events));

        Ok(Self { child, writer: Mutex::new(writer), pending, next_request: AtomicU64::new(1) })
    }

    /// Sends one IPC command (e.g. `["loadfile", url]`) and waits for its
    /// reply's `data`.
    pub async fn command(&self, args: &[Value]) -> anyhow::Result<Value> {
        let request_id = self.next_request.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(request_id, tx);

        let mut payload = serde_json::to_vec(&json!({ "command": args, "request_id": request_id }))?;
        payload.push(b'\n');
        let written = async {
            let mut writer = self.writer.lock().await;
            writer.write_all(&payload).await?;
            writer.flush().await
        }
        .await;
        if let Err(err) = written {
            self.pending.lock().await.remove(&request_id);
            tracing::error!(%err, "embedded mpv IPC write failed");
            anyhow::bail!("mpv IPC write failed: {err}");
        }

        match rx.await {
            Ok(Ok(data)) => Ok(data),
            Ok(Err(error)) => {
                tracing::debug!(?args, error, "embedded mpv command failed");
                anyhow::bail!("mpv command failed: {error}")
            }
            Err(_) => anyhow::bail!("mpv IPC connection closed"),
        }
    }

    /// The mpv process id - used to find the child window it creates
    /// inside `wid`.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Asks mpv to quit, then kills it if it hasn't exited within 2s.
    pub async fn quit(mut self) {
        tracing::debug!("quitting embedded mpv");
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), self.command(&["quit".into()])).await;
        if tokio::time::timeout(std::time::Duration::from_secs(2), self.child.wait()).await.is_err() {
            tracing::warn!("embedded mpv didn't exit after quit, killing");
            let _ = self.child.kill().await;
        }
        tracing::info!("embedded mpv exited");
    }
}

async fn read_loop(
    mut reader: BufReader<tokio::io::ReadHalf<Pipe>>,
    pending: Pending,
    events: mpsc::UnboundedSender<Value>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(%err, "embedded mpv IPC read failed");
                break;
            }
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            tracing::warn!(line = line.trim(), "unparseable mpv IPC line");
            continue;
        };
        if message.get("event").is_some() {
            if events.send(message).is_err() {
                break;
            }
            continue;
        }
        let Some(request_id) = message.get("request_id").and_then(Value::as_u64) else {
            continue;
        };
        if let Some(reply) = pending.lock().await.remove(&request_id) {
            let result = match message.get("error").and_then(Value::as_str) {
                Some("success") => Ok(message.get("data").cloned().unwrap_or(Value::Null)),
                other => Err(other.unwrap_or("unknown error").to_string()),
            };
            let _ = reply.send(result);
        }
    }
    tracing::info!("embedded mpv IPC closed");
    // Fail every in-flight command instead of leaving them hanging.
    pending.lock().await.clear();
}
