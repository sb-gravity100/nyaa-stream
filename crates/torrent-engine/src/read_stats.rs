//! Per-torrent stream read accounting for the app's load profiler (PLAN.md
//! "Load profiler"): stream requests, reads that waited on the engine
//! (Pending -> Ready), and bytes served from the resume buffer vs. the
//! torrent. A trace reads what happened since it began.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::TorrentId;

/// Events kept per torrent - plenty for one load's window.
const MAX_EVENTS: usize = 1024;

enum Event {
    Request(Instant),
    Wait { end: Instant, duration: Duration },
}

#[derive(Default)]
pub(crate) struct TorrentReads {
    bytes: AtomicU64,
    buffer_bytes: AtomicU64,
    events: Mutex<VecDeque<Event>>,
}

impl TorrentReads {
    fn push(&self, event: Event) {
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub(crate) fn request(&self) {
        self.push(Event::Request(Instant::now()));
    }

    /// A read that went Pending on the engine and has now completed.
    pub(crate) fn waited(&self, duration: Duration) {
        self.push(Event::Wait { end: Instant::now(), duration });
    }

    pub(crate) fn read(&self, bytes: u64) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub(crate) fn from_buffer(&self, bytes: u64) {
        self.buffer_bytes.fetch_add(bytes, Ordering::Relaxed);
    }
}

/// Byte counters at one moment - a trace's baseline.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadSnapshot {
    pub bytes: u64,
    pub buffer_bytes: u64,
}

/// What a torrent's streams did since a trace began.
#[derive(Debug, Clone, Default)]
pub struct ReadSummary {
    pub requests: usize,
    /// First stream request after `since`.
    pub first_request: Option<Instant>,
    pub waits: usize,
    pub wait_total: Duration,
    pub longest_wait: Duration,
    pub bytes: u64,
    pub buffer_bytes: u64,
}

#[derive(Clone, Default)]
pub struct ReadStats {
    torrents: Arc<Mutex<HashMap<TorrentId, Arc<TorrentReads>>>>,
}

impl ReadStats {
    pub(crate) fn for_torrent(&self, torrent_id: &TorrentId) -> Arc<TorrentReads> {
        self.torrents.lock().unwrap_or_else(|e| e.into_inner()).entry(torrent_id.to_ascii_lowercase()).or_default().clone()
    }

    pub fn snapshot(&self, torrent_id: &TorrentId) -> ReadSnapshot {
        let reads = self.for_torrent(torrent_id);
        ReadSnapshot { bytes: reads.bytes.load(Ordering::Relaxed), buffer_bytes: reads.buffer_bytes.load(Ordering::Relaxed) }
    }

    pub fn summary_since(&self, torrent_id: &TorrentId, since: Instant, base: ReadSnapshot) -> ReadSummary {
        let reads = self.for_torrent(torrent_id);
        let now = self.snapshot(torrent_id);
        let mut summary = ReadSummary {
            bytes: now.bytes.saturating_sub(base.bytes),
            buffer_bytes: now.buffer_bytes.saturating_sub(base.buffer_bytes),
            ..ReadSummary::default()
        };
        for event in reads.events.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            match *event {
                Event::Request(at) if at >= since => {
                    summary.requests += 1;
                    summary.first_request.get_or_insert(at);
                }
                // Waits that ended inside the window (clipped to it).
                Event::Wait { end, duration } if end >= since => {
                    let duration = duration.min(end - since);
                    summary.waits += 1;
                    summary.wait_total += duration;
                    summary.longest_wait = summary.longest_wait.max(duration);
                }
                _ => {}
            }
        }
        summary
    }

    pub fn remove_torrent(&self, torrent_id: &TorrentId) {
        self.torrents.lock().unwrap_or_else(|e| e.into_inner()).remove(&torrent_id.to_ascii_lowercase());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_only_the_window() {
        let stats = ReadStats::default();
        let id: TorrentId = "AB".into();
        let reads = stats.for_torrent(&id);
        reads.request();
        reads.read(100);
        let since = Instant::now();
        let base = stats.snapshot(&id);
        reads.request();
        reads.waited(Duration::from_millis(5));
        reads.read(50);
        reads.from_buffer(20);
        let summary = stats.summary_since(&"ab".into(), since, base);
        assert_eq!(summary.requests, 1);
        assert_eq!(summary.waits, 1);
        assert_eq!(summary.bytes, 50);
        assert_eq!(summary.buffer_bytes, 20);
        assert!(summary.first_request.is_some());
    }
}
