//! nyaa-stream's torrent engine over sbtl (https://github.com/sb-gravity100/sbtl),
//! the streaming-first BitTorrent library written for it. Replaces the
//! vendored enginefs - see PLAN.md "sbtl_engine".
//!
//! sbtl schedules downloads from reads: a reader's position is the playback
//! cursor, the read-ahead window follows it, and a read outside the window
//! is a seek that preempts queued background requests. So there is no
//! playback coordinator here - only torrents, readers, and a lifecycle loop
//! that removes torrents nobody uses.
//!
//! Magnet metadata is cached as `<download_dir>/.sbtl/<infohash>.torrent`;
//! sbtl's own resume file sits next to it, so a reopened episode starts from
//! the data already on disk. sbtl brings its own DHT (state kept in
//! `<download_dir>/.sbtl/dht.dat`), uTP, seeding and HTTPS trackers.

mod magnet;
mod torrent;
pub mod tracker_prober;
pub mod trackers;

pub use torrent::{BufferStatus, FileInfo, FileStats, READY_MS, ReadKind, Reader, Torrent, TorrentState, TorrentStats};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

/// How long `add` waits for a magnet's metadata.
const METADATA_TIMEOUT: Duration = Duration::from_secs(120);
/// How often the lifecycle loop runs.
const SWEEP_INTERVAL: Duration = Duration::from_secs(15);
/// A playback lease lasts this long after its last `touch_playback`.
const LEASE_TTL: Duration = Duration::from_secs(15);
/// A torrent with no reader, no lease and no access for this long is removed.
const IDLE_REMOVE_AFTER: Duration = Duration::from_secs(300);

const DEFAULT_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://9.rarbg.com:2810/announce",
    "udp://tracker.openbittorrent.com:80/announce",
    "http://tracker.openbittorrent.com:80/announce",
    "udp://opentracker.i2p.rocks:6969/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://tracker.tiny-vps.com:6969/announce",
    "udp://tracker.moeking.me:6969/announce",
    "udp://ipv4.tracker.harry.lu:80/announce",
];

/// What to add: a magnet or .torrent URL, or .torrent bytes.
pub enum Source {
    Url(String),
    Bytes(Vec<u8>),
}

pub struct Engine {
    session: sbtl::Session,
    download_dir: PathBuf,
    dht: bool,
    torrents: Mutex<HashMap<String, Arc<Torrent>>>,
    /// One add at a time per info-hash, held across a magnet's metadata
    /// wait, so two requests for the same torrent can't both create it.
    adding: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Last `touch_playback` per (info-hash, file).
    leases: Mutex<HashMap<(String, usize), Instant>>,
    trackers: trackers::TrackerManager,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    /// Starts the sbtl session in `download_dir` (which must be inside a
    /// tokio runtime: the tracker refresh and the lifecycle loop run on it).
    pub fn start(download_dir: PathBuf) -> Result<Arc<Self>> {
        std::fs::create_dir_all(download_dir.join(".sbtl")).context("creating the sbtl state directory")?;
        let dht = std::env::var_os("NYAA_SBTL_NO_DHT").is_none();
        let cfg = sbtl::Config {
            // node id + known nodes survive restarts: lookups start warm
            dht_state_path: Some(download_dir.join(".sbtl").join("dht.dat")),
            // tests against local swarms (engine_smoke) must not pull in
            // real peers for a well-known infohash
            dht,
            ..Default::default()
        };
        let session = sbtl::Session::new(&cfg).map_err(|e| anyhow!("sbtl session: {e}"))?;
        tracing::info!(download_dir = %download_dir.display(), port = session.listen_port(), dht, "sbtl engine started");
        let engine = Arc::new(Self {
            session,
            download_dir,
            dht,
            torrents: Mutex::new(HashMap::new()),
            adding: Mutex::new(HashMap::new()),
            leases: Mutex::new(HashMap::new()),
            trackers: trackers::TrackerManager::new(),
        });
        tokio::spawn(lifecycle(Arc::downgrade(&engine)));
        Ok(engine)
    }

    /// Whether the DHT is on (`NYAA_SBTL_NO_DHT` turns it off).
    pub fn dht(&self) -> bool {
        self.dht
    }

    fn metadata_cache(&self, info_hash: &str) -> PathBuf {
        self.download_dir.join(".sbtl").join(format!("{info_hash}.torrent"))
    }

