# PLAN.md — nyaa-stream

Anime-focused Stremio-like desktop client. Not a full Stremio addon-ecosystem
clone — a focused tool that searches nyaa.si for torrents, matches them
against AniList metadata, and streams them straight to mpv.

## Reference

`reference/stremio-core/` — shallow clone of https://github.com/Stremio/stremio-core,
kept for architecture reference only (gitignored, not a dependency). Useful
modules: `src/types/streaming_server.rs`, `src/models/streaming_server.rs`
(local streaming server model), `src/types/resource.rs` (catalog/stream
aggregation patterns).

## Stack

- **Shell:** Tauri 2 (Rust backend + Preact/TypeScript frontend via Vite)
- **Player:** system `mpv` (must be on PATH — not bundled), controlled
  per-session over its JSON IPC socket/named pipe
- **Torrent engine + streaming server:** `librqbit` (Session + Api), fronted
  by our own `axum` HTTP server that streams a torrent file's bytes with
  Range support (`axum-range`), so mpv can start playback before the full
  file is downloaded
- **Torrent source:** nyaa.si search via its RSS feed (`/?page=rss&q=...&c=...`)
- **Metadata:** AniList GraphQL API (`https://graphql.anilist.co`), no auth
  required for public queries

## Workspace layout

```
nyaa_stream/
  Cargo.toml                 workspace root
  src-tauri/                 Tauri app crate (commands, window, app state)
  crates/
    torrent-engine/          librqbit wrapper + local streaming HTTP server
    nyaa-client/             nyaa.si RSS search client
    anilist-client/          AniList GraphQL client
    mpv-ipc/                 spawns mpv, talks JSON IPC (pause/seek/volume)
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
```

## Data flow (search → play)

1. User searches a title → frontend calls `search_anime` (AniList) for
   metadata/posters and `search_torrents` (nyaa.si RSS) for releases,
   scoped to the anime category.
2. User picks a release → frontend calls `play_magnet` with the magnet
   link. Backend adds it to the librqbit session with sequential download
   enabled, gets a `stream_url` from the local streaming server, and spawns
   `mpv` pointed at that URL.
3. Frontend controls playback (pause/seek/volume) via Tauri commands that
   forward to `mpv-ipc`, which talks to the running mpv instance's IPC
   socket.

## Known gaps / not yet implemented

- Episode-to-release matching (mapping AniList episode numbers to specific
  nyaa.si torrents/batches) is not built yet — `search_torrents` is a plain
  keyword search.
- No persistence yet (watch history, library, continue-watching).
- `play_magnet` currently always streams file index `0` — needs real file
  selection when a torrent contains multiple files (e.g. batch releases).
- No download progress / buffering state surfaced to the frontend yet.
- Frontend (`src/App.tsx`) is still the scaffold template, not wired to any
  of the backend commands yet.

See `PHASES.md` for the build order.
