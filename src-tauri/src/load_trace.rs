//! Load profiler (PLAN.md "Load profiler"): one trace per playback start or
//! seek, with stages marked from the frontend and the backend, logged as a
//! debug line per stage and one info summary when it completes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::State;
use torrent_engine::read_stats::ReadSnapshot;

use crate::AppState;

/// A trace nobody finished (a crashed view) is dropped after this long.
const STALE_AFTER: Duration = Duration::from_secs(600);

struct Stage {
    name: String,
    /// Since the trace began.
    at_ms: u64,
    /// For spans timed in one place (`torrent_add`, `metadata`...).
    duration_ms: Option<u64>,
    detail: Option<String>,
}

struct Trace {
    kind: String,
    began: Instant,
    stages: Vec<Stage>,
    /// Extra `key=value` fields for the summary (waits, peers...).
    fields: Vec<(String, String)>,
    /// The torrent being loaded, once known, with its stream byte counters
    /// then (the end-of-trace summary's baseline).
    torrent: Option<(String, ReadSnapshot)>,
}

#[derive(Default)]
pub struct LoadTraces {
    next_id: AtomicU64,
    traces: Mutex<HashMap<u64, Trace>>,
}

impl LoadTraces {
    pub fn begin(&self, kind: &str) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut traces = self.traces.lock().unwrap_or_else(|e| e.into_inner());
        traces.retain(|_, t| t.began.elapsed() < STALE_AFTER);
        traces.insert(id, Trace { kind: kind.to_string(), began: Instant::now(), stages: Vec::new(), fields: Vec::new(), torrent: None });
        tracing::debug!(trace = id, kind, "[trace] begin");
        id
    }

    /// Records `stage` now; `duration` when the stage is a span that just ended.
    pub fn mark(&self, id: u64, stage: &str, duration: Option<Duration>, detail: Option<String>) {
        let mut traces = self.traces.lock().unwrap_or_else(|e| e.into_inner());
        let Some(trace) = traces.get_mut(&id) else {
            tracing::trace!(trace = id, stage, "[trace] mark for an unknown trace");
            return;
        };
        let at_ms = trace.began.elapsed().as_millis() as u64;
        let duration_ms = duration.map(|d| d.as_millis() as u64);
        tracing::debug!(trace = id, kind = %trace.kind, stage, at_ms, ?duration_ms, ?detail, "[trace] stage");
        trace.stages.push(Stage { name: stage.to_string(), at_ms, duration_ms, detail });
    }

    pub fn set_torrent(&self, id: u64, torrent_id: &str, base: ReadSnapshot) {
        if let Some(trace) = self.traces.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&id) {
            trace.torrent = Some((torrent_id.to_string(), base));
        }
    }

    /// The trace's torrent, start and read baseline, once known.
    fn torrent(&self, id: u64) -> Option<(String, Instant, ReadSnapshot)> {
        let traces = self.traces.lock().unwrap_or_else(|e| e.into_inner());
        let trace = traces.get(&id)?;
        trace.torrent.clone().map(|(torrent, base)| (torrent, trace.began, base))
    }

    /// Adds a `key=value` field to the trace's summary.
    pub fn field(&self, id: u64, key: &str, value: impl ToString) {
        let mut traces = self.traces.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(trace) = traces.get_mut(&id) {
            trace.fields.push((key.to_string(), value.to_string()));
        }
    }

    /// Ends the trace and logs its summary line.
    pub fn end(&self, id: u64, outcome: &str) {
        let Some(trace) = self.traces.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) else {
            return;
        };
        let summary = summarize(&trace, outcome);
        tracing::info!(trace = id, "load trace {summary}");
    }
}

fn summarize(trace: &Trace, outcome: &str) -> String {
    let mut parts = vec![
        format!("kind={}", trace.kind),
        format!("outcome={outcome}"),
        format!("total_ms={}", trace.began.elapsed().as_millis()),
    ];
    for stage in &trace.stages {
        let value = match stage.duration_ms {
            Some(ms) => format!("{}={ms}", stage.name),
            None => format!("{}@{}", stage.name, stage.at_ms),
        };
        parts.push(match &stage.detail {
            Some(detail) => format!("{value}({detail})"),
            None => value,
        });
    }
    parts.extend(trace.fields.iter().map(|(k, v)| format!("{k}={v}")));
    parts.join(" ")
}

/// Opens a trace (`start` or `seek`) and returns its id. A seek passes its
/// torrent; a start learns it in `play_magnet`.
#[tauri::command]
pub fn trace_begin(traces: State<'_, LoadTraces>, app_state: State<'_, std::sync::Arc<AppState>>, kind: String, torrent_id: Option<String>) -> u64 {
    let id = traces.begin(&kind);
    if let Some(torrent_id) = torrent_id {
        traces.set_torrent(id, &torrent_id, app_state.torrent_engine.read_snapshot(&torrent_id));
    }
    id
}

#[tauri::command]
pub fn trace_mark(traces: State<'_, LoadTraces>, id: u64, stage: String, detail: Option<String>) {
    traces.mark(id, &stage, None, detail);
}

/// `outcome`: `ok`, or `abandoned` when the player closed first. Adds the
/// torrent's stream reads since the trace began (requests, engine waits,
/// buffer vs torrent bytes) and its peers / download rate now.
#[tauri::command]
pub async fn trace_end(traces: State<'_, LoadTraces>, app_state: State<'_, std::sync::Arc<AppState>>, id: u64, outcome: String) -> Result<(), String> {
    if let Some((torrent_id, began, base)) = traces.torrent(id) {
        let engine = &app_state.torrent_engine;
        let reads = engine.read_summary(&torrent_id, began, base);
        if let Some(first) = reads.first_request {
            traces.field(id, "first_request_ms", first.saturating_duration_since(began).as_millis());
        }
        traces.field(id, "requests", reads.requests);
        traces.field(id, "torrent_wait_ms", reads.wait_total.as_millis());
        traces.field(id, "waits", reads.waits);
        traces.field(id, "longest_wait_ms", reads.longest_wait.as_millis());
        traces.field(id, "torrent_bytes", reads.bytes.saturating_sub(reads.buffer_bytes));
        traces.field(id, "buffer_bytes", reads.buffer_bytes);
        if let Ok(stats) = engine.stats(&torrent_id, 0).await {
            traces.field(id, "peers", stats.connected_peers);
            traces.field(id, "rate_mbps", format!("{:.2}", stats.download_speed_mbps));
        }
    }
    traces.end(id, &outcome);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_lists_stages_and_fields() {
        let traces = LoadTraces::default();
        let id = traces.begin("start");
        traces.mark(id, "metadata", Some(Duration::from_millis(1200)), None);
        traces.mark(id, "first_frame", None, Some("resume".into()));
        traces.field(id, "waits", 3);
        let guard = traces.traces.lock().unwrap();
        let summary = summarize(&guard[&id], "ok");
        assert!(summary.starts_with("kind=start outcome=ok total_ms="));
        assert!(summary.contains(" metadata=1200 "));
        assert!(summary.contains(" first_frame@"));
        assert!(summary.contains("(resume)"));
        assert!(summary.ends_with(" waits=3"));
    }
}
