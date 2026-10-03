//! (nyaa-stream) Backend over sb_torrent's `tl` engine (feature `tl`).
//!
//! tl schedules downloads from reads: a reader's position is the playback
//! cursor, the read-ahead window follows it, and a read outside the window
//! is a seek that preempts queued background requests. So this backend
//! needs no playback coordinator: readers are tl streams, and
//! `manages_playback_lifecycle()` is `true` so enginefs' libtorrent-specific
//! lifecycle (probing incomplete files from disk, prepare calls) stays off.
//!
//! What downloads, by stream:
//! - foreground readers: sequential, tail prefetch, then the rest of their
//!   file in order (idle SEQ_AHEAD);
//! - probe / background readers: sequential, nothing beyond their window;
//! - `set_preload_file` / `keep_file_downloading`: a reader-less stream
//!   that fetches the file in order behind everything else;
//! - each torrent keeps one idle stream (idle NONE) open, so files nobody
//!   asked for (other episodes in a batch) are not downloaded.
//!
//! Magnet metadata is cached as `<download_dir>/.tl/<infohash>.torrent`;
//! tl's own resume sidecar sits next to it, so a reopened episode starts
//! from the data already on disk. tl brings its own DHT (state kept in
//! `<download_dir>/.tl/dht.dat`), uTP, seeding and HTTPS trackers.

use crate::backend::{
    BackendFileInfo, BackendMemoryDiagnostics, BufferStatus, EngineStats, FileStreamTrait, Growler, PeerSearch,
    PieceReadiness, StatsFile, StatsOptions, SwarmCap, TorrentBackend, TorrentHandle, TorrentSource,
};
use crate::backend::priorities::PlaybackIntent;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::io::Seek;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// How long `add_torrent` waits for a magnet's metadata.
const METADATA_TIMEOUT: Duration = Duration::from_secs(120);
/// Readers report no data after this long (`direct_input` reopens on error).
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// tl's ready threshold: playback time verified ahead before starting.
pub const READY_MS: u64 = 3000;

pub struct TlBackend {
    session: tl::Session,
    download_dir: PathBuf,
    torrents: Mutex<HashMap<String, TlHandle>>,
    /// One add at a time per info-hash, held across a magnet's metadata
    /// wait, so two requests for the same torrent can't both create it.
    adding: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
    policy: Arc<Policy>,
}

/// Backend-wide settings handles need to respect.
struct Policy {
    /// `set_seeding_enabled`: per-torrent throttling must not turn it back on.
    seeding: std::sync::atomic::AtomicBool,
    dht: bool,
}

impl TlBackend {
    pub fn new(download_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(download_dir.join(".tl")).context("creating the tl state directory")?;
        let dht_on = std::env::var_os("NYAA_TL_NO_DHT").is_none();
        let cfg = tl::Config {
            // node id + known nodes survive restarts: lookups start warm
            dht_state_path: Some(download_dir.join(".tl").join("dht.dat")),
            // tests against local swarms (engine_smoke) must not pull in
            // real peers for a well-known infohash
            dht: dht_on,
            ..Default::default()
        };
        let session = tl::Session::new(&cfg).map_err(|e| anyhow!("tl session: {e}"))?;
        tracing::info!(
            download_dir = %download_dir.display(),
            port = session.listen_port(),
            "tl backend started"
        );
        Ok(Self {
            session,
            download_dir,
            torrents: Mutex::new(HashMap::new()),
            adding: std::sync::Mutex::new(HashMap::new()),
            policy: Arc::new(Policy { seeding: std::sync::atomic::AtomicBool::new(true), dht: dht_on }),
        })
    }

