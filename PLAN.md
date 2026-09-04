# PLAN.md — nyaa-stream

Anime-focused Stremio-like desktop client. Not a full Stremio addon-ecosystem
clone — a focused tool that searches nyaa.si for torrents, matches them
against AniList metadata, and streams them via HLS into an in-app HTML5
`<video>` element (see "Playback" below - `mpv` is used only for headless
thumbnail capture now, not real playback).

## Reference

`reference/stremio-core/` — shallow clone of https://github.com/Stremio/stremio-core,
kept for architecture reference only (gitignored, not a dependency). Useful
modules: `src/types/streaming_server.rs`, `src/models/streaming_server.rs`
(local streaming server model), `src/types/resource.rs` (catalog/stream
aggregation patterns), `src/types/resource/meta_item.rs` (confirms
`Video.series_info` is always `{season, episode}` — validates defaulting
unlabeled releases to season 1 rather than leaving season unknown).

`reference/stream-server/` — clone of https://github.com/stremio-native/stream-server,
kept for reference (gitignored) - but unlike the other two `reference/`
clones, this one is also the source of a real build dependency: the
`enginefs` crate torrent-engine depends on is a pinned git dependency
pointing at this same repo (not this local clone - see
`crates/torrent-engine/Cargo.toml`), and this project's own
`vcpkg.json`/`triplets/x64-windows-v3-static-md-release.cmake` were copied
from this clone's own build config. Useful for understanding `enginefs`
internals beyond its public API (e.g. `enginefs/src/backend/libtorrent/
playback.rs`'s `LibtorrentPlaybackCoordinator`, which owns real hot-file
piece-priority scheduling) since its own doc comments are the only
documentation for a lot of this.

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
- **Player:** an in-app HTML5 `<video>` element driven by `hls.js`
  (`src/PlayerView.tsx`) - not `mpv`; see "Playback" below for why and
  PLAN.md's Known gaps for the history. System `mpv` (must be on PATH — not
  bundled) is still used, but only for headless thumbnail capture
  (`crates/mpv-ipc`)
