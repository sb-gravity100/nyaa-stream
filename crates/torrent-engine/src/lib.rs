use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::Router;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use axum_range::{KnownSize, Ranged};
use librqbit::api::{Api, TorrentIdOrHash};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};
use serde::Serialize;

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

        let app = Router::new()
            .route("/stream/{torrent_id}/{file_idx}", get(stream_handler))
            .with_state(api.clone());
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
        let response = match self.session.add_torrent(add, Some(AddTorrentOptions::default())).await {
            Ok(response) => response,
            Err(err) => {
                tracing::error!(%err, "failed to add torrent");
                return Err(err);
            }
        };

        let id = match response {
            AddTorrentResponse::Added(id, _) => {
                tracing::info!(torrent_id = id, "torrent added");
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
    /// start before the whole torrent has downloaded.
    pub fn stream_url(&self, torrent_id: TorrentId, file_idx: usize) -> String {
        format!("http://{}/stream/{}/{}", self.stream_addr, torrent_id, file_idx)
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
        })
    }
}

async fn stream_handler(
    State(api): State<Api>,
    Path((torrent_id, file_idx)): Path<(TorrentId, usize)>,
    range: Option<TypedHeader<Range>>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    tracing::debug!(torrent_id, file_idx, "stream request received");
    let file_stream = api.api_stream(TorrentIdOrHash::Id(torrent_id), file_idx).await.map_err(|err| {
        tracing::warn!(torrent_id, file_idx, %err, "stream request for unknown torrent/file");
        axum::http::StatusCode::NOT_FOUND
    })?;
    let byte_size = file_stream.len();
    let body = KnownSize::sized(file_stream, byte_size);
    let range = range.map(|TypedHeader(range)| range);
    Ok(Ranged::new(range, body))
}
