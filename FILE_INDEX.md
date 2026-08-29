# FILE_INDEX.md — nyaa-stream

| File | Purpose | Tags |
|---|---|---|
| `Cargo.toml` | Workspace root, shared dependency versions | workspace, config |
| `package.json` | Frontend deps + Tauri CLI scripts | frontend, config |
| `PLAN.md` | Stack, architecture, data flow, known gaps | docs |
| `PHASES.md` | Build order / task list | docs |
| `commits.md` | Commit log (prepend after every commit) | docs |
| `src-tauri/Cargo.toml` | Tauri app crate manifest, depends on all `crates/*` | backend, config |
| `src-tauri/src/main.rs` | Binary entrypoint, calls `nyaa_stream_lib::run()` | backend |
| `src-tauri/src/lib.rs` | Tauri commands (`search_anime`, `get_anime_details`, `get_latest_episodes`, `search_torrents`, `search_torrents_for_anime`, `get_torrent_details_batch`, `play_magnet`, `get_stream_stats`, `stop_playback`, `log_frontend`), tracing-subscriber init, app state wiring | backend, core |
| `src-tauri/tauri.conf.json` | Window/bundle config | config |
| `crates/torrent-engine/src/lib.rs` | Wraps librqbit `Session`/`Api`, runs local axum streaming HTTP server with Range support; `stats()` exposes a `StreamStats` progress/speed/peers snapshot mirroring Stremio's streaming-server statistics endpoint | backend, torrent, streaming |
| `crates/nyaa-client/src/lib.rs` | nyaa.si search: paginated HTML-table scrape (RSS was found to cap at 75 results and ignore `p=`), query sanitization (curly-quote fix), view-page scrape for submitter/batch ground truth | backend, search |
| `crates/anilist-client/src/lib.rs` | AniList GraphQL client: title search, get-by-id (incl. `streamingEpisodes` thumbnails), batched `airingSchedules` lookup for the latest-episodes feed | backend, metadata |
| `crates/mpv-ipc/src/lib.rs` | Spawns headless mpv, talks JSON IPC; used only for `capture_torrent_thumbnail`'s frame grab (`spawn_headless` + `screenshot_to_file`) now that real playback is an HTML5 `<video>` element instead (see PLAN.md's Known gaps) | backend, thumbnail |
| `crates/kitsu-client/src/lib.rs` | Kitsu API client: resolves AniList id → Kitsu id via the mapping endpoint, fetches wide backdrop banner + per-episode thumbnails | backend, metadata |
| `src/App.tsx` | Frontend root: debounced live-search dropdown, routes between the home page and the media page | frontend, core |
| `src/HomePage.tsx` | Default view: horizontally-scrollable "Latest Episodes" row (current + previous calendar month, across saved library) and the "Library" grid | frontend |
| `src/MediaPage.tsx` | Per-anime detail page (ported from stremio-web's real `MetaDetails` layout — fixed low-opacity backdrop, docked frosted-glass episode list panel), add/remove-library toggle, renders `PlayerView` when an episode's play button is pressed | frontend |
| `src/PlayerView.tsx` | Full-viewport player: HTML5 `<video>` playing torrent-engine's stream URL directly, PotPlayer-style hover-reveal bottom control bar (play/pause, seek, volume, fullscreen, time), buffering readout from `get_stream_stats` | frontend |
| `src/episodeParser.ts` | Deterministic nyaa title → season/episode/batch parser (regex-only, no fuzzy matching), fansub group-tag submitter extraction | frontend |
| `src/library.ts` | Saved-anime persistence via `localStorage` (PLAN.md's real store choice is still open — this doesn't commit to sqlite/flat-file) | frontend |
| `src/types.ts` | Shared TS interfaces mirroring the Rust structs (`AnimeMedia`, `NyaaResult`, `TorrentDetails`, `AiringEntry`, ...) | frontend |
| `src/browserFallback.ts` | Dev-only path used when no Tauri IPC bridge is present (plain browser preview): direct AniList/nyaa fetches mirroring the Rust client logic | frontend, dev-tool |
| `src/kitsu.ts` | Frontend wrapper around `get_kitsu_metadata`, used by `MediaPage.tsx` for the backdrop/thumbnail fallback chain | frontend |
| `src/torrentThumbnail.ts` | Frontend wrapper around `capture_torrent_thumbnail`, the last-resort thumbnail source when Kitsu and AniList both lack episode art | frontend |
| `src/playback.ts` | Frontend wrapper around `play_magnet`/`get_stream_stats`/`stop_playback`; rejects/no-ops in the dev-only browser preview since torrenting has no meaningful fallback there | frontend |
| `src/devLogger.ts` | Forwards frontend `console.*`/uncaught errors into the Rust backend's `tracing` log (`log_frontend` command) - the native window has no accessible devtools console during development | frontend, dev-tool |
| `src/main.tsx` | Frontend entrypoint, installs `devLogger` before rendering | frontend |
| `reference/stremio-core/` | Reference-only clone of Stremio's core (state/data layer), gitignored, not a build dependency | reference |
| `reference/stremio-web/` | Reference-only clone of Stremio's actual UI (React), gitignored — used to verify real layout/styling/data-flow decisions against source rather than guessing | reference |

Update this table whenever files are added, moved, or removed.
