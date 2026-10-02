//! Load profiler (PLAN.md "Load profiler"): one trace per playback start or
//! seek, with stages marked from the frontend and the backend, logged as a
//! debug line per stage and one info summary when it completes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::State;

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
    /// The torrent being loaded, once known (for the end-of-trace snapshot).
    torrent: Option<String>,
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

    pub fn set_torrent(&self, id: u64, torrent_id: &str) {
        if let Some(trace) = self.traces.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&id) {
            trace.torrent = Some(torrent_id.to_string());
        }
    }

    pub fn torrent(&self, id: u64) -> Option<String> {
        self.traces.lock().unwrap_or_else(|e| e.into_inner()).get(&id).and_then(|t| t.torrent.clone())
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

/// Opens a trace (`start` or `seek`) and returns its id.
#[tauri::command]
pub fn trace_begin(traces: State<'_, LoadTraces>, kind: String) -> u64 {
    traces.begin(&kind)
}

#[tauri::command]
pub fn trace_mark(traces: State<'_, LoadTraces>, id: u64, stage: String, detail: Option<String>) {
    traces.mark(id, &stage, None, detail);
}

/// `outcome`: `ok`, or `abandoned` when the player closed first.
#[tauri::command]
pub fn trace_end(traces: State<'_, LoadTraces>, id: u64, outcome: String) {
    traces.end(id, &outcome);
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