    fn hash_lock(&self, info_hash: &str) -> Arc<Mutex<()>> {
        let mut adding = self.adding.lock().unwrap_or_else(|e| e.into_inner());
        adding.retain(|_, l| Arc::strong_count(l) > 1);  /* drop locks nobody holds */
        adding.entry(info_hash.to_string()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }

    fn metadata_cache(&self, info_hash: &str) -> PathBuf {
        self.download_dir.join(".tl").join(format!("{info_hash}.torrent"))
    }

    /// Turns a source into bytes tl accepts: .torrent bytes, a cached
    /// .torrent for a known magnet, the magnet itself, or a downloaded URL.
    async fn resolve_source(&self, source: TorrentSource) -> Result<Vec<u8>> {
        match source {
            TorrentSource::Bytes(b) => Ok(b),
            TorrentSource::Url(url) if url.starts_with("magnet:") => {
                if let Some(hash) = magnet_info_hash(&url) {
                    let cached = self.metadata_cache(&hash);
                    if let Ok(bytes) = tokio::fs::read(&cached).await {
                        tracing::info!(%hash, "tl: magnet metadata from cache");
                        return Ok(bytes);
                    }
                }
                Ok(url.into_bytes())
            }
            TorrentSource::Url(url) => {
                tracing::info!(%url, "tl: downloading .torrent");
                let resp = reqwest::get(&url).await.context("fetching .torrent")?.error_for_status()?;
                Ok(resp.bytes().await?.to_vec())
            }
        }
    }
}

/// A magnet's `tr=` announce URLs, decoded. A cached .torrent holds only
/// the info dictionary, so these are added explicitly.
fn magnet_trackers(url: &str) -> Vec<String> {
    url.split(['?', '&'])
        .filter_map(|kv| kv.strip_prefix("tr="))
        .filter_map(|v| urlencoding::decode(&v.replace('+', " ")).ok().map(|d| d.into_owned()))
        .collect()
}

/// Lowercase hex info-hash from a magnet's `xt=urn:btih:` (hex or base32).
fn magnet_info_hash(url: &str) -> Option<String> {
    let xt = url.split(['?', '&']).find_map(|kv| kv.strip_prefix("xt=urn:btih:"))?;
    match xt.len() {
        40 => Some(xt.to_ascii_lowercase()),
        32 => {
            // base32 -> 20 bytes
            let mut bits = 0u64;
            let mut nbits = 0;
            let mut out = Vec::with_capacity(20);
            for c in xt.bytes() {
                let v = match c.to_ascii_uppercase() {
                    b'A'..=b'Z' => c.to_ascii_uppercase() - b'A',
                    b'2'..=b'7' => c - b'2' + 26,
                    _ => return None,
                } as u64;
                bits = bits << 5 | v;
                nbits += 5;
                if nbits >= 8 {
                    nbits -= 8;
                    out.push((bits >> nbits) as u8);
                }
            }
            Some(hex::encode(out))
        }
        _ => None,
    }
}

#[async_trait::async_trait]
impl TorrentBackend for TlBackend {
    type Handle = TlHandle;

