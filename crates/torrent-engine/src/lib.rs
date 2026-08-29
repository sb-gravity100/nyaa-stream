use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::Router;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use axum_range::{KnownSize, Ranged};
use librqbit::api::{Api, TorrentIdOrHash};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;

/// librqbit's `TorrentId` type alias (`usize`) isn't re-exported from the
/// crate root, so we mirror it here rather than depend on a private path.
pub type TorrentId = usize;

pub struct TorrentEngine {
    session: Arc<Session>,
    api: Api,
    stream_addr: SocketAddr,
}

pub struct AddedTorrent {
    pub id: TorrentId,
}

#[derive(Clone)]
struct StreamRouterState {
    api: Api,
    stream_addr: SocketAddr,
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
}

impl TorrentEngine {
    /// Starts a librqbit session rooted at `download_dir` and a local HTTP
    /// server that serves torrent file bytes with Range support, mirroring
    /// Stremio's local streaming server.
    pub async fn start(download_dir: PathBuf) -> anyhow::Result<Self> {
        tracing::debug!(?download_dir, "starting librqbit session");
        let session = Session::new(download_dir).await?;
        let api = Api::new(session.clone(), None);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let stream_addr = listener.local_addr()?;
        tracing::info!(%stream_addr, "streaming server listening");

        let router_state = StreamRouterState { api: api.clone(), stream_addr };
        let app = Router::new()
            .route("/stream/{torrent_id}/{file_idx}", get(stream_handler))
            .route("/remux/{torrent_id}/{file_idx}", get(remux_handler))
            .with_state(router_state);
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!(?err, "streaming server stopped");
            }
        });

        Ok(Self { session, api, stream_addr })
    }

    /// Adds a torrent from a magnet link or .torrent URL and starts
    /// downloading it. librqbit prioritizes pieces for whichever file is
    /// actually being streamed once a stream is opened on it (see
    /// `stream_url`/`api_stream`), so no extra "sequential mode" flag is
    /// needed here.
    pub async fn add(&self, magnet_or_url: &str) -> anyhow::Result<AddedTorrent> {
        tracing::debug!("adding torrent");
        let add = AddTorrent::from_url(magnet_or_url);
        // Without this, re-adding a torrent whose destination file already
        // exists on disk (replaying an episode, or a thumbnail capture
        // colliding with a real download of the same release) fails
        // outright with "allow_overwrite = false" instead of resuming from
        // whatever bytes are already there - verified live.
        let opts = AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        };
        let response = match self.session.add_torrent(add, Some(opts)).await {
            Ok(response) => response,
            Err(err) => {
                tracing::error!(%err, "failed to add torrent");
                return Err(err);
            }
        };

        let id = match response {
            AddTorrentResponse::Added(id, handle) => {
                tracing::info!(torrent_id = id, "torrent added");
                // A magnet link's file list/sizes aren't known until its
                // metadata arrives from peers (BEP 9) - without waiting
                // here, a request to /stream or /remux made immediately
                // after `add()` returns can 404 because the torrent isn't
                // queryable yet. Verified live: ffmpeg's very first request
                // in remux_handler consistently lost this race otherwise.
                if let Err(err) = handle.wait_until_initialized().await {
                    tracing::error!(torrent_id = id, %err, "torrent failed to initialize");
                    return Err(err);
                }
                id
            }
            AddTorrentResponse::AlreadyManaged(id, _) => {
                tracing::info!(torrent_id = id, "torrent already managed");
                id
            }
            AddTorrentResponse::ListOnly(_) => {
                tracing::error!("torrent was added in list-only mode, expected it to start");
                anyhow::bail!("torrent was added in list-only mode, expected it to start")
            }
        };

        Ok(AddedTorrent { id })
    }

    /// Removes a torrent and deletes its downloaded files. Used to clean up
    /// after a scratch download that only existed to pull a thumbnail frame
    /// out of the first few seconds of an episode - unlike a torrent the
    /// user actually chose to watch, there's no reason to keep seeding or
    /// keep the partial file around afterward.
    pub async fn remove(&self, id: TorrentId) -> anyhow::Result<()> {
        tracing::debug!(torrent_id = id, "removing torrent");
        self.session.delete(TorrentIdOrHash::Id(id), true).await
    }

    /// URL that streams a specific file within an already-added torrent.
    /// The local HTTP server serves it with Range support so playback can
    /// start before the whole torrent has downloaded. Raw container bytes -
    /// use `remux_url` for browser playback (see its doc comment for why).
    pub fn stream_url(&self, torrent_id: TorrentId, file_idx: usize) -> String {
        format!("http://{}/stream/{}/{}", self.stream_addr, torrent_id, file_idx)
    }

    /// URL that remuxes a specific file through `ffmpeg` into fragmented
    /// MP4 before serving it - see `remux_handler`'s doc comment for why
    /// this is what the frontend's `<video>` element should actually point
    /// at instead of `stream_url`.
    pub fn remux_url(&self, torrent_id: TorrentId, file_idx: usize) -> String {
        format!("http://{}/remux/{}/{}", self.stream_addr, torrent_id, file_idx)
    }

    /// Download progress/speed/peer-count snapshot for an in-progress
    /// torrent, polled by the frontend to show buffering feedback while
    /// mpv waits for enough data to start decoding.
    pub fn stats(&self, id: TorrentId) -> anyhow::Result<StreamStats> {
        let stats = self.api.api_stats_v1(TorrentIdOrHash::Id(id))?;
        let progress_percent = if stats.total_bytes > 0 {
            stats.progress_bytes as f64 / stats.total_bytes as f64 * 100.0
        } else {
            0.0
        };
        let (download_speed_mbps, connected_peers) = stats
            .live
            .as_ref()
            .map(|live| (live.download_speed.mbps, live.snapshot.peer_stats.live))
            .unwrap_or((0.0, 0));
        Ok(StreamStats {
            state: stats.state.to_string(),
            progress_percent,
            download_speed_mbps,
            connected_peers,
            finished: stats.finished,
            downloaded_bytes: stats.progress_bytes,
            total_bytes: stats.total_bytes,
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

async fn stream_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx)): Path<(TorrentId, usize)>,
    range: Option<TypedHeader<Range>>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    tracing::debug!(torrent_id, file_idx, "stream request received");
    let file_stream = state.api.api_stream(TorrentIdOrHash::Id(torrent_id), file_idx).await.map_err(|err| {
        tracing::warn!(torrent_id, file_idx, %err, "stream request for unknown torrent/file");
        axum::http::StatusCode::NOT_FOUND
    })?;
    let byte_size = file_stream.len();
    let body = KnownSize::sized(file_stream, byte_size);
    let range = range.map(|TypedHeader(range)| range);

    let content_type = state
        .api
        .api_torrent_details(TorrentIdOrHash::Id(torrent_id))
        .ok()
        .and_then(|details| details.files)
        .and_then(|files| files.into_iter().nth(file_idx))
        .map(|file| mime_for_filename(&file.name))
        .unwrap_or("application/octet-stream");

    Ok(([(axum::http::header::CONTENT_TYPE, content_type)], Ranged::new(range, body)))
}