- **Torrent engine:** `enginefs`'s libtorrent backend (vendored via git
  dependency from https://github.com/stremio-native/stream-server, pinned
  to a specific commit - see `crates/torrent-engine/Cargo.toml`), replacing
  an earlier librqbit-based implementation. Chosen for its real per-file
  hot-piece prioritization (a batch torrent's actively-watched episode gets
  the swarm's attention, others don't), RTT-ranked tracker probing, and a
  working `remove_torrent` - librqbit's equivalent methods in the same
  `enginefs` crate turned out to be mostly stub/no-op implementations, only
  the libtorrent backend actually does this work (verified by reading its
  source before adopting it). Building this crate now requires a C++
  toolchain: CMake, MSVC, and `vcpkg` (see "Build prerequisites" below) -
  `enginefs` compiles libtorrent-rasterbar + OpenSSL from source via a
  vcpkg manifest (`vcpkg.json`/`triplets/` at the project root, copied from
  stream-server's own build config) the first time it's built.
  Torrents are keyed by info-hash `String` (`torrent_engine::TorrentId`),
  not a numeric session id like librqbit's - this ripples into
  `PlaySession.torrentId`/`get_stream_stats` on the frontend, which just
  treat it as an opaque id.
  Storage is disk-backed (`LibtorrentBackend::new_disk_backed`, not
  `enginefs`'s memory-only mode) so partial downloads still survive an app
  restart, matching the previous librqbit-based behavior.
  **Known regression:** unlike the previous librqbit-based `remove()`,
  `enginefs`'s libtorrent `remove_torrent` does not delete a torrent's
  downloaded files from disk (verified in its vendored source - it calls
  libtorrent's own removal with `delete_files = false`) - only our own HLS
  transcode cache is guaranteed cleaned up on `remove()` today.
- **Streaming server:** our own `axum` HTTP server (unchanged by the
  torrent-engine swap above - it talks to the torrent engine only through
  `enginefs`'s `Engine`/`TorrentHandle` API, not to librqbit or libtorrent
  directly) with two layers: raw Range-capable file bytes (`axum-range`),
  and an HLS layer on top of that (`ffmpeg`-produced playlist + on-demand
  segments, one continuous stream-copied (`-c:v copy`) process per file)
  that real playback actually uses, since raw torrent bytes aren't reliably
  playable in a browser `<video>` (system `ffmpeg`/`ffprobe`, also not
  bundled, must be on PATH). Deliberately did **not** adopt `enginefs`'s own
  HLS module (`hls.rs`) - it always re-encodes video (no stream-copy path)
  and spawns one `ffmpeg` process per segment, which is the design this
  project already tried and moved away from (see `torrent-engine/src/lib.rs`'s
  `HlsJobs` doc comment for the documented audio-discontinuity/muxer-error
  history). The two layers are decoupled by design - `enginefs`'s HLS
  module could be swapped in later without touching the torrent engine, or
  vice versa.

### Build prerequisites (new, added with the libtorrent backend)

Building `torrent-engine` (and therefore the whole workspace) now additionally
requires, beyond Rust/Node:
- CMake
- A working MSVC C++ toolchain (Visual Studio 2022 Build Tools or full IDE)
- [`vcpkg`](https://github.com/microsoft/vcpkg), bootstrapped, with
  `VCPKG_ROOT` pointed at it. On the dev machine this is `C:\vcpkg` - this
  is a machine-local path, not something this repo can fully automate; a
  fresh clone needs `vcpkg` bootstrapped once before its first
  `cargo build`.
- The first build compiles libtorrent-rasterbar 2.1.1 + OpenSSL from source
  via `vcpkg install` against this project's `vcpkg.json`/`triplets/`
  (took ~10 minutes on the dev machine; cached by vcpkg afterward).
- `.cargo/config.toml` at the project root sets `target-cpu=x86-64-v3`
  (Haswell/2013+ CPUs) for both Rust and the vendored C++ code, matching
  stream-server's own build config - this is a real minimum CPU
  requirement for anyone building or running this app, not just a compiler
  hint.
- **Torrent source:** nyaa.si search, scraping its paginated HTML results
  table (its RSS feed was found to silently ignore the `p=` page param and
  always cap at 75 results — see `crates/nyaa-client`)
- **Metadata:** AniList GraphQL API (`https://graphql.anilist.co`), no auth
  required for public queries; Kitsu API (`crates/kitsu-client`) as a
  secondary source for wide backdrop banners and per-episode thumbnails,
  resolved from an AniList id via Kitsu's crowdsourced mapping table
- **Thumbnails:** backdrop/episode art falls back Kitsu → AniList
  `streamingEpisodes` → a torrent-captured frame (`capture_torrent_thumbnail`
  spawns a headless mpv against the episode's stream, seeks it to roughly
  the episode's midpoint using the same AniList duration estimate the HLS
  playlist uses, grabs one frame, caches it to disk) when neither has
  coverage - falls back to a fixed early point when no duration estimate
  is available, since waiting for real-time playback to reach an actual
  multi-minute midpoint isn't practical within the capture's timeout
- **Persistence:** browser `localStorage` for the saved-anime library
  (`src/library.ts`) — deliberately not committing to the sqlite-vs-flat-file
  backend store decision below, which is still open

## Workspace layout

```
nyaa_stream/
  Cargo.toml                 workspace root
  vcpkg.json                 vcpkg manifest (libtorrent + openssl, for torrent-engine)
  triplets/                  custom vcpkg triplet (x64-windows-v3-static-md-release)
  .cargo/config.toml         vcpkg env vars + target-cpu=x86-64-v3 rustflags
  src-tauri/                 Tauri app crate (commands, window, app state)
  crates/
    torrent-engine/          enginefs (libtorrent backend) wrapper + local streaming/HLS HTTP server
    nyaa-client/              nyaa.si search client (paginated HTML scrape)
    anilist-client/          AniList GraphQL client
    kitsu-client/            Kitsu API client (backdrop + episode thumbnails)
    mpv-ipc/                 spawns headless mpv for thumbnail capture only
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
  reference/stremio-web/     reference-only clone, gitignored
  reference/stream-server/   reference-only clone, gitignored - source of the vendored
                              `enginefs` git dependency + vcpkg.json/triplets/ above
```

## Data flow (search → browse → play)

1. User types in the top search bar → debounced `search_anime` (AniList)
   fills a live dropdown (max 10 results, real ratings/format/season).
2. Picking a result opens the media page (`MediaPage.tsx`) and, in
   parallel: `search_torrents_for_anime` searches nyaa.si using both the
   English and romaji titles and merges the results, deduplicated by view
   URL (candidate titles are punctuation-sanitized — a raw AniList curly
   apostrophe was found to drop nyaa.si matches from 75 to 1). Both titles
   are always searched, not English-first-with-a-romaji-fallback as this
   used to work: verified live that many fansub groups title releases in
   romaji only with no English cross-reference text at all, and nyaa.si's
   per-word AND-matching tokenizer means a real show's English search
   almost never fell back in the old scheme anyway - together this
   silently dropped roughly half of "That Time I Got Reincarnated as a
   Slime"'s real releases regardless of how many pages got fetched. Also
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
   keep request volume down. `App.tsx`'s `groupedSources` then drops any
   result whose own parsed season doesn't match the currently-browsed
   anime's season (from its own AniList title, via the same
   `extractSeasonNumber`) - nyaa.si's search isn't a strict phrase match,
   verified live that browsing a specific numbered season (e.g. "That Time
   I Got Reincarnated as a Slime Season 4") still returns plenty of other
   seasons' releases too, which would otherwise show up mixed into that
   season's own episode list. Only applied when the browsed anime's own
   title actually names a season >1 - an unnumbered "season 1" is exactly
   extractSeasonNumber's ambiguous default for a genuinely unparseable
   title too, so filtering there would risk hiding real matches instead of
   removing noise. It also corrects releases numbered *absolutely* across
   a whole franchise instead of relative to the season being browsed (e.g.
   `[Kaizoku] Jujutsu Kaisen - 25 ... (Season 2)`, where 25 is that show's
   overall episode count, not "episode 25 of season 2" - `episodeParser.ts`
   has no way to tell from title text alone, verified live across multiple
   shows/groups as a real, recurring pattern) using
   `get_absolute_episode_offset` (`AniListClient::cumulative_prequel_episodes`
   in `crates/anilist-client`): walks the AniList relations graph's
   PREQUEL chain backward from the browsed season, summing prior seasons'
   episode counts (skipping through, but not counting, an OVA/movie/special
   prequel that itself further PREQUELs a real season - verified live that
   Slime's own relations graph has exactly this gap between its Season 2
   and Season 1 entries) to recognize and rewrite an absolute episode
   number back to its real season-relative one before grouping.
4. The media page's docked panel lists one row per episode (stremio-web's
   real `VideosList` pattern), not a flat list of raw torrent releases —
   picking a specific source is deferred to a future video-player dropdown,
   not built yet.
5. Saving an anime (`library.ts`, `localStorage`) makes it appear on the
   home page's "Library" grid and folds its recent episodes (AniList's
   batched `airingSchedules` query, this calendar week + last week)
   into the "Latest Episodes" row.
6. Playback: the media page's play button (or clicking an episode directly
   in the home page's "Latest Episodes" row, which jumps straight into the
   player via `MediaPage.tsx`'s `autoplayEpisode` prop instead of leaving
   the user on the episode list) hands `PlayerView.tsx` the full list of
   that episode's releases; it auto-picks the one with the most seeders
   (`releases.ts`'s `bestRelease`) and calls `play_magnet`, which adds it
   to the `enginefs` libtorrent engine and returns an HLS playlist URL from
   the local streaming server. **Known gap from the librqbit→libtorrent
   swap**: the previous implementation explicitly waited on librqbit's
   `handle.wait_until_initialized()` before returning, since a magnet's
   metadata arrives from peers asynchronously and a request made
   immediately after `add()` used to 404 without that wait. `enginefs`'s
   `BackendEngineFS::add_torrent` has no equivalent documented wait step -
   this hasn't yet been verified live to confirm the race doesn't
   reappear; test a fresh magnet add → immediate play before considering
   this fully done. The frontend plays that
   via `hls.js` in a plain HTML5 `<video>` element — a solid-black bottom
   control bar (play/pause, seek with a download-progress highlight,
   mute, volume, fullscreen, time, a source-picker dropdown over the same
   release list) that only appears while the mouse is directly over it
   (a dedicated invisible hover zone the same size as the bar, not "any
   mouse movement over the video" the way this used to work - see
   PlayerView.tsx), plus PotPlayer/YouTube-style keybinds (space/K
   play-pause, arrows/J/L seek, up/down volume, M mute, F fullscreen, Esc
   close) that work whether or not the bar is currently shown. Seeking is
   a plain `video.currentTime` set - hls.js fetches whichever segment
   covers it, configured with longer-than-default fragment-load
   timeouts/retries and a fatal-error recovery handler (`hls.startLoad()`/
   `recoverMediaError()`, hls.js's own recommended pattern, capped at 8
   attempts to avoid retrying forever on a fault recovery can't actually
   fix) - verified live that hls.js's own default fragment timeout was
   shorter than torrent-engine's segment-wait budget, so it was declaring
   a *fatal* error and killing playback outright before the server would
   have actually delivered a slow segment. Picking a different source just calls
   `play_magnet` again - its defensive cleanup tears down the previous
   torrent. Buffering feedback and the statistics panel port stremio-web's
   real Player UI (`Buffering.tsx`/`StatisticsMenu.tsx`/`loadingProgress.ts`,
   verified against `reference/stremio-web` and `reference/stremio-core`
   rather than designed from scratch) polling `get_stream_stats` every
   second. Closing the player or navigating away calls `stop_playback` to
   remove the torrent (stop seeding, drop partial files).

   **Raw torrent bytes aren't served directly to the browser** - verified
   live that real anime releases (MKV, H.264 video, E-AC-3 audio) fail in
   WebView2's `<video>` with `MEDIA_ERR_SRC_NOT_SUPPORTED` even though
   `canPlayType` reports "probably" for every codec involved. The actual
   blocker is structural: Matroska's seek index (Cues/SeekHead) - and
   often its declared duration too - is commonly placed at/near the *end*
   of the file, which an incrementally-downloading torrent can't provide
   up front, and a browser's own duration-scanning has the identical
   problem. **HLS solves this properly** (replacing an earlier "remux the
   whole episode through ffmpeg, restart on every seek" approach, which
   worked but leaked `ffmpeg.exe` processes under rapid seeking since
   closing the client side alone doesn't reliably kill it on Windows):
   `torrent_engine::hls_playlist_handler` serves a VOD `.m3u8` built from a
   duration the *frontend* supplies (AniList's per-episode runtime -
   the backend can't reliably determine this itself, for the same
   Cues-at-the-end reason above). Segments themselves are produced by
   `HlsJobs`: a single continuous `ffmpeg` process per torrent file
   (`-c:v copy`, audio always transcoded to `aac` since MPEG-TS/browsers
   don't reliably handle several codecs real releases use, e.g. E-AC-3;
   `-map 0:v:0 -map 0:a:0 -sn` to drop any embedded subtitle/attachment
   streams, which MPEG-TS can't carry and which otherwise got auto-included
   and corrupted timestamps) reading from `stream_handler` over loopback,
   writing real segment files to disk (`-f hls -hls_flags temp_file` -
   atomic rename on completion, so a request never sees a half-written
   file). A segment request waits for that job's sequential progress to
   reach it, or restarts the job at a new offset only when the request is
   a real seek (far from current progress), not ordinary buffering.
   Replaced an earlier "one `ffmpeg -ss`/`-t` invocation per segment"
   design: each independent process reinitialized its own AAC encoder and
   timestamp timeline from zero, producing audible artifacts at every
   segment boundary and, combined with the embedded-subtitle issue above,
   occasional outright muxer failures that dropped whole segments
   (verified live). A single continuous process per file has one
   timestamp timeline for the whole episode and its sequential HTTP reads
   line up with librqbit's own sequential piece-priority download
   strategy instead of fighting it with scattered probe reads.

   **`mpv`/`mpv-ipc` is no longer the real playback engine** — an earlier
   attempt embedded mpv into the app window via `--wid` and a transparent
   webview background, but `transparent: true` turned out to break all
   click input app-wide on this Tauri/WebView2/Windows combination (a
   known upstream bug, not something fixable from app code). `mpv-ipc` and
   its `MpvPlayer` are kept only for headless thumbnail capture
   (`capture_torrent_thumbnail`), where no window/transparency is involved.
   CLAUDE.md's "player is the user's system mpv" line is now stale for
   real playback; still accurate for the thumbnail-capture path.

   **Torrent downloads now live under AppData/cache** (`dirs::cache_dir()`
   joined with `nyaa-stream/downloads`), not the user's actual Downloads
   folder - this is app-owned scratch data cleaned up via `stop_playback`,
   not something the user is meant to keep or browse to directly.

## Known gaps / not yet implemented

- `play_magnet` currently always streams file index `0` — needs real file
  selection when a torrent contains multiple files (e.g. batch releases).
- The HLS playlist's declared duration is only as good as the frontend's
  AniList-derived estimate (falls back to a generic 24-minute guess if
  even that's missing) - not the file's real, exact length. Good enough
  for a working seek bar; the last segment may be trimmed slightly short
  or read past EOF if the estimate is off by more than a few seconds.
- No watch history / continue-watching (the library only tracks *which*
  anime are saved, not watch progress).
- Batches without an explicit episode range in their title (most of them)
  can't be attributed to specific episodes and stay in an undifferentiated
  per-season "Batch" bucket.
- Torrent-captured thumbnails only ever use file index `0` — same
  file-selection gap as playback itself. The seek point targets roughly
  the episode's midpoint when a duration estimate is available (a fixed
  early point otherwise), but without a Matroska Cues index on a
  still-downloading torrent that seek is best-effort and can land short
  of the target or occasionally not resolve in time, same tradeoff as
  torrent-engine's HLS segment restarts.
See `PHASES.md` for the build order.
