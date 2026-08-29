# PLAN.md — nyaa-stream

Anime-focused Stremio-like desktop client. Not a full Stremio addon-ecosystem
clone — a focused tool that searches nyaa.si for torrents, matches them
against AniList metadata, and streams them straight to mpv.

## Reference

`reference/stremio-core/` — shallow clone of https://github.com/Stremio/stremio-core,
kept for architecture reference only (gitignored, not a dependency). Useful
modules: `src/types/streaming_server.rs`, `src/models/streaming_server.rs`
(local streaming server model), `src/types/resource.rs` (catalog/stream
aggregation patterns), `src/types/resource/meta_item.rs` (confirms
`Video.series_info` is always `{season, episode}` — validates defaulting
unlabeled releases to season 1 rather than leaving season unknown).

`reference/stremio-web/` — shallow clone of https://github.com/Stremio/stremio-web
(there is no separate `stremio-desktop` repo — the desktop app is this same
UI wrapped by a native shell). Gitignored, reference-only. Used to verify
real behavior instead of guessing: `src/routes/MetaDetails/` (detail-page
layout — fixed low-opacity background image, no blur, docked frosted-glass
side panel, not a stacked banner), `src/components/MetaPreview/MetaPreview.js`
(confirms `logo`/`background`/`poster` are three distinct addon-supplied
fields, and that anime entries usually lack `logo` since Cinemeta/Kitsu
don't provide one), `src/components/*/Placeholder` (loading state is
solid-color skeleton blocks, not a spinner).

## Stack

- **Shell:** Tauri 2 (Rust backend + Preact/TypeScript frontend via Vite)
- **Player:** system `mpv` (must be on PATH — not bundled), controlled
  per-session over its JSON IPC socket/named pipe
- **Torrent engine + streaming server:** `librqbit` (Session + Api), fronted
  by our own `axum` HTTP server that streams a torrent file's bytes with
  Range support (`axum-range`), so mpv can start playback before the full
  file is downloaded
- **Torrent source:** nyaa.si search, scraping its paginated HTML results
  table (its RSS feed was found to silently ignore the `p=` page param and
  always cap at 75 results — see `crates/nyaa-client`)
- **Metadata:** AniList GraphQL API (`https://graphql.anilist.co`), no auth
  required for public queries; Kitsu API (`crates/kitsu-client`) as a
  secondary source for wide backdrop banners and per-episode thumbnails,
  resolved from an AniList id via Kitsu's crowdsourced mapping table
- **Thumbnails:** backdrop/episode art falls back Kitsu → AniList
  `streamingEpisodes` → a torrent-captured frame (`capture_torrent_thumbnail`
  spawns a headless mpv against the episode's stream, grabs one frame after
  a few seconds of playback, caches it to disk) when neither has coverage
- **Persistence:** browser `localStorage` for the saved-anime library
  (`src/library.ts`) — deliberately not committing to the sqlite-vs-flat-file
  backend store decision below, which is still open

## Workspace layout

```
nyaa_stream/
  Cargo.toml                 workspace root
  src-tauri/                 Tauri app crate (commands, window, app state)
  crates/
    torrent-engine/          librqbit wrapper + local streaming HTTP server
    nyaa-client/              nyaa.si search client (paginated HTML scrape)
    anilist-client/          AniList GraphQL client
    kitsu-client/            Kitsu API client (backdrop + episode thumbnails)
    mpv-ipc/                 spawns mpv, talks JSON IPC (pause/seek/volume)
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
  reference/stremio-web/     reference-only clone, gitignored
```

## Data flow (search → browse → play)

1. User types in the top search bar → debounced `search_anime` (AniList)
   fills a live dropdown (max 10 results, real ratings/format/season).
2. Picking a result opens the media page (`MediaPage.tsx`) and, in
   parallel: `search_torrents_for_anime` searches nyaa.si using the
   English title first, falling back to romaji if too few results
   (candidate titles are punctuation-sanitized — a raw AniList curly
   apostrophe was found to drop nyaa.si matches from 75 to 1); and
   `get_anime_details` lazily fetches the fuller AniList record (synopsis,
   `streamingEpisodes` thumbnails) that the lightweight dropdown search
   doesn't request.
3. Each nyaa.si result is parsed by `episodeParser.ts` into season/episode
   or batch (deterministic regex, not fuzzy matching — verified against a
   150-title real-world scrape). Batches with an explicit episode range in
   the title are spread across the specific episodes they cover; ambiguous
   titles fall back to `get_torrent_details_batch`, which scrapes each
   torrent's own nyaa.si view page for its real file count (batch or not)
   and submitter — only for titles the regex couldn't already resolve, to
   keep request volume down.
4. The media page's docked panel lists one row per episode (stremio-web's
   real `VideosList` pattern), not a flat list of raw torrent releases —
   picking a specific source is deferred to a future video-player dropdown,
   not built yet.
5. Saving an anime (`library.ts`, `localStorage`) makes it appear on the
   home page's "Library" grid and folds its recent episodes (AniList's
   batched `airingSchedules` query, this calendar week + last week)
   into the "Latest Episodes" row.
6. Playback: the media page's play button hands `PlayerView.tsx` the full
   list of that episode's releases; it auto-picks the one with the most
   seeders (`releases.ts`'s `bestRelease`) and calls `play_magnet`, which
   adds it to the librqbit session and returns a `stream_url` from the
   local streaming server. The frontend plays that URL directly in a plain
   HTML5 `<video>` element — a PotPlayer-style bottom control bar
   (play/pause, seek, volume, fullscreen, time, a source-picker dropdown
   over the same release list) fades in on mouse movement and auto-hides
   after idle, all driven by the browser's native video API (no IPC round
   trip for pause/seek/volume). Picking a different source just calls
   `play_magnet` again - its defensive cleanup tears down the previous
   torrent. Buffering feedback and the statistics panel port stremio-web's
   real Player UI (`Buffering.tsx`/`StatisticsMenu.tsx`/
   `loadingProgress.ts`, verified against `reference/stremio-web` and
   `reference/stremio-core` rather than designed from scratch) polling
   `get_stream_stats` every second. Closing the player or navigating away
   calls `stop_playback` to remove the torrent (stop seeding, drop partial
   files).

   **`mpv`/`mpv-ipc` is no longer the real playback engine** — an earlier
   attempt embedded mpv into the app window via `--wid` and a transparent
   webview background, but `transparent: true` turned out to break all
   click input app-wide on this Tauri/WebView2/Windows combination (a
   known upstream bug, not something fixable from app code). `mpv-ipc` and
   its `MpvPlayer` are kept only for headless thumbnail capture
   (`capture_torrent_thumbnail`), where no window/transparency is involved.
   CLAUDE.md's "player is the user's system mpv" line is now stale for
   real playback; still accurate for the thumbnail-capture path.

## Known gaps / not yet implemented

- `play_magnet` currently always streams file index `0` — needs real file
  selection when a torrent contains multiple files (e.g. batch releases).
- Playback is limited to whatever codecs/containers the WebView2/Chromium
  engine can decode natively (e.g. no built-in HEVC in some builds) — a
  real regression from the mpv-based approach's much broader format
  support, accepted as the cost of avoiding the transparency bug above.
- No watch history / continue-watching (the library only tracks *which*
  anime are saved, not watch progress).
- Batches without an explicit episode range in their title (most of them)
  can't be attributed to specific episodes and stay in an undifferentiated
  per-season "Batch" bucket.
- Torrent-captured thumbnails only ever use file index `0` and a fixed
  8-second seek point — same file-selection gap as playback itself.

See `PHASES.md` for the build order.