#[derive(Deserialize)]
struct RemuxQuery {
    /// Seconds into the file to start the remux from - the frontend
    /// restarts the whole remux at this offset when the user seeks, since
    /// a live-piped fragmented MP4 can't be seeked within once bytes have
    /// already been sent (see `remux_handler`'s doc comment).
    start: Option<f64>,
}

/// Runs `ffprobe` against `input_url` to get the source's real duration in
/// seconds - see `remux_handler`'s doc comment for why this can't just be
/// left to ffmpeg's own remux pass. `None` on any failure (ffprobe missing,
/// times out, unparseable output) - the remux still proceeds, just without
/// an accurate duration in its output header.
///
/// Known limitation: many real MKV releases don't declare duration in their
/// upfront SegmentInfo, so ffprobe (like the browser) ends up needing to
/// scan toward the *end* of the file to compute it - verified live: this
/// reliably succeeds once a torrent is fully downloaded (probably reading
/// data already on disk from earlier plays) but reliably times out on a
/// fresh download, since that data isn't downloaded yet and may not be
/// prioritized. The short timeout here just keeps that from delaying
/// playback start; duration is simply wrong (whatever ffmpeg's own fallback
/// estimate is) until a later play/seek probes it again with more of the
/// file already on disk.
async fn probe_duration_seconds(input_url: &str) -> Option<f64> {
    let probe = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                input_url,
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output(),
    )
    .await;

    match probe {
        Ok(Ok(output)) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().parse::<f64>().ok()
        }
        Ok(Ok(output)) => {
            tracing::warn!(status = %output.status, "ffprobe duration probe exited non-zero");
            None
        }
        Ok(Err(err)) => {
            tracing::warn!(%err, "failed to spawn ffprobe for duration probe (is it installed and on PATH?)");
            None
        }
        Err(_) => {
            tracing::warn!("ffprobe duration probe timed out");
            None
        }
    }
}

