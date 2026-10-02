import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";

// Load profiler (PLAN.md "Load profiler"): a trace per playback start or
// seek, logged by the backend as one summary line when it ends. Best
// effort - tracing never affects playback.

export type TraceKind = "start" | "seek";

/** An open trace. `id` resolves to null outside Tauri or on failure. */
export interface LoadTrace {
  id: Promise<number | null>;
  ended: boolean;
}

export function traceBegin(kind: TraceKind, torrentId?: string): LoadTrace {
  const id = isTauriAvailable()
    ? invoke<number>("trace_begin", { kind, torrentId: torrentId ?? null }).catch((err) => {
        console.debug("[trace] begin failed", { kind, err: String(err) });
        return null;
      })
    : Promise.resolve(null);
  return { id, ended: false };
}

export function traceMark(trace: LoadTrace | null, stage: string, detail?: string): void {
  if (!trace || trace.ended) return;
  void trace.id.then((id) => {
    if (id == null) return;
    invoke("trace_mark", { id, stage, detail: detail ?? null }).catch((err) =>
      console.debug("[trace] mark failed", { stage, err: String(err) }),
    );
  });
}

/** `ok` when the load finished, `abandoned` when the player gave up first. */
export function traceEnd(trace: LoadTrace | null, outcome: "ok" | "abandoned"): void {
  if (!trace || trace.ended) return;
  trace.ended = true;
  void trace.id.then((id) => {
    if (id == null) return;
    invoke("trace_end", { id, outcome }).catch((err) => console.debug("[trace] end failed", { outcome, err: String(err) }));
  });
}
