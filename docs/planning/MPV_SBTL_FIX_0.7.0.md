# mpv-sbtl: custom mpv fed directly by sbtl

Base: **mpv v0.41.0**. Consumers: nyaa-stream (Tauri, embedded libmpv, dlopen'd in `crates/mpv-player/src/libmpv.rs`) and sbtl (C torrent lib, streaming-first).

## Goal
- Replace `mpv -> loopback HTTP Range (axum) -> sbtl_stream_read` with a direct `sbtl://` stream.
- Lower RAM/disk (no double buffering), faster seeks, one source of truth for buffering state in the UI.

## Current path (problems)
- Extra socket hop + copies; each seek = new Range GET, cursor lag.
- mpv caches independently: `--demuxer-max-back-bytes=150MiB`, `--cache-pause-wait=10` (adds up to 10 s start/resume latency).
- UI gets state from two places (mpv `paused-for-cache`/`demuxer-cache-state` + sbtl stats) -> can disagree/flicker; cannot tell stall vs seek vs initial load.

## Target architecture
```
sbtl (session owned by app) --ops vtable--> libmpv (stream_sbtl.c) --> demuxer --> decoders
                                   \--> sbtl-buffer property --> UI
```
- mpv never links sbtl. App passes an ops vtable: `read, wait, wake, seek_time, get_buffer, get_ranges, set_bitrate, set_idle` via exported `mpv_sbtl_register(ops, stream*) -> id`. URL: `sbtl://<id>`.
- `libmpv.rs` already dlopens libmpv -> dlsym the register symbol.

## Phases / patch series (against v0.41.0)
| # | Patch | Notes |
|---|---|---|
| P0 | Baseline: stock libmpv + `mpv_stream_cb_add_ro("sbtl")` | No fork. `cancel_fn` already bridged via `mp_cancel_set_cb`. Metrics reference. |
| P1 | `include/mpv/sbtl.h`, `stream/stream_sbtl.c` | vtable + id registry; `read` loops `wait(50ms)`, WOULDBLOCK -> wait (never return 0, 0 = EOF); `cancel` -> `wake`. Edit `stream_list[]` (stream.c ~L45-85), `meson.build` (L186 sources, L1799 headers). |
| P2 | Buffer-driven cache-pause + `sbtl-buffer` property | `STREAM_CTRL_GET_SBTL_BUFFER`; `playloop.c` L724-776: `is_low \|\| sbtl level < READY`; percent = `ahead_ms/ready_ms`; new option `--sbtl-ready-ms` (near `options.c` L662). Poll `sbtl_stream_get_buffer` ~10 Hz; **never drain `sbtl_next_event`** (single consumer = app). Property notified on change from `player/command.c`. |
| P3 | Seek hook | In `demux_seek` (`demux.c` L3804) before `queue_seek`: `sbtl_media_seek_time(ms)` -> `sbtl_seq_seek(key_off)` so prefetch starts during the seek, not after first read. |
| P4 | Hints | `sbtl_stream_set_bitrate` = bitrate x playback speed (from `sbtl_media_get_info`); on pause `SBTL_IDLE_SEQ_AHEAD`; on seek preempt. |
| P5 | Zero-copy (optional) | Needs `sbtl_stream_peek` (ptr+len+refcount into verified-piece cache) -> wrap as `AVBufferRef`. |

sbtl-side additions: `sbtl_stream_wake` (P1), `sbtl_stream_peek` (P5).

## Risks and fixes
- **Cancellation:** `sbtl_stream_read` never blocks; wait in 50 ms slices, check cancel between slices; `wake` for instant abort.
- **Multiple readers/probes:** only main demuxer reader opens `SBTL_STREAM_SEQUENTIAL`; probes/secondary use CHUNKED so they don't move the playback cursor. `SBTL_STREAM_TAIL_PREFETCH` covers Cues/moov jump.
- **Event queue:** one consumer (app). mpv polls, never pops events.
- **Double buffering:** with sbtl window (default 32 MiB) ahead, shrink mpv: `demuxer-max-bytes=24MiB`, `demuxer-max-back-bytes=8MiB`, `cache-on-disk=no`, `stream-buffer-size=256k`. Back-seeks re-read from sbtl storage.
- **Over-probing:** lower `demuxer-lavf-probesize` when sbtl media index is READY.
- **Platform:** sbtl only built on Windows so far; vtable design keeps mpv free of sbtl link/ABI.

## UI signalling (single source of truth)
Read-only mpv property `sbtl-buffer` (node map); `paused-for-cache` derived from the same state. Add name to `OBSERVED` in `src/mpvVideo.ts`.

Fields: `state, reason, level, ahead_ms, ready_ms, eta_ready_ms, fill_rate`

Final state set (decided): `loading`, `buffering`, `seeking`, `waiting-peers`, `ready`, `error`.

| state | when | label |
|---|---|---|
| loading | no metadata / media index PROBING | Loading... |
| buffering | level < READY (initial pre-roll or mid-play stall) | Buffering + % / ETA |
| seeking | seek target not yet verified | Seeking... + ETA |
| waiting-peers | STALLED and fill_rate ~0 for ~4 s | Waiting for peers |
| ready | level >= READY and demuxer has data | none |
| error | `MEDIA_FAILED` / `SBTL_ERR_MEDIA` / dead torrent | error message + retry |

- `buffering-initial` merged into `buffering`; the difference lives in `reason` (`initial | stall | seek`) for logs/debug only.
- `seeking` stays separate (resolves differently, ETA is meaningful).
- `error` prevents an infinite spinner.

Anti-flicker rules (implement in P2):
- Enter `buffering` below LOW, leave only at READY (hysteresis).
- Delay spinner ~250 ms so instant seeks never flash it.
- Keep spinner visible >= ~400 ms once shown.
- Dedupe property events in `mpvVideo.ts` (log shows each `[mpv] buffering` fires twice).

`loadingProgress.ts` can use `ahead_ms/ready_ms` from the property directly.

## Debug / logging
- `[sbtl]` prefix: reader-open -> first-byte latency (new),  seek (target ms, key offset, already available?, time to first byte), any wait > 100 ms, level transitions, 1 s stat (`ahead_ms`, `fill_rate`, RSS).
- Every state change: `[sbtl] state A->B reason=... ahead=... eta=...`.
- mpv: `--msg-level=all=info,sbtl=trace,demux=debug`.

## Test / metrics (compare P0 vs HTTP path, then each patch)
- Start-up time to first frame, seek latency (random + sequential), stall count/duration, RSS, disk usage, wrong-bytes = 0.
- Reuse sbtl `bench/` swarm + paced-playback harness (10 random seeks).

## Log findings (nyaa-stream_2026-11-01.log, sbtl clean: 0 sbtl_engine WARN/ERROR)
**High**
1. mkv header parse errors ~4 ms after `loadfile` (Attachments past end, no clusters found, failed header seek; x3 at 03:37:40-58). Likely short data / early EOF on the HTTP path before first pieces verify; embedded fonts may be lost. Fixed by design in `sbtl://` (reads wait for verified bytes, never return 0 early). Confirm with reader-open -> first-byte log.
2. Stale torrent id -> 404 + `Seek failed (size 0)` (x2: 02:14:36, 03:30:53; id f1ca5bf21cd74c580723df8fae972b2a9a424d0a). Torrent removed/evicted while mpv still held the URL. Fix: validate id before `loadfile`, stop player before removal, surface a clear UI error.
