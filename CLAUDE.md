# _CLAUDE.md — General Session Rules

General-purpose session rules, independent of any specific project or tech stack. Copy this into any project's `CLAUDE.md` as a base.

---

## ⚡ START EVERY SESSION HERE

Before writing any code or reading any source file, always do this first:

1. **Ask the user to provide `PLAN.md`, `PHASES.md`, and `FILE_INDEX.md`** if they are not already in context. Do not proceed until at least `FILE_INDEX.md` is available.
2. Read `FILE_INDEX.md` (repo root) — it maps every file to its purpose and tags. Use it to find the exact file you need without scanning the tree.
3. Read `CLAUDE.md` in full.

**Do not scan directories or read source files until you have identified the target files via `FILE_INDEX.md`.**

---

## Token efficiency — CRITICAL

Every file read and every output costs money. Minimize both aggressively.

- **Read only what you need.** Use `FILE_INDEX.md` to identify the exact file before opening anything. Never open a file speculatively.
- **Read only the relevant section.** Use `view_range` when you need one function or block, not the whole file.
- **No unnecessary confirmations.** Don't summarize what you're about to do before doing it. Don't recap what you just did after doing it. Act, then move on.
- **No padding.** No filler phrases ("Great question!", "Sure, I'll help with that", "Here's what I did"). Responses should contain only information the user needs.
- **Prefer targeted edits.** Use `str_replace` on the specific lines that change. Never rewrite a whole file to change a few lines.
- **One read per file per task.** Read a file once, make all needed changes, move on. Don't re-read files you already have in context.

---

## Session saves — CRITICAL

**The user's PC crashes unpredictably, anywhere between 10 minutes and 1 hour into a session.**

- After every logical unit of work: finish the change → `git commit` → update `commits.md`.
- A "logical unit" is: one file created, one feature completed, one test passing, one doc updated.
- Never leave more than one uncommitted logical change in the working tree.
- Every commit gets its own entry prepended to `commits.md` (format below).

---

## Logging rules

**Every function, endpoint, event handler, service call, and error path must be logged.**

Log at:
- Entry point (params/body — never credentials)
- Service/handler entry
- DB or external API queries
- Branch decisions
- Exception raises
- Significant state transitions

**Never log passwords, PINs, or raw tokens of any kind.**

Use appropriate log levels:
- `debug` — fine-grained flow
- `info` — significant events
- `warning` — recoverable anomalies
- `error` — failures

---

## Key workflow rules

### commits.md rule
After every commit, **prepend** a new entry to `commits.md`:
```
### `<short-hash>` · <YYYY-MM-DD> · <commit subject>
- `affected/file1`
- `affected/file2`
```
`commits.md` itself gets its own commit: `docs: update commits.md`.

### Commit granularity
Every distinguishable change gets its own commit. Never bundle unrelated changes. If you need "and" in the commit message, it should be two commits.

### Straggler sweep
After any rename, removal, or refactor — grep for old references across all code and doc files before committing.

### Planning doc sync
When any change affects design, behaviour, schema, or architecture — update the relevant planning docs in the same task:
- `PLAN.md` — tech stack, modules, API endpoints, DB schema
- `PHASES.md` — per-phase task lists
- `FILE_INDEX.md` — file system index (update when files are added, moved, or removed)

All planning docs must stay consistent with each other at all times.

---

## Reference documents (adapt per project)
- `FILE_INDEX.md` — **file system index with tags and descriptions — read this first**
- `PLAN.md` — full tech stack, API endpoints, schema
- `PHASES.md` — detailed per-phase tasks and done criteria
- `commits.md` — commit log (prepend after every commit)

---

## Project: nyaa-stream

Anime-focused, Stremio-inspired desktop streaming client. See `PLAN.md` for
full architecture; summary:

- Tauri 2 desktop app: Rust backend (`src-tauri/` + `crates/*`) + Preact/TS
  frontend (`src/`)
- `crates/torrent-engine` — librqbit-backed torrent client + local axum
  streaming HTTP server with Range support
- `crates/nyaa-client` — nyaa.si search via RSS
- `crates/anilist-client` — AniList GraphQL metadata client
- `crates/mpv-ipc` — spawns headless system `mpv` over JSON IPC for
  torrent-thumbnail frame capture only; requires `mpv` on PATH
- Real playback is a plain HTML5 `<video>` element driven by `hls.js`
  (`src/PlayerView.tsx`) against torrent-engine's HLS playlist, not mpv —
  embedding mpv into the app window was tried and abandoned (a
  Tauri/WebView2 transparency bug on Windows broke click input app-wide;
  see PLAN.md's Known gaps)
- `ffmpeg`/`ffprobe` on PATH are required too: raw torrent bytes aren't
  reliably playable in a browser `<video>` (Matroska's seek index/duration
  commonly live near the file's end, which an incrementally-downloading
  torrent can't provide up front), so `torrent_engine::HlsJobs` runs one
  continuous `ffmpeg` transcode per torrent file, writing real HLS segment
  files to disk that `hls_segment_handler` serves - see PLAN.md
- `reference/stremio-core/` is a gitignored, reference-only clone of
  Stremio's core (https://github.com/Stremio/stremio-core) — never a build
  dependency, consult it for architecture ideas only
- Run `cargo check --workspace` after touching any Rust crate before
  considering a change done