    fn hash_lock(&self, info_hash: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut adding = lock(&self.adding);
        adding.retain(|_, l| Arc::strong_count(l) > 1); // drop locks nobody holds
        adding.entry(info_hash.to_string()).or_default().clone()
    }

    /// The torrent already serving `info_hash`, with `trackers` added to it.
    fn existing(&self, info_hash: &str, trackers: &[String]) -> Option<Arc<Torrent>> {
        let existing = lock(&self.torrents).get(info_hash).cloned()?;
        tracing::debug!(%info_hash, "sbtl: torrent already added");
        existing.add_trackers(trackers);
        existing.touch();
        Some(existing)
    }

    /// Turns a source into bytes sbtl accepts: .torrent bytes, a cached
    /// .torrent for a known magnet, the magnet itself, or a downloaded URL.
    async fn resolve(&self, source: Source) -> Result<Vec<u8>> {
        match source {
            Source::Bytes(b) => Ok(b),
            Source::Url(url) if url.starts_with("magnet:") => {
                if let Some(hash) = magnet::info_hash(&url) {
                    if let Ok(bytes) = tokio::fs::read(self.metadata_cache(&hash)).await {
                        tracing::info!(%hash, "sbtl: magnet metadata from cache");
                        return Ok(bytes);
                    }
                }
                Ok(url.into_bytes())
            }
            Source::Url(url) => {
                tracing::info!(%url, "sbtl: downloading .torrent");
                let resp = reqwest::get(&url).await.context("fetching .torrent")?.error_for_status()?;
                Ok(resp.bytes().await?.to_vec())
            }
        }
    }

    /// Adds a torrent (or returns the one already serving it) and waits for
    /// a magnet's metadata.
    pub async fn add(&self, source: Source) -> Result<Arc<Torrent>> {
        let mut trackers: Vec<String> = DEFAULT_TRACKERS.iter().map(|s| s.to_string()).collect();
        trackers.extend(self.trackers.get_trackers().await);
        // Magnets: known hash up front, so the duplicate check and the
        // per-hash lock come before sbtl is touched and before the metadata
        // wait. Other sources: the hash is known once sbtl parsed them (sbtl
        // itself returns the existing torrent for a known hash).
        let mut guard = None;
        if let Source::Url(url) = &source {
            if url.starts_with("magnet:") {
                trackers.extend(magnet::trackers(url));
                if let Some(hash) = magnet::info_hash(url) {
                    let held = self.hash_lock(&hash).lock_owned().await;
                    trackers.sort();
                    trackers.dedup();
                    if let Some(existing) = self.existing(&hash, &trackers) {
                        return Ok(existing);
                    }
                    guard = Some(held);
                }
            }
        }
        trackers.sort();
        trackers.dedup();
        let bytes = self.resolve(source).await?;
        let torrent = self.session.add_torrent(&bytes, &self.download_dir).map_err(|e| anyhow!("sbtl add_torrent: {e}"))?;
        let info_hash = torrent.status().infohash_hex();
        let _guard = match guard {
            Some(g) => g,
            None => self.hash_lock(&info_hash).lock_owned().await,
        };
        if let Some(existing) = self.existing(&info_hash, &trackers) {
            return Ok(existing); // `torrent` is the same sbtl torrent; dropping it is harmless
        }
        tracing::info!(%info_hash, trackers = trackers.len(), has_metadata = torrent.status().has_metadata, "sbtl: torrent added");
        for t in &trackers {
            if let Err(e) = torrent.add_tracker(t) {
                tracing::warn!(%info_hash, tracker = %t, %e, "sbtl: tracker rejected");
            }
        }
        if !torrent.status().has_metadata {
            let t = torrent.clone();
            let started = Instant::now();
            tokio::task::spawn_blocking(move || t.wait_metadata(METADATA_TIMEOUT))
                .await?
                .map_err(|e| anyhow!("sbtl: no metadata for {info_hash} after {METADATA_TIMEOUT:?}: {e}"))?;
            tracing::info!(%info_hash, elapsed = ?started.elapsed(), "sbtl: magnet metadata received");
            if let Ok(meta) = torrent.metainfo() {
                if let Err(e) = tokio::fs::write(self.metadata_cache(&info_hash), meta).await {
                    tracing::warn!(%info_hash, %e, "sbtl: could not cache metadata");
                }
            }
        }
        let torrent = Arc::new(Torrent::new(torrent, info_hash.clone(), self.download_dir.clone())?);
        lock(&self.torrents).insert(info_hash, torrent.clone());
        Ok(torrent)
    }

