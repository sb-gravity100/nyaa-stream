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

## Versioning — bump every time

**Every user-facing change (feat/fix) ships with a version bump.** The in-app
updater only offers builds with a higher version than the installed one.

- **Current exception (PLAN.md "Release plan"):** milestones v0.3.2-v0.8.0
  are bumped and tagged locally only - never push those tags or use
  `git push --tags`/`--follow-tags`. Only v0.9.0 is published.
- Bump with `npm run bump -- patch|minor|major` (keeps `package.json`,
  `tauri.conf.json`, `src-tauri/Cargo.toml` in lockstep), commit
  `chore: release vX.Y.Z`, tag `vX.Y.Z`.
- Release builds need the key *contents* (the `_PATH` variant is not read):
  `export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/nyaa-stream.key)"` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` (never commit the key).
- Publishing: on the tagged release commit, `npm run release -- --notes
  "<text>"` (`scripts/release.mjs`; try `--dry-run` first). It builds with the
  signing key, then uploads the NSIS `-setup.exe`, its `.sig`, the MSI +
  `.sig` and a `latest.json` (`version`, `notes`, `pub_date`,
  `platforms.windows-x86_64.{signature,url}` using the NSIS `.sig` text) via
  `gh release create vX.Y.Z --verify-tag`, pushing main and only that tag.
  Endpoint: `tauri.conf.json` → `plugins.updater`. GitHub Actions is
  unavailable (account billing-locked); `.github/workflows/release.yml` is
  manual-only (`workflow_dispatch`).
- `.githooks/pre-commit` (`core.hooksPath`, set once per clone with
  `git config core.hooksPath .githooks`) warns when code changed but the
  version still equals the latest tag. Remind the user when it fires.

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
- `crates/mpv-player` — in-process libmpv (`libmpv-2.dll`, runtime-loaded,
  bundled from `src-tauri/lib/`): `EmbeddedMpv` (playback) and headless
  `MpvPlayer` (torrent-thumbnail capture)
- Real playback is libmpv embedded in the app window (`wid`,
  `src-tauri/src/player.rs`) under a transparent *webview* background —
  never `transparent: true` on the window, which broke click input — with
  the HTML controls in `src/PlayerView.tsx` driving it via
  `src/mpvVideo.ts`. `src/HlsPlayerView.tsx` (hls.js `<video>` against
  torrent-engine's HLS playlist) is the fallback when mpv isn't installed
- FFmpeg runs in-process for the HLS fallback (`crates/torrent-engine/src/media.rs`, ez-ffmpeg +
  ffmpeg-next), statically linked from the FFmpeg that `vcpkg.json` builds
  (LGPL - never enable x264/GPL features): raw torrent bytes aren't
  reliably playable in a browser `<video>`, so `torrent_engine::HlsJobs`
  runs one continuous HLS run per torrent file, writing real segment files
  that `hls_segment_handler` serves - see PLAN.md. Building needs vcpkg +
  LLVM's libclang (see PLAN.md's Build prerequisites)
- `reference/stremio-core/` is a gitignored, reference-only clone of
  Stremio's core (https://github.com/Stremio/stremio-core) — never a build
  dependency, consult it for architecture ideas only
- Run `cargo check --workspace` after touching any Rust crate before
  considering a change done