    async fn add_torrent(&self, source: TorrentSource, mut trackers: Vec<String>) -> Result<Self::Handle> {
        // Magnets: known hash up front, so the duplicate check and the
        // per-hash lock come before tl is touched and before the metadata
        // wait. Other sources: the hash is known once tl parsed them (tl
        // itself returns the existing torrent for a known hash).
        let mut guard = None;
        if let TorrentSource::Url(url) = &source {
            if url.starts_with("magnet:") {
                trackers.extend(magnet_trackers(url));
                if let Some(hash) = magnet_info_hash(url) {
                    let lock = self.hash_lock(&hash);
                    let held = lock.lock_owned().await;
                    if let Some(existing) = self.existing(&hash, &trackers).await {
                        return Ok(existing);
                    }
                    guard = Some(held);
                }
            }
        }
        let bytes = self.resolve_source(source).await?;
        let torrent = self
            .session
            .add_torrent(&bytes, &self.download_dir)
            .map_err(|e| anyhow!("tl add_torrent: {e}"))?;
        let info_hash = torrent.status().infohash_hex();
        let _guard = match guard {
            Some(g) => g,
            None => self.hash_lock(&info_hash).lock_owned().await,
        };
        if let Some(existing) = self.existing(&info_hash, &trackers).await {
            return Ok(existing);  /* `torrent` is the same tl torrent; dropping it is harmless */
        }
        for t in &trackers {
            if let Err(e) = torrent.add_tracker(t) {
                tracing::warn!(%info_hash, tracker = %t, %e, "tl: tracker rejected");
            }
        }
        tracing::info!(%info_hash, has_metadata = torrent.status().has_metadata, "tl: torrent added");

        if !torrent.status().has_metadata {
            let t = torrent.clone();
            let started = Instant::now();
            tokio::task::spawn_blocking(move || t.wait_metadata(METADATA_TIMEOUT))
                .await?
                .map_err(|e| anyhow!("tl: no metadata for {info_hash} after {METADATA_TIMEOUT:?}: {e}"))?;
            tracing::info!(%info_hash, elapsed = ?started.elapsed(), "tl: magnet metadata received");
            if let Ok(meta) = torrent.metainfo() {
                if let Err(e) = tokio::fs::write(self.metadata_cache(&info_hash), meta).await {
                    tracing::warn!(%info_hash, %e, "tl: could not cache metadata");
                }
            }
        }

        let idle = torrent
            .open_stream(0, &tl::StreamOptions { sequential: false, idle: tl::Idle::None, tail_prefetch: false, ..Default::default() })
            .map_err(|e| anyhow!("tl idle stream: {e}"))?;
        let handle = TlHandle {
            inner: Arc::new(HandleInner {
                policy: self.policy.clone(),
                torrent,
                info_hash: info_hash.clone(),
                save_dir: self.download_dir.clone(),
                state: Mutex::new(HandleState { _idle: idle, preload: None, keep: HashMap::new(), readers: HashMap::new() }),
            }),
        };
        self.torrents.lock().await.insert(info_hash, handle.clone());
        Ok(handle)
    }

    async fn get_torrent(&self, info_hash: &str) -> Option<Self::Handle> {
        self.torrents.lock().await.get(&info_hash.to_lowercase()).cloned()
    }

    async fn remove_torrent(&self, info_hash: &str) -> Result<()> {
        // tl frees the torrent once its last reader closes.
        let removed = self.torrents.lock().await.remove(&info_hash.to_lowercase());
        tracing::info!(%info_hash, found = removed.is_some(), "tl: torrent removed");
        Ok(())
    }

    async fn list_torrents(&self) -> Vec<String> {
        self.torrents.lock().await.keys().cloned().collect()
    }

    async fn memory_diagnostics(&self) -> BackendMemoryDiagnostics {
        BackendMemoryDiagnostics::default()
    }

    fn set_seeding_enabled(&self, enabled: bool) {
        tracing::info!(enabled, "tl: seeding");
        self.policy.seeding.store(enabled, std::sync::atomic::Ordering::Relaxed);
        self.session.set_upload(enabled, None);
    }
}

impl TlBackend {
    /// The handle already serving `info_hash`, with `trackers` added to it.
    async fn existing(&self, info_hash: &str, trackers: &[String]) -> Option<TlHandle> {
        let existing = self.torrents.lock().await.get(info_hash).cloned()?;
        tracing::debug!(%info_hash, "tl: torrent already added");
        for t in trackers {
            let _ = existing.inner.torrent.add_tracker(t);
        }
        Some(existing)
    }
}

struct HandleState {
    /// Held only to stay open: keeps files nobody reads from downloading
    /// (see the module docs).
    _idle: tl::Stream,
    preload: Option<(usize, tl::Stream)>,
    keep: HashMap<usize, tl::Stream>,
    /// Newest foreground reader per file (weak: doesn't keep it open).
    readers: HashMap<usize, tl::BufferProbe>,
}

struct HandleInner {
    policy: Arc<Policy>,
    torrent: tl::Torrent,
    info_hash: String,
    save_dir: PathBuf,
    state: Mutex<HandleState>,
}

#[derive(Clone)]
pub struct TlHandle {
    inner: Arc<HandleInner>,
}

impl TlHandle {
    fn files(&self) -> Vec<tl::FileInfo> {
        self.inner.torrent.files().unwrap_or_default()
    }

