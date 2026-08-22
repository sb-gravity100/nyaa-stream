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
| `src-tauri/src/lib.rs` | Tauri commands (`search_anime`, `search_torrents`, `play_magnet`, `set_pause`), app state wiring | backend, core |
| `src-tauri/tauri.conf.json` | Window/bundle config | config |
| `crates/torrent-engine/src/lib.rs` | Wraps librqbit `Session`/`Api`, runs local axum streaming HTTP server with Range support | backend, torrent, streaming |
| `crates/nyaa-client/src/lib.rs` | nyaa.si RSS search client | backend, search |
| `crates/anilist-client/src/lib.rs` | AniList GraphQL client (search + get-by-id) | backend, metadata |
| `crates/mpv-ipc/src/lib.rs` | Spawns mpv, talks JSON IPC (pause/seek/volume/quit) | backend, player |
| `src/App.tsx` | Frontend root component — **still scaffold template, not yet wired to backend commands** | frontend, TODO |
| `src/main.tsx` | Frontend entrypoint | frontend |
| `reference/stremio-core/` | Reference-only clone of Stremio's core, gitignored, not a build dependency | reference |

Update this table whenever files are added, moved, or removed.
