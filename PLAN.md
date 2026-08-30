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
- **Torrent engine + streaming server:** `librqbit` (Session + Api), fronted
  by our own `axum` HTTP server with two layers: raw Range-capable file
  bytes (`axum-range`), and an HLS layer on top of that (`ffmpeg`-produced
  playlist + on-demand segments) that real playback actually uses, since
  raw torrent bytes aren't reliably playable in a browser `<video>`
  (system `ffmpeg`/`ffprobe`, also not bundled, must be on PATH)
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
    torrent-engine/          librqbit wrapper + local streaming/HLS HTTP server
    nyaa-client/              nyaa.si search client (paginated HTML scrape)
    anilist-client/          AniList GraphQL client
    kitsu-client/            Kitsu API client (backdrop + episode thumbnails)
    mpv-ipc/                 spawns headless mpv for thumbnail capture only
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
  reference/stremio-web/     reference-only clone, gitignored
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
   removing noise.
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
   to the librqbit session (waiting on `wait_until_initialized()` so the
   torrent's file list is actually queryable before returning - a magnet's
   metadata arrives from peers asynchronously, and a request made
   immediately after `add()` returned used to 404) and returns an HLS
   playlist URL from the local streaming server. The frontend plays that
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
   covers it. Picking a different source just calls
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
- Torrent-captured thumbnails only ever use file index `0` and a fixed
  8-second seek point — same file-selection gap as playback itself.
- Some fansub groups number releases *absolutely* across a whole franchise
  (e.g. `[Kaizoku] Jujutsu Kaisen - 25 ... (Season 2)`, where 25 is that
  show's overall episode count, not "episode 25 of season 2") with no
  season-relative number anywhere in the title to fall back on -
  `episodeParser.ts` has no way to recover the real within-season number
  from title text alone, so these land in their own bogus single-release
  "episode" bucket instead of merging with the season-relative releases
  for the same actual episode. Verified live across multiple shows/groups
  (Jujutsu Kaisen/Kaizoku, Spy x Family, That Time I Got Reincarnated as a
  Slime/Doomdos) - a real, recurring pattern, not a one-off. Fixing this
  properly needs each season's cumulative episode-count offset (from
  AniList) threaded into the parser, which `parseEpisode(title)` doesn't
  have access to today - not attempted yet.

See `PHASES.md` for the build order.