    /// Torrent-relative path with native separators, as the libtorrent
    /// backend reports it (`Show\Episode 01.mkv` on Windows).
    fn relative(&self, f: &tl::FileInfo) -> String {
        Path::new(&f.path)
            .strip_prefix(&self.inner.save_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| f.path.clone())
            .replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)
    }

    fn background_stream(&self, file_idx: usize) -> Result<tl::Stream> {
        self.inner
            .torrent
            .open_stream(
                file_idx as u32,
                &tl::StreamOptions { sequential: false, idle: tl::Idle::SeqAhead, tail_prefetch: false, ..Default::default() },
            )
            .map_err(|e| anyhow!("tl stream for file {file_idx}: {e}"))
    }

    /// Verified runs already on disk: safe for anything reading the file
    /// directly (resume buffers, probes), and cheap (no stream).
    fn file_ranges(&self, file_idx: usize) -> Vec<(u64, u64)> {
        self.inner.torrent.file_ranges(file_idx as u32).unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl TorrentHandle for TlHandle {
    fn info_hash(&self) -> String {
        self.inner.info_hash.clone()
    }

    fn name(&self) -> Option<String> {
        let n = self.inner.torrent.status().name;
        (!n.is_empty()).then_some(n)
    }

    async fn stats(&self) -> EngineStats {
        let st = self.inner.torrent.status();
        let files: Vec<StatsFile> = self
            .files()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let ranges = self.file_ranges(i);
                let downloaded: u64 = ranges.iter().map(|(a, b)| b - a).sum();
                StatsFile {
                    name: Path::new(&f.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    path: self.relative(f),
                    length: f.size,
                    offset: f.offset,
                    downloaded,
                    progress: if f.size > 0 { downloaded as f64 / f.size as f64 } else { 1.0 },
                    downloaded_ranges: ranges,
                }
            })
            .collect();
        let peers = st.peers_connected as u64;
        EngineStats {
            name: st.name.clone(),
            info_hash: self.inner.info_hash.clone(),
            files,
            sources: vec![],
            opts: StatsOptions {
                dht: self.inner.policy.dht,
                tracker: true,
                path: self.inner.save_dir.to_string_lossy().into_owned(),
                growler: Growler { flood: 0, pulse: None },
                peer_search: PeerSearch { max: 80, min: 0, sources: vec![] },
                swarm_cap: SwarmCap { max_speed: None, min_peers: None },
                connections: None,
                handshake_timeout: None,
                timeout: None,
                r#virtual: false,
            },
            download_speed: st.download_rate as f64,
            upload_speed: st.upload_rate as f64,
            downloaded: st.downloaded_bytes,
            uploaded: st.uploaded_bytes,
            unchoked: peers,
            peers,
            queued: 0,
            unique: st.peers_known as u64,
            connection_tries: 0,
            peer_search_running: !st.complete,
            stream_len: st.total_bytes,
            stream_name: st.name,
            stream_progress: if st.total_bytes > 0 { st.verified_bytes as f64 / st.total_bytes as f64 } else { 0.0 },
            swarm_connections: peers,
            swarm_paused: false,
            swarm_size: st.peers_known as u64,
            is_finished: st.complete,
            has_metadata: st.has_metadata,
            // libtorrent's numbering, which the load profiler understands
            state: if st.pieces_checking > 0 {
                1
            } else if !st.has_metadata {
                2
            } else if st.complete {
                4
            } else {
                3
            },
        }
    }

    async fn add_trackers(&self, trackers: Vec<String>) -> Result<()> {
        for t in trackers {
            if let Err(e) = self.inner.torrent.add_tracker(&t) {
                tracing::warn!(info_hash = %self.inner.info_hash, tracker = %t, %e, "tl: tracker rejected");
            }
        }
        Ok(())
    }

    async fn set_upload_throttled(&self, throttled: bool) -> Result<()> {
        // Per torrent, and never re-enables seeding the user turned off:
        // the session-wide seeding switch stays as set_seeding_enabled left it.
        tracing::debug!(
            info_hash = %self.inner.info_hash,
            throttled,
            seeding = self.inner.policy.seeding.load(std::sync::atomic::Ordering::Relaxed),
            "tl: upload throttle"
        );
        self.inner.torrent.set_upload_limit(throttled.then_some(16 * 1024));
        Ok(())
    }

    async fn is_finished(&self) -> bool {
        self.inner.torrent.status().complete
    }

    async fn set_preload_file(&self, file_idx: Option<usize>) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        tracing::debug!(info_hash = %self.inner.info_hash, ?file_idx, "tl: preload file");
        state.preload = match file_idx {
            Some(i) => Some((i, self.background_stream(i)?)),
            None => None,
        };
        Ok(())
    }