/// Remuxes a torrent's file through `ffmpeg` into fragmented MP4
/// (`-c:v copy`, no video re-encoding)
/// before serving it, rather than handing the browser the raw container
/// bytes `stream_handler` serves.
///
/// This is necessary, not cosmetic: verified live that a real anime
/// release (MKV, H.264 video, E-AC-3 audio) fails in WebView2's `<video>`
/// with `MEDIA_ERR_SRC_NOT_SUPPORTED` even though `canPlayType` reports
/// "probably" for every codec involved - the actual blocker is that
/// Matroska's seek index (Cues/SeekHead) is commonly placed at the *end*
/// of the file, which an incrementally-downloading torrent can't provide
/// up front, so the browser's demuxer refuses to treat it as playable at
/// all. Fragmented MP4 has no such requirement - each fragment is
/// self-contained and playable as it arrives. Reads from `stream_handler`
/// (over loopback) rather than the file on disk directly, reusing its
/// existing Range-aware, piece-priority-aware blocking reads instead of
/// teaching ffmpeg about librqbit's growing sparse file.
///
/// Seeking works by restarting the whole remux at a new `start` offset
/// (see `RemuxQuery`) rather than seeking within one long-lived stream -
/// there's no way to jump backward/forward in an already-flowing live-piped
/// fragmented MP4 once bytes have been sent, so the frontend opens a fresh
/// request instead.
async fn remux_handler(
    State(state): State<StreamRouterState>,
    Path((torrent_id, file_idx)): Path<(TorrentId, usize)>,
    Query(query): Query<RemuxQuery>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    let start_seconds = query.start.unwrap_or(0.0);
    tracing::debug!(torrent_id, file_idx, start_seconds, "remux request received");
    let input_url = format!("http://{}/stream/{}/{}", state.stream_addr, torrent_id, file_idx);

    // ffmpeg's fragmented-MP4 muxer doesn't reliably pick up the source's
    // own declared duration when remuxing from a live HTTP input (verified
    // live: the frontend's <video> ended up with duration ~2s instead of
    // the real ~1440s), even though the MKV header has it and `ffprobe`
    // finds it fine. Probing it ourselves and passing `-t` explicitly below
    // is what actually gets a correct duration into the output header.
    let total_duration_seconds = probe_duration_seconds(&input_url).await;

    // `-ss` before `-i` seeks the *input* (ffmpeg's demuxer issues whatever
    // Range requests it needs against stream_handler to locate that
    // timestamp, which blocks on librqbit exactly like a normal read) -
    // this is what makes seeking possible at all: the frontend restarts
    // the whole remux from the requested point rather than trying to seek
    // within one long-lived stream, since a live-piped fragmented MP4 has
    // no way to jump backward/forward once bytes have already been sent.
    // `-copyts` preserves the original timestamps across that restart so
    // the resulting fragment's times still line up with the real duration
    // written into its header (see the movflags comment below) instead of
    // resetting to 0 every time the user seeks.
    let mut args: Vec<String> = vec!["-loglevel".into(), "error".into()];
    if start_seconds > 0.0 {
        args.push("-ss".into());
        args.push(start_seconds.to_string());
    }
    args.extend([
        "-i".to_string(),
        input_url,
        "-copyts".to_string(),
    ]);
    if let Some(total) = total_duration_seconds {
        // Output duration remaining from the seek point, not the full
        // episode length - `-t` limits the OUTPUT, which starts at
        // `start_seconds` here.
        args.push("-t".to_string());
        args.push((total - start_seconds).max(0.0).to_string());
    }
    args.extend([
        "-c:v".to_string(),
        "copy".to_string(),
        // Audio is always transcoded to AAC rather than copied: ffmpeg's
        // fragmented-MP4 muxer can't remux several codecs real releases
        // commonly use (verified live - E-AC-3 fails with "Cannot write
        // moov atom before EAC3 packets parsed"), and AAC is universally
        // supported by browsers. Audio transcoding is cheap CPU-wise
        // regardless, unlike video, which is why that stays `copy`.
        "-c:a".to_string(),
        "aac".to_string(),
        "-f".to_string(),
        "mp4".to_string(),
        // Deliberately omits `empty_moov` - that flag is for genuinely
        // unbounded live streams with no known duration, and produces
        // exactly that on the frontend (a <video> stuck reporting whatever's
        // currently buffered as the duration, e.g. 0:01/0:02, instead of the
        // real episode length). The source MKV already declares its real
        // duration in its own metadata, so ffmpeg can still write a proper
        // initial `moov` with it while fragmenting the actual media data
        // for progressive playback.
        "-movflags".to_string(),
        "frag_keyframe+default_base_moof".to_string(),
        "pipe:1".to_string(),
    ]);

    let mut child = tokio::process::Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            tracing::error!(torrent_id, file_idx, %err, "failed to spawn ffmpeg for remux (is it installed and on PATH?)");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let stdout = child.stdout.take().ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let stderr = child.stderr.take().ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    // The response body stream owns `stdout`; ffmpeg exits on its own once
    // the client disconnects and the read end of its stdout pipe closes.
    // Reaped here instead of left to become a zombie process. stderr is
    // captured (rather than discarded) and logged on a non-zero exit,
    // since `-loglevel error` means anything it prints there is exactly
    // what killed the remux.
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stderr_output = String::new();
        let _ = tokio::io::BufReader::new(stderr).read_to_string(&mut stderr_output).await;
        match child.wait().await {
            Ok(status) if !status.success() => {
                tracing::warn!(torrent_id, file_idx, %status, stderr = %stderr_output.trim(), "ffmpeg remux exited non-zero")
            }
            Err(err) => tracing::warn!(torrent_id, file_idx, %err, "failed to wait on ffmpeg remux process"),
            _ => {}
        }
    });

    let body = Body::from_stream(ReaderStream::new(stdout));
    Ok(([(axum::http::header::CONTENT_TYPE, "video/mp4")], body))
}