    /// The torrent serving `info_hash`, if added (counts as an access).
    pub fn get(&self, info_hash: &str) -> Option<Arc<Torrent>> {
        let torrent = lock(&self.torrents).get(&info_hash.to_ascii_lowercase()).cloned();
        if let Some(t) = &torrent {
            t.touch();
        }
        torrent
    }

    /// Stops serving `info_hash`. sbtl drops the torrent once its last
    /// reader closes; its files stay on disk.
    pub fn remove(&self, info_hash: &str) {
        let hash = info_hash.to_ascii_lowercase();
        let removed = lock(&self.torrents).remove(&hash).is_some();
        lock(&self.leases).retain(|(h, _), _| *h != hash);
        tracing::info!(info_hash = %hash, found = removed, "sbtl: torrent removed");
    }

    /// Keeps `info_hash` from being removed as idle while `file_idx` plays
    /// (HLS polls and stream requests call it; expires after `LEASE_TTL`).
    pub fn touch_playback(&self, info_hash: &str, file_idx: usize, source: &'static str) {
        let hash = info_hash.to_ascii_lowercase();
        let fresh = lock(&self.leases).insert((hash.clone(), file_idx), Instant::now()).is_none();
        if fresh {
            tracing::debug!(info_hash = %hash, file_idx, source, "playback lease started");
        }
    }

    /// One lifecycle pass: expires leases, then removes idle torrents.
    fn sweep(&self, now: Instant) {
        let leased: Vec<String> = {
            let mut leases = lock(&self.leases);
            leases.retain(|(hash, file_idx), at| {
                let alive = now.duration_since(*at) < LEASE_TTL;
                if !alive {
                    tracing::debug!(info_hash = %hash, file_idx, "playback lease expired");
                }
                alive
            });
            leases.keys().map(|(h, _)| h.clone()).collect()
        };
        let idle: Vec<String> = lock(&self.torrents)
            .iter()
            .filter(|(hash, t)| is_idle(t.open_readers(), leased.contains(hash), now.duration_since(t.last_access())))
            .map(|(hash, _)| hash.clone())
            .collect();
        for hash in idle {
            tracing::info!(info_hash = %hash, "removing idle torrent");
            self.remove(&hash);
        }
    }
}

/// Whether a torrent can go: no open reader, no live lease, and no access
/// for `IDLE_REMOVE_AFTER`.
fn is_idle(open_readers: usize, leased: bool, since_access: Duration) -> bool {
    open_readers == 0 && !leased && since_access > IDLE_REMOVE_AFTER
}

async fn lifecycle(engine: Weak<Engine>) {
    let mut tick = tokio::time::interval(SWEEP_INTERVAL);
    tick.tick().await;
    loop {
        tick.tick().await;
        let Some(engine) = engine.upgrade() else {
            tracing::debug!("sbtl engine dropped, lifecycle loop ends");
            return;
        };
        engine.sweep(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_needs_no_reader_no_lease_and_time() {
        let long = IDLE_REMOVE_AFTER + Duration::from_secs(1);
        assert!(is_idle(0, false, long));
        assert!(!is_idle(1, false, long), "an open reader keeps it");
        assert!(!is_idle(0, true, long), "a live lease keeps it");
        assert!(!is_idle(0, false, IDLE_REMOVE_AFTER), "recently accessed");
    }

    #[tokio::test]
    async fn leases_expire_after_ttl() {
        let dir = std::env::temp_dir().join(format!("sbtl-engine-test-{}", std::process::id()));
        std::env::set_var("NYAA_SBTL_NO_DHT", "1");
        let engine = Engine::start(dir.clone()).unwrap();
        engine.touch_playback("ABCD", 0, "test");
        assert_eq!(lock(&engine.leases).len(), 1, "hash lowercased, one lease");
        engine.touch_playback("abcd", 0, "test");
        assert_eq!(lock(&engine.leases).len(), 1);
        engine.sweep(Instant::now());
        assert_eq!(lock(&engine.leases).len(), 1, "still fresh");
        engine.sweep(Instant::now() + LEASE_TTL + Duration::from_secs(1));
        assert!(lock(&engine.leases).is_empty(), "expired");
        drop(engine);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