    fn manages_playback_lifecycle(&self) -> bool {
        true
    }

    async fn buffer_status(&self, file_idx: usize) -> Option<BufferStatus> {
        let probe = self.inner.state.lock().await.readers.get(&file_idx).cloned()?;
        let b = probe.buffer()?;
        Some(BufferStatus {
            level: match b.level {
                tl::BufferLevel::Stalled => "stalled",
                tl::BufferLevel::Low => "low",
                tl::BufferLevel::Ready => "ready",
                tl::BufferLevel::Full => "full",
            }
            .to_string(),
            pos: b.pos,
            ahead_bytes: b.ahead_bytes,
            ahead_ms: b.ahead.as_millis() as u64,
            ready_ms: READY_MS,
            eta_ready_ms: b.eta_ready.map(|d| d.as_millis() as u64),
            rate: b.rate as u64,
            rate_known: b.rate_known,
        })
    }

    async fn is_file_complete(&self, file_idx: usize) -> bool {
        let len = self.files().get(file_idx).map(|f| f.size);
        matches!((len, self.file_ranges(file_idx).as_slice()), (Some(l), [(0, end)]) if *end == l)
    }

    async fn keep_file_downloading(&self, file_idx: usize) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        if !state.keep.contains_key(&file_idx) {
            tracing::debug!(info_hash = %self.inner.info_hash, file_idx, "tl: keep file downloading");
            let s = self.background_stream(file_idx)?;
            state.keep.insert(file_idx, s);
        }
        Ok(())
    }

    async fn get_file_reader(
        &self,
        file_idx: usize,
        start_offset: u64,
        priority: u8,
        bitrate: Option<u64>,
        intent: PlaybackIntent,
    ) -> Result<Box<dyn FileStreamTrait>> {
        let foreground = priority != 0
            && priority != 255
            && !matches!(intent, PlaybackIntent::Background | PlaybackIntent::InternalProbe | PlaybackIntent::ContainerMetadata);
        let opts = tl::StreamOptions {
            sequential: true,
            idle: if foreground { tl::Idle::SeqAhead } else { tl::Idle::None },
            tail_prefetch: foreground,
            bitrate: bitrate.map(|b| b.min(u32::MAX as u64) as u32),
            read_timeout: Some(READ_TIMEOUT),
        };
        let mut stream = self
            .inner
            .torrent
            .open_stream(file_idx as u32, &opts)
            .map_err(|e| anyhow!("tl reader for file {file_idx}: {e}"))?;
        stream.seek(std::io::SeekFrom::Start(start_offset))?;
        stream.set_buffer_targets(Some(Duration::from_millis(READY_MS)), None);
        let reader = tl::AsyncStream::from(stream);
        if foreground {
            self.inner.state.lock().await.readers.insert(file_idx, reader.buffer_probe());
        }
        tracing::info!(
            info_hash = %self.inner.info_hash,
            file_idx,
            start_offset,
            ?intent,
            foreground,
            ?bitrate,
            "tl: reader opened"
        );
        Ok(Box::new(reader))
    }

    async fn get_files(&self) -> Vec<BackendFileInfo> {
        self.files().iter().map(|f| BackendFileInfo { name: self.relative(f), length: f.size }).collect()
    }

    async fn get_file_path(&self, file_idx: usize) -> Option<String> {
        let path = self.files().get(file_idx)?.path.clone();
        Path::new(&path).is_file().then_some(path)
    }

    async fn prepare_file_for_streaming(&self, file_idx: usize) -> Result<()> {
        // Not called while manages_playback_lifecycle() is true; kept
        // meaningful anyway: wait for the file's first bytes.
        let ready = self.wait_for_piece_ready(file_idx, 0, Duration::from_secs(30), PlaybackIntent::DirectInitial).await?;
        if ready.ready { Ok(()) } else { Err(anyhow!("tl: file {file_idx} not ready: {}", ready.reason)) }
    }

    async fn clear_file_streaming(&self, file_idx: usize) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        tracing::debug!(info_hash = %self.inner.info_hash, file_idx, "tl: clear file streaming");
        state.keep.remove(&file_idx);
        if state.preload.as_ref().is_some_and(|(i, _)| *i == file_idx) {
            state.preload = None;
        }
        Ok(())
    }

    async fn wait_for_piece_ready(
        &self,
        file_idx: usize,
        offset: u64,
        timeout: Duration,
        _intent: PlaybackIntent,
    ) -> Result<PieceReadiness> {
        let stream = self
            .inner
            .torrent
            .open_stream(file_idx as u32, &tl::StreamOptions { idle: tl::Idle::None, tail_prefetch: false, ..Default::default() })
            .map_err(|e| anyhow!("tl stream for file {file_idx}: {e}"))?;
        let started = Instant::now();
        let ready = tokio::task::spawn_blocking(move || stream.wait(offset, 1, timeout))
            .await?
            .map_err(|e| anyhow!("tl wait: {e}"))?;
        let st = self.inner.torrent.status();
        tracing::debug!(info_hash = %self.inner.info_hash, file_idx, offset, ready, elapsed = ?started.elapsed(), "tl: wait_for_piece_ready");
        Ok(PieceReadiness {
            ready,
            piece: -1,
            ready_pieces: ready as u32,
            target_pieces: 1,
            elapsed_ms: started.elapsed().as_millis() as u64,
            peers: st.peers_connected as u64,
            download_rate: st.download_rate as u64,
            reason: if ready { "tl-verified".into() } else { "tl-timeout".into() },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::magnet_info_hash;

    #[test]
    fn magnet_hashes() {
        assert_eq!(
            magnet_info_hash("magnet:?xt=urn:btih:DD8255ECDC7CA55FB0BBF81323D87062DB1F6D1C&dn=x").as_deref(),
            Some("dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c")
        );
        // base32 of the same hash
        assert_eq!(
            magnet_info_hash("magnet:?xt=urn:btih:3WBFL3G4PSSV7MF37AJSHWDQMLNR63I4").as_deref(),
            Some("dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c")
        );
        assert_eq!(magnet_info_hash("magnet:?dn=x"), None);
    }

    #[test]
    fn magnet_tracker_list() {
        let m = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce&dn=x&tr=https%3A%2F%2Fa.b%2Fannounce";
        assert_eq!(
            super::magnet_trackers(m),
            vec!["udp://tracker.example:1337/announce".to_string(), "https://a.b/announce".to_string()]
        );
    }
}
