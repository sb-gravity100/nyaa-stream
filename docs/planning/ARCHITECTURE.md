# Architecture and development reference

Current roadmap: [PLAN.md](../../PLAN.md). File lookup: [FILE_INDEX.md](../../FILE_INDEX.md).

Implementation descriptions and historical measurements are retained below. The
legacy gap inventory at the end needs code verification before treating an item
as an active defect; current milestones live in PLAN.md and PHASES.md.

## Reference

`reference/stremio-core/` — shallow clone of https://github.com/Stremio/stremio-core,
kept for architecture reference only (gitignored, not a dependency). Useful
modules: `src/types/streaming_server.rs`, `src/models/streaming_server.rs`
(local streaming server model), `src/types/resource.rs` (catalog/stream
aggregation patterns), `src/types/resource/meta_item.rs` (confirms
`Video.series_info` is always `{season, episode}` — validates defaulting
unlabeled releases to season 1 rather than leaving season unknown).

`reference/stream-server/` — clone of https://github.com/stremio-native/stream-server
(gitignored, reference only; not on the dev machine any more). Its
`enginefs` crate was nyaa's torrent layer until v0.4.0 (vendored, see
"sbtl_engine"); `vcpkg.json`/`triplets/` were first copied from its build
config, and `crates/sbtl-engine`'s `trackers.rs`/`tracker_prober.rs` are
ported from it (MIT). No build dependency remains.

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
- **Player:** libmpv (`libmpv-2.dll`, a GPL build, bundled from
  `src-tauri/lib/` and loaded at runtime with `libloading`, so a missing DLL
  only disables it) running in-process, embedded in the app window via
  `wid` (`crates/mpv-player`'s `EmbeddedMpv`, `src-tauri/src/player.rs`),
  driven by the HTML controls in `src/PlayerView.tsx` through
  `src/mpvVideo.ts` with mpv-IPC-shaped JSON commands/events. Fallback
  without libmpv: the HLS `<video>` + `hls.js` player
  (`src/HlsPlayerView.tsx`). Thumbnail frames use the in-process FFmpeg
  pipeline over direct seekable sbtl readers
- **Torrent engine:** sbtl (https://github.com/sb-gravity100/sbtl, a C
  library with Rust bindings, built for nyaa-stream) through
  `crates/sbtl-engine` (see "sbtl_engine"), which replaced the vendored
  `enginefs` from stream-server. sbtl's readers drive the download: a
  reader's position is the playback cursor, so seeks preempt queued work
  and no playback coordinator is needed - see "sbtl backend". **The only
  engine since 2026-10-04:** history - nyaa first ran its own librqbit
  session, then `enginefs`'s libtorrent backend (chosen for real per-file
  hot-piece priority; `enginefs`'s librqbit backend was mostly stubs), then
  sbtl, which seeks faster - first as an enginefs backend, then through
  its own `sbtl_engine`. libtorrent, librqbit, `vendor/libtorrent-sys`,
  `vendor/enginefs` and the engine build features were removed;
  sections below that describe libtorrent behaviour (e.g. "Fast playback
  start") are history.
  Torrents are keyed by info-hash `String` (`torrent_engine::TorrentId`) -
  `PlaySession.torrentId`/`get_stream_stats` on the frontend treat it as an
  opaque id. Storage is disk-backed (`downloads/`, plus sbtl's state in
  `downloads/.sbtl/`), so partial downloads survive an app restart; a
  torrent's `remove()` keeps its files (the download cache decides when
  they go).
- **Streaming server:** our own `axum` HTTP server (it talks to the torrent
  engine only through `sbtl_engine`'s `Engine`/`Torrent`/`Reader` API,
  never to sbtl directly) with two layers: raw Range-capable file bytes (`axum-range`),
  and an HLS layer on top of that (FFmpeg-produced segments, one
  continuous run per file) that real playback actually uses, since raw
  torrent bytes aren't reliably playable in a browser `<video>`. FFmpeg
  runs **in-process** (`crates/torrent-engine/src/media.rs`: ez-ffmpeg for
  the HLS/subtitle pipelines, ffmpeg-next for probing and font
  extraction), statically linked from FFmpeg 7.1.2 built by our
  `vcpkg.json` - LGPL features only (avcodec/avformat/avdevice/avfilter/
  swresample/swscale, nvcodec, qsv, amf, openh264, dav1d; no x264, which
  would make the app GPL). It replaced spawning the `ffmpeg`/`ffprobe`
  CLI: no console windows in the GUI-subsystem release build, no PATH
  dependency, FFmpeg's own log routed into our tracing output, jobs
  stopped with `abort()`. Accepted tradeoff: a libav crash on a malformed
  file now takes down the app rather than one child process. Deliberately did **not** adopt `enginefs`'s own
  HLS module (`hls.rs`) - it always re-encodes video (no stream-copy path)
  and spawns one `ffmpeg` process per segment, which is the design this
  project already tried and moved away from (see `torrent-engine/src/lib.rs`'s
  `HlsJobs` doc comment for the documented audio-discontinuity/muxer-error
  history). The two layers are decoupled by design: the HLS layer reads
  the torrent engine only through readers.

### Build prerequisites

**No vcpkg needed to build:** `npm run setup` (`scripts/fetch-native-deps.mjs`)
downloads the prebuilt static FFmpeg tree (release asset `native-deps-v1`,
SHA-256 checked; v1 also holds libtorrent/OpenSSL from before sbtl, unused)
into `vcpkg_installed/`, which `FFMPEG_DIR` reads directly. vcpkg is only
needed to *rebuild* that archive after changing `vcpkg.json` (then
re-upload it as a new `native-deps-vN` release and bump the constants in
the script). CMake and vcpkg bullets below describe that rebuild path;
MSVC and LLVM are still required to build.

Building the workspace requires, beyond Rust/Node:
- A working MSVC toolchain (Visual Studio 2022 Build Tools or full IDE)
- LLVM: bindgen needs libclang. sbtl itself is linked as the prebuilt
  `native/sbtl/sbtl.dll` (see "sbtl versions and updates"), so `clang-cl`
  is only needed to rebuild that DLL or when `SBTL_PREBUILT_DIR` is unset
  (`sbtl-sys`'s build.rs then compiles the C sources, falling back to MSVC
  `cl.exe /std:c11`)
- Network access to github.com on the first build: cargo fetches sbtl
  (public repo, tagged) - see "sbtl backend" for how its versions are
  pinned
- Rebuilding `native-deps` only: CMake and
  [`vcpkg`](https://github.com/microsoft/vcpkg), bootstrapped, with
  `VCPKG_ROOT` pointed at it (`C:\vcpkg` on the dev machine).
- `vcpkg install` builds FFmpeg 7.1.2 (LGPL feature set, see
  the streaming-server section; ~6 minutes, cached afterward).
  `ffmpeg-sys-next` finds it through `FFMPEG_DIR` (set in
  `.cargo/config.toml` to `vcpkg_installed/<triplet>` - the `vcpkg` crate
  it would otherwise use only understands classic-mode installs), and
  `crates/torrent-engine/build.rs` links FFmpeg's static dependencies.
  bindgen needs LLVM's libclang (`LIBCLANG_PATH`, default
  `C:\Program Files\LLVM\bin` in the same config).
- `.cargo/config.toml` at the project root sets `target-cpu=x86-64-v3`
  (Haswell/2013+ CPUs) for both Rust and the C code built from source (sbtl, FFmpeg), matching
  stream-server's own build config - this is a real minimum CPU
  requirement for anyone building or running this app, not just a compiler
  hint.
- **Release database:** every nyaa.si release seen is stored permanently in a
  local SQLite database (`nyaa.db` in the app data dir, `nyaa-client/src/store.rs`)
  keyed by nyaa's release id (`/view/<id>`), so overlapping queries never
  duplicate a torrent. A show page paints in phases: `search_local_releases`
  (database only, instant) → `search_fansubber_releases` (only the Settings
  preferred fansubber, ~1 request) → the full `search_torrents_for_anime`,
  which replays a fresh stored query with no request and otherwise pages only
  until it reaches releases already stored. All requests share one throttle
  (`throttle.rs`: 2 in flight, paced, 429/503 back-off with `Retry-After`).
  The database is user data - "Clear cache" leaves it alone.
- **Batch playback:** a batch source plays only its matched episode file (each
  torrent keeps an idle sbtl stream open, so files nobody reads don't
  download). While it plays, the next episode's file is registered as a
  *preload file* (`preload_next_file` → `sbtl_engine`'s `set_preload_file`, a
  reader-less sbtl stream behind everything else) so it only takes spare
  bandwidth; the next episode keeps using the same batch and torrent
  (`lastBatchMagnet`, `play_magnet` reuse, 4s deferred `stop_playback`).
- **Torrent source:** nyaa.si search, scraping its paginated HTML results
  table (its RSS feed was found to silently ignore the `p=` page param and
  always cap at 75 results — see `crates/nyaa-client`); responses are
  cached on disk (`nyaa-client/src/cache.rs`) because nyaa.si rate-limits:
  searches reused for 30 min and served stale when a refetch fails (a 429
  is an error, never an empty result), view-page details kept forever.
  **Main filter (since 2026-10-08):** `?f=0&c=1_0&q=` - no trusted/remake
  filter, category Anime (all: English-translated, raw, non-English), i.e.
  `Category::AllAnime`; it replaced `c=1_2` (English-translated) in every
  app search and in the browser RSS fallback. Cache and store entries are
  keyed by category code, so releases stored under `1_2` aren't found by
  `1_0` searches until re-fetched. Target version: next patch (bump pending)
  **Sort (user 2026-10-17):** every app search page adds `&s=seeders&o=desc`
  (`?f=0&c=1_0&q=<query>&s=seeders&o=desc&p=<n>`, also `/user/<name>?...`).
  Why: nyaa.si returns at most 1,000 results per query, newest first by
  default, so a long show's early-season releases (e.g. Slime S1, 2018-19) sat
  beyond the cap and were never fetched (seen 2026-10-16: 1,000-result queries
  reached back only to 2021); seeders-descending fills the cap with the
  releases people actually download. Consequences handled with it: (1) the
  database's early stop ("first page fully stored means everything older was
  seen") only holds for newest-first order, so a seeders-sorted fetch pages
  to the end (up to `MAX_SEARCH_PAGES`) instead; (2) the search cache and
  database key gain `|seeders` so answers fetched newest-first are not
  replayed as seeder-sorted ones (they are re-fetched once); (3) merged
  results stay newest-first by release id (`merge_newest_first`), the UI
  ranks by its own rules. Trade-off: a brand-new release with no seeders yet
  can fall outside the 1,000 for a very large franchise until it gains
  seeders. The browser RSS fallback (dev only) is unchanged. Target version:
  next patch (bump pending), with the main filter above.
  **Entry-specific search (done, v0.5.7 patch; user 2026-10-26).** Remote
  queries use nyaa.si exact quoted title phrases joined with `|`, plus
  season-qualified phrase alternatives. Same-season AniList sibling names add
  aliases for split cours and translated titles. TV searches do not query a
  bare shared base title. Unmarked titles shared with a different-season
  sibling are rejected unless a trusted AniDB id or distinctive full title
  identifies the entry. Season 1 uses exact names and season-1 forms with a
  negated group for sibling titles and season markers. Movie/OVA/special
  entries use their full entry names. Local release lookups retain plain
  word-query candidates, then use the same strict result matcher. The trusted
  `aid` bypass is intact.

  **Precision check (2026-10-27, `title_match_fixture_recall_and_precision`).**
  Query construction passes for all 1,275 AniList entries in the fixture,
  with no-TVDB and mapped-TVDB variants. Across the three fixture franchises,
  explicit Mushoku S3 titles keep 375/375, Food Wars S1 keeps 24/24, and Slime
  S4 Part 3 keeps 655/655 (up from 588/655 before same-season alias handling).
  The targeted Mushoku S3 and Slime S4 Part 3 checks have zero wrongly kept
  titles. Mushoku S1 keeps 24/37 title-labeled releases and Cour 2 keeps 24/37
  without a TVDB season or 28/37 with it. This recall loss is the chosen cost
  of excluding unmarked titles shared with earlier entries.

  **v0.5.8 patch follow-up (2026-10-29).** A live Slime Season 4 query
  returned HTTP 400 because exact-title alternatives formed an oversized OR
  expression. Remote queries now split at eight alternatives or 600
  characters, preserving exact phrase and exclusion semantics; results still
  merge and deduplicate. Logs record query bytes, branch count and HTTP status
  without printing full search URLs. Seek traces capture completion before
  collecting optional torrent stats, so stats latency no longer inflates the
  measured playback restart time; summaries include issued/retargeted seek
  targets and the restart position. E-AC-3/audio decoder errors remain
  separate from video/stream corruption and now log playhead, seek target and
  active audio-track context without triggering a buffer rebuild. Live
  verification on the same Slime search and a seek/audio session remains.
  Cursor visibility is independent of the hover controls: pointer movement
  within the player keeps the cursor visible and restarts its idle timer;
  moving outside control zones may hide the controls immediately, but the
  cursor hides only after the idle timeout.

  **v0.5.8 subtitle edge quality (2026-10-29).** The HLS fallback's JASSUB
  renderer now uses native device-pixel output with libass's per-pixel glyph
  coverage and no CSS blur or supersampling. JASSUB composes with WebGL when
  available and falls back to Canvas2D. Embedded mpv now uses
  `blend-subtitles=video`, EWA Lanczos sharp upscaling with blur 1.5, Mitchell
  downscaling, and `sub-blur=1.0` on its GPU output path. The scaler blur
  applies to the full image; `sub-blur` affects subtitle borders only.
  Video-stage blending can clip subtitles in letterbox areas. The HLS
  fallback keeps its subtitle-only renderer. Visual verification on
  standard/high-resolution HLS and mpv playback remains.
- **Metadata:** AniList GraphQL API (`https://graphql.anilist.co`), no auth
  required for public queries; Kitsu API (`crates/kitsu-client`) as a
  secondary source for wide backdrop banners and per-episode thumbnails,
  resolved from an AniList id via Kitsu's crowdsourced mapping table
- **Thumbnails:** provider art uses Kitsu → AniList `streamingEpisodes` →
  TMDB stills, then a torrent-captured frame when none has coverage.
  `capture_torrent_thumbnail` seeks directly through sbtl with in-process
  FFmpeg and stops after one frame; it uses AniList's episode duration for
  the midpoint or an 8s fallback. Nyaa release-description images are not
  fetched. Thumbnails are cached for good
  (they belong to one episode): captured frames on disk, checked first via
  `cached_torrent_thumbnail` before any nyaa search, and Kitsu metadata
  (`thumbnails/kitsu/<anilistId>.json`) reused for a day, stale on failure.
  Cached JPEGs reach `<img>` tags by URL through the `thumb` URI scheme
  (`http://thumb.localhost/<key>.jpg?v=<mtime>` on Windows, served
  immutable) - never base64 over IPC. Captures run one at a time, and a
  failed one leaves a `<key>.fail` marker that skips retries (and their
  nyaa search) for a day. Closing the player saves the frame on screen
  into the same cache (`save_frame_thumbnail`, raw JPEG body, cache key in
  an `x-cache-key` header); Continue watching cards prefer it
- **Persistence:** browser `localStorage` for the saved-anime library, watch progress (`watchProgress.ts`), settings (`settings.ts`) and per-anime preferred fansub group (`releases.ts`)
  (`src/library.ts`) — deliberately not committing to the sqlite-vs-flat-file
  backend store decision below, which is still open

## Workspace layout

```
nyaa_stream/
  Cargo.toml                 workspace root
  vcpkg.json                 vcpkg manifest (static LGPL FFmpeg, for torrent-engine)
  triplets/                  custom vcpkg triplet (x64-windows-v3-static-md-release)
  .cargo/config.toml         vcpkg env vars + target-cpu=x86-64-v3 rustflags

  src-tauri/                 Tauri app crate (commands, window, app state)
  crates/
    sbtl-engine/             torrent engine over sbtl: torrents, readers, stats, trackers, idle removal
    torrent-engine/          local streaming/HLS HTTP server over sbtl-engine
    nyaa-client/              nyaa.si search client (paginated HTML scrape, SQLite release store, EpisodeLabel)
    release-parse/           release title parser: MPL-2.0 port of anitopy + nyaa fixes
    anilist-client/          AniList GraphQL client
    kitsu-client/            Kitsu API client (backdrop + episode thumbnails)
    mpv-player/                 in-process libmpv for embedded playback
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
  reference/stremio-web/     reference-only clone, gitignored
  reference/stream-server/   reference-only clone, gitignored - where enginefs and
                              vcpkg.json/triplets/ came from
```

## Data flow (search → browse → play)

1. User types in the top search bar → debounced `search_anime` (AniList)
   fills a live dropdown (max 10 results, real ratings/format/season).
2. Picking a result opens the media page (`MediaPage.tsx`). `App.tsx`'s
   `pickAnime` caches both the nyaa.si search results and the per-torrent
   view-page scrape (`sourcesByMedia`/`detailsByMedia`, keyed by AniList id,
   never invalidated within a session - same pattern as `kitsuByMedia`/
   `episodeOffsetByMedia`) - revisiting an anime already browsed this
   session (back-and-forth from the library/latest-episodes, or picking the
   same search result twice) reuses that instantly instead of re-running the
   search and re-scraping every ambiguous title's view page from scratch.
   On a cache miss, in parallel: `search_torrents_for_anime` searches nyaa.si using both the
   English and romaji titles and merges the results, deduplicated by view
   URL (candidate titles are punctuation-sanitized — a raw AniList curly
   apostrophe was found to drop nyaa.si matches from 75 to 1). Both titles
   are always searched, not English-first-with-a-romaji-fallback as this
   used to work: verified live that many fansub groups title releases in
   romaji only with no English cross-reference text at all, and nyaa.si's
   per-word AND-matching tokenizer means a real show's English search
   almost never fell back in the old scheme anyway - together this
   silently dropped roughly half of "That Time I Got Reincarnated as a
   Slime"'s real releases regardless of how many pages got fetched. A third
   candidate, `strip_season_suffix`'s output, is also searched whenever the
   English/romaji title carries a trailing "Season N"/"Nth Season"/"Part
   N"/"Final Season" qualifier: that same per-word AND-matching tokenizer
   means a literal "Season 4" in the query only matches releases whose own
   title text also spells out "Season" and "4" - verified live that
   ToonsHub numbers Slime Season 4 as plain "S04E21" with no "Season" token
   anywhere, so the full-title query returned zero of its ~100 real Season
   4 releases even though the season-stripped "That Time I Got Reincarnated
   as a Slime" query finds every one of them. Also
   `get_anime_details` lazily fetches the fuller AniList record (synopsis,
   `streamingEpisodes` thumbnails) that the lightweight dropdown search
   doesn't request.
3. Each nyaa.si result is parsed in the backend by `crates/release-parse`
   (MPL-2.0 anitopy port + nyaa fixes, since v0.4.2 - see "Season-aware
   source matching") into an `EpisodeLabel` (episode / batch / movie /
   special / unknown) carried on the `NyaaResult` and on play files; the
   frontend reads it via `labelOf`, and `episodeParser.ts`'s regex parser
   remains only as the browser-preview fallback (until v0.4.2 it was the
   parser, verified against a 150-title scrape). Batches with an explicit episode range in
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
   number back to its real season-relative one before grouping. This
   absolute-offset correction runs *before* the season-mismatch drop above,
   not after: a release with no season marker in its title text at all
   (`extractSeasonNumber` then defaults its season to 1) would otherwise get
   dropped as "wrong season" noise before the offset math ever got a chance
   to recognize it as the browsed season's own episode. Once the offset
   math confirms a release's absolute number resolves to a valid in-season
   episode, its season is snapped to the browsed season so the drop check
   doesn't then remove it anyway.
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
   to the torrent engine (`sbtl_engine`) and returns an HLS playlist URL
   from the local streaming server. A magnet's metadata arrives from peers
   asynchronously; the sbtl backend's `add_torrent` waits for it (up to
   120 s, cached in `downloads/.sbtl/<hash>.torrent` afterwards) before
   returning, so a request right after `add()` doesn't 404. The frontend plays that
   via `hls.js` in a plain HTML5 `<video>` element — a solid-black bottom
   control bar (play/pause, seek with a download-progress highlight,
   mute, volume, fullscreen, time, a source-picker dropdown over the same
   release list) that appears on any mouse movement and fades out (with
   the cursor) after the mouse sits idle, unless the pointer rests over
   the top or bottom control areas - re-checked on every pointer move
   against the bars and their invisible hover zones, see PlayerView.tsx's
   CONTROLS_AREA_SELECTOR), plus player keyboard shortcuts (space/K
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
   and corrupted timestamps) reading the torrent directly through AVIO
   callbacks (`direct_input.rs`: reopen-on-early-end, cancellable reads so
   an abort never waits on an undownloaded piece), writing real segment files to disk (`-f hls -hls_flags temp_file` -
   atomic rename on completion, so a request never sees a half-written
   file). Segments are **fMP4** (`<index>.m4s` + one `init.mp4` per run,
   served by `hls_init_handler`; the playlist's `EXT-X-MAP` carries the
   resume segment as a start hint), not MPEG-TS: hls.js appends fMP4 to MSE
   without transmuxing in JavaScript, and copied HEVC needs it (tagged
   `hvc1`). The HLS muxer's paths are passed with forward slashes - it finds
   the init file's directory by splitting on `/` only. Aborting a run for a
   seek restart still finalizes its in-progress segment cut short;
   `HlsJob::kill` deletes that tail segment so it's never served as
   complete. A segment request waits for that job's sequential progress to
   reach it only if the job's measured production rate says it will get
   there within ~4s (`job_will_reach_soon`); otherwise it's a seek and the
   job restarts at the target (a fixed 20-segment lookahead used to make
   short forward seeks wait for everything in between to download). Every
   run uses `-copyts` so segments keep the
   source's own timestamps: without it a seek-restarted run re-based PTS to
   0 and hls.js placed it at the wrong point on the timeline. hls.js still
   anchors media time 0 to the first fragment it loads, so a mid-file start
   (resume) is off by up to one keyframe interval - the frontend reads
   hls.js's `INIT_PTS_FOUND` offset and corrects the time display, saved
   progress and subtitle timing with it. `StreamStats.readyRanges` reports
   every produced stretch (seek restarts leave several) for the seek bar.
   Replaced an earlier "one `ffmpeg -ss`/`-t` invocation per segment"
   design: each independent process reinitialized its own AAC encoder and
   timestamp timeline from zero, producing audible artifacts at every
   segment boundary and, combined with the embedded-subtitle issue above,
   occasional outright muxer failures that dropped whole segments
   (verified live). A single continuous process per file has one
   timestamp timeline for the whole episode and its sequential HTTP reads
   line up with librqbit's own sequential piece-priority download
   strategy instead of fighting it with scattered probe reads.

   **Subtitles are extracted as ASS by the HLS job itself and rendered
   with libass (JASSUB) in the frontend** - MPEG-TS can't carry them, and
   burning them in would fix one track/style at transcode time. The
   per-file media probe (`MediaProbes`, `ffprobe` on the container header,
   cached on success only) lists text subtitle tracks (bitmap PGS/VobSub
   are skipped - ffmpeg can't convert them and would fail the whole
   process) and font attachments. Each HLS transcode run adds one
   `-map 0:<index>` ASS output per **English** track (`eng`/`en`/`enm`;
   every track when none is English - other languages come from the
   full-file background pass), stream-copied for ASS sources, with
   `flush_packets` and `ignore_readorder` (the ass muxer otherwise holds
   every event after a ReadOrder gap until the file ends - Kaleido-subs'
   out-of-order scripts extracted one event during playback)
   (`sub_<index>_<startSegment>.ass`), so subtitles are extracted at the
   playhead alongside the video they belong to. This replaced a separate
   extraction process that read from byte 0 and a one-shot `<track>` WebVTT
   fetch: subtitles were missing, partial, or late after any forward seek.
   `subtitle_handler` serves each track from an append-only event log
   (`subtitle_log.rs`: run files read incrementally, `Dialogue:` lines
   de-duplicated - identical across runs thanks to `-copyts`); `?from=N`
   returns only events added since, with the total in `X-Subtitle-Events`.
   `assRenderer.ts` loads the script once, then every 3s feeds only new
   lines to libass (`processData`) instead of re-fetching and re-parsing
   the whole script (a 40 MB, ~77k-event Kaleido-subs track). libass renders
   at the output's native device-pixel size; JASSUB preserves its coverage
   masks in the WebGL composition, with Canvas2D fallback. Fonts are dumped
   once (`-dump_attachment`) and served at
   `/fonts/...` for libass. Plain (SRT/WebVTT) tracks become ASS with a
   single `Default` style that the user's default-subtitle-style setting
   rewrites - by default Crunchyroll's own dialogue style (Gandhi Sans bold,
   bundled in `src/assets/fonts`, taken from a ToonsHub CR WEB-DL's ASS) (`subtitles.ts`'s `applySubtitleStyle`); real ASS tracks keep
   their styling unless the user opts in, and then only dialogue-looking
   styles change. Z/X shift subtitle delay by 0.1s.

   **Video plan: copy or transcode.** Decided once per file
   (`HlsJobs::plan_for`) from the probe's video stream and what the
   frontend reported via `set_decoder_support` (MSE `isTypeSupported`):
   8-bit 4:2:0 H.264 (and HEVC where natively supported) is stream-copied;
   everything else - Hi10P H.264, HEVC, AV1, VP9... - is re-encoded to
   8-bit H.264 High with the first encoder that opens of NVENC > QSV >
   AMF > OpenH264 (`media::detect_h264_encoder` opens each on a tiny frame
   at startup), hwaccel "auto" decode, and IDR keyframes forced every 6s so segments
   sit exactly on the playlist grid (frame-accurate seeks). The cache
   directory's `video_mode` marker drops segments produced under a
   different plan. Verified live: 10-bit HEVC via NVENC at ~16x realtime,
   playable in 4.1s, 10-minute seek in 1.2s. `StreamStats.videoMode`
   drives the player's "Converting to H.264" chip.

   (enginefs's libtorrent disk reader once handed out zero bytes for
   pieces libtorrent had verified but not yet written, corrupting demux;
   sbtl reads only verified bytes through its own storage, so that guard
   went with libtorrent.) Video never waits on
   the probe: a run that starts before it finishes gets a subtitle-only
   ffmpeg attached (`HlsJobs::attach_subtitles`). A separate full-file
   subtitle pass (`sub_<index>_bg.ass`) reads `stream_handler` with
   `?intent=background` (no playback-lease refresh, an sbtl stream with no
   read-ahead beyond its window) so the whole track fills in as the torrent downloads
   without pulling priority from the playhead. hls.js `initPTS` is a raw
   33-bit PTS and is unwrapped before use (B-frame DTS just below zero
   wraps to ~95443s). A transcode whose ffmpeg exited is restarted on the
   next request, and every ffmpeg/ffprobe read uses `-reconnect` flags
   (an sbtl reader fails after its 60 s read timeout when no peer delivers
   a piece).

   **Embedded mpv playback (the default player).** The approach
   stremio-shell-ng uses: `mpv_start` starts an in-process libmpv with
   `wid=<app window>` (`--idle --force-window --no-config`, no OSC/input,
   `--hwdec=auto-safe`), finds the child window mpv creates (window
   class `mpv`) and pushes it to `HWND_BOTTOM` so the webview stays on top, then sets
   the *webview's* background to alpha 0 (`set_background_color`). The
   window itself is never `transparent: true` - that is what broke click
   input app-wide in the first embedding attempt; with only the webview
   background transparent, clicks work (verified with real OS clicks over
   both controls and bare video). `html.mpv-active` hides the rest of the
   app while the player is mounted. mpv opens the file's raw
   `stream_url` directly - no HLS, no transcoding, no codec checks - and
   renders embedded ASS/SRT subtitles and font attachments itself through
   libass (bundled Gandhi Sans is written to a `--sub-fonts-dir`). The
   user's subtitle style maps to `sub-*` options for plain tracks and, with
   "apply to styled", to `sub-ass-style-overrides` on the dialogue styles
   named in `sub-ass-extradata`. `MpvVideo` mirrors mpv properties
   (`observe_property` → `mpv-event`) behind an `HTMLVideoElement`-shaped
   facade and interpolates `time-pos` between updates. Thumbnails and
   Ctrl+C use `screenshot-to-file` (`mpv_frame` downscales to 640px JPEG,
   `mpv_copy_frame` includes subtitles). Fullscreen is the window's own
   (`setFullscreen`), since mpv only grows with the window. Measured live:
   a 1080p SubsPlease MKV resumed 6:26 in ~9s from click. mpv stays idle
   between episodes (`mpv_stop` unloads the file and restores the opaque
   background). Not carried over from the `<video>` player: the ambient
   dock tint (the webview can't read mpv's pixels) and lifting bottom
   subtitle lines above the visible control dock.

   When the native window enters fullscreen, `fullscreen.ts` mirrors the
   state onto the document root and removes the body's 32px title-bar offset.
   The home app bar remains visible at the top of the screen.

   **Torrent downloads now live under AppData/cache** (`dirs::cache_dir()`
   joined with `nyaa-stream/downloads`), not the user's actual Downloads
   folder - app-owned data, not something the user is meant to keep or
   browse to directly. (Correction 2026-09-29: `stop_playback` removes the
   torrent from the session but never deletes its files - the folder had
   grown to 7.1 GB. v0.4.0's download cache bounds it.)

   **Hide unlisted sources** (setting, on by default): the anime page's
   list, the player's episode list, Play/Resume and next-episode only use
   rows that are real episodes - numbered within the episode count
   (AniList's, or Kitsu's via the fallback). Batch and Unknown rows are
   hidden; a show still airing (no count yet) keeps every numbered episode
   because Kitsu's list trails new releases. The route still resolves
   against every row, so a direct link to a hidden one plays; the header
   says how many rows are hidden.

   **Movies.** AniList `format: MOVIE` pages have one "Movie" group
   (`MOVIE_KEY`, labelled as episode 1 so `#/anime/:id/episode/1`, resume
   and Continue watching resolve to it) holding every release that is the
   film: titles the parser can't number, or that say movie/film/劇場版,
   minus the franchise's TV episodes and season packs that share its name
   (`movieSources` in App.tsx). Movies skip the nyaa view-page scrape, the
   player always plays the torrent's largest video (never an extra), and
   the page/search show runtime instead of an episode count.

   **Kitsu fallback for AniList.** `search_anime` and `get_anime_details`
   divert to Kitsu (`kitsu_client::search_anime`/`anime_by_anilist_id`) when
   AniList answers 429 or 5xx or can't be reached, and skip AniList for 60s
   after that (`metadata_fallback::AniListCooldown`) rather than extending a
   rate limit. Kitsu results carry the AniList id Kitsu maps them to, so
   routes, library and progress stay keyed by AniList id; entries Kitsu
   can't map are dropped. Other AniList calls (airing feed, relations
   offset) still just fail soft.

## Dev build speed (tooling, no version bump)

Measured 2026-09-29: a dev build writes `nyaa_stream.pdb` (488 MB) and
`nyaa_stream_lib.pdb` (343 MB); MSVC `link.exe` rewrites them after every
Rust change. Three changes, each timed on an incremental rebuild (touch
`src-tauri/src/lib.rs`, `cargo build -p nyaa-stream`) before/after:

1. **Less dev debug info** (`Cargo.toml`): `[profile.dev] debug =
   "line-tables-only"` (panics/backtraces keep file:line; no variable-level
   debugging) and `[profile.dev.package."*"] debug = 0` for dependencies.
   Release profile untouched.
2. **rust-lld for dev links** (`.cargo/config.toml`, MSVC target): link with
   the `rust-lld.exe` shipped in the toolchain instead of `link.exe`. Kept
   only if it links the static FFmpeg/libtorrent libs and the app starts.
3. **rust-analyzer's own target dir** (`.vscode/settings.json`,
   `rust-analyzer.cargo.targetDir: true`, un-ignored in `.gitignore`), so
   the editor's `cargo check` never blocks or invalidates `tauri dev`.

Result (incremental rebuild after touching `src-tauri/src/lib.rs`):
3m34s baseline → 2m16s with (1), PDBs 831 MB → 437 MB (the rest is debug
info inside the static FFmpeg/libtorrent libs) → 1m36s with (2). The
rust-lld build starts and runs normally. `scripts/release.mjs` sets
`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=link.exe`, so shipped builds
still link with MSVC.

Not done: sccache (clean builds only); a Windows Defender exclusion for
`target\`, `~\.cargo`, `~\.rustup` is the user's call.

## sbtl backend (the torrent engine; default from v0.4.0)

The torrent engine: sbtl, a streaming-first BitTorrent library
(C + Rust crates) written for nyaa-stream, published at
https://github.com/sb-gravity100/sbtl and pinned by tag (`v0.1.0`) in
`crates/sbtl-engine/Cargo.toml` and used through `sbtl_engine` (see
"sbtl_engine"; until then it was an `enginefs` backend,
`vendor/enginefs/src/backend/sbtl_backend.rs`). It started as a
build-time alternative to libtorrent; since 2026-10-04 it is the only
engine (libtorrent and librqbit were removed - see the Stack section) and
the build has no engine features.

Why: sbtl is built around the stalls documented above (startup queue delay,
pieces stuck on one slow peer, slow seeks, zero/stale bytes):

- a reader's position is the playback cursor; a read outside the
  read-ahead window is a seek that preempts queued background requests at
  once (no 100 ms re-anchor debounce);
- urgent blocks go only to peers at least 1/4 as fast as the fastest one,
  duplicated only when another peer would clearly deliver first;
- reads return only verified bytes, through sbtl's own storage handle (no
  second OS handle racing a cache), so no zero guard or broker is needed;
- a foreground reader gets the file tail early (MKV Cues / MP4 moov) and
  the rest of its file in order afterwards; other files of a batch are not
  downloaded unless preloaded or kept.

Measured in sbtl's own benches (local throttled swarm, not real
swarms yet): paced 700 KB/s playback with 10 random seeks per run - 1-3
stalls (0.14-0.62 s total) and ~1 s seek start-up; a mid-file 8 MiB seek
range completes in 1.50 s vs libtorrent's 2.24 s on the same swarm.

Through this app's own streaming server (`crates/torrent-engine/examples/engine_smoke.rs`:
magnet with peer hints -> metadata -> Range GETs for head 1 MiB, tail 2 MiB,
middle 4 MiB and three random 2 MiB seeks; local swarm of 8 seeders at
512 KiB/s each, debug builds, two runs each):

| backend | metadata | head | middle | seeks | total | byte mismatches |
|---|---|---|---|---|---|---|
| sbtl | 0.36 s | 1.4-1.6 s | 0.8-1.3 s | 1.3-3.2 s | 8.7-9.8 s | 0 |
| libtorrent | 1.6-1.8 s | 11.6-15.3 s | 16.5-19.2 s | 3.4-12.5 s | 50.6-65.2 s | 1 (head, first run) |

Synthetic swarm, so it is no substitute for real ones; the libtorrent
coordinator's leases and watch hints are not exercised by bare GETs.

Buffer signals (sbtl only): `TorrentHandle::buffer_status` reports the
verified playback time ahead of the newest foreground reader of a file
(level stalled / low / ready / full, ETA to ready). `get_stream_stats`
carries it as `StreamStats.buffer`, `src/loadingProgress.ts` shows real
readiness from it instead of the peers/bytes/speed estimate, and mpv's
`--cache-pause-wait` follows sbtl's ready threshold (3 s instead of 10 s;
`torrent_engine::MPV_CACHE_PAUSE_WAIT_SECS`) since sbtl reports when that much
is verified ahead. Measured with engine_smoke's playback phase (local swarm,
700 KB/s reader from a fresh offset): stalled -> ready at 1.4 s, full (30 s
ahead) at 4.1 s.

sbtl now has the swarm features the libtorrent backend relied on:
- **DHT:** IPv4 only. Its state is kept in `<download_dir>/.sbtl/dht.dat`. `NYAA_SBTL_NO_DHT=1` turns it off for local-swarm tests.
- **uTP.**
- **Seeding with a choker:** `set_seeding_enabled` maps onto sbtl's session-wide seeding switch. `set_upload_throttled` sets a per-torrent upload limit of 16 KiB/s and never turns seeding back on.
- **Adding torrents:** a magnet already added, or one still waiting for its metadata, joins the existing torrent: a per-infohash lock covers the metadata wait. A cached `.torrent` keeps the magnet's `tr=` trackers.
- **Downloaded ranges and file completeness** count only data already written to disk. sbtl's disk worker can hold verified pieces in memory before writing them, and resume buffers and probes read the files directly.
- **HTTPS trackers:** these use WinHTTP.

Remaining gaps:
- No IPv6 DHT.
- `stats()` peer-search fields are approximations.
- Not yet measured on real swarms (only local ones): the v0.4.0 live test
  covers that, including a poorly seeded torrent.
- Data libtorrent downloaded into the same directory should be reused
  (sbtl re-checks it once, then keeps a resume file in
  `<download_dir>/.sbtl`) - to verify live in the v0.4.0 test.

### sbtl versions and updates

nyaa-stream pins a released sbtl: `sbtl = { git =
"https://github.com/sb-gravity100/sbtl", tag = "vX.Y.Z" }` in
`crates/sbtl-engine/Cargo.toml` (cargo finds the crate in the repo's
`bindings/rust/sbtl`; `sbtl-sys`'s build.rs compiles the C sources from the
same checkout). nyaa's `Cargo.lock` is gitignored, so the tag is the pin:
a pushed sbtl tag is never moved or re-created.

- **Working on both:** `npm run dev:sbtl-local` runs the app against the
  sibling `../sb_torrent` checkout through `cargo --config
  "patch.'https://github.com/sb-gravity100/sbtl'.sbtl.path=..."` - nothing
  committed. Never put that `[patch]` in a `.cargo/config.toml` (e.g. one
  in a parent directory): it would reach release builds silently.
- **Release guard:** `scripts/release.mjs`'s preflight asks `cargo tree -p
  sbtl` which sbtl the build resolves and fails unless it is a tag of the
  sbtl repo.
- **Releasing an sbtl change:** in sbtl - commit, bump the workspace
  version (0.x: minor for API breaks, patch for fixes), tag `vX.Y.Z`,
  push the tag. In nyaa - change `tag`, `cargo update -p sbtl -p
  sbtl-sys`, rebuild the DLL at that tag and replace `native/sbtl/*`
  (`native/sbtl/README.md`), `cargo check --workspace`, `engine_smoke`,
  commit `chore: sbtl vX.Y.Z` (one line on what changed), add a row below.
  nyaa never pins an untagged sbtl commit.
- **The DLL:** nyaa links `native/sbtl/sbtl.dll` (built with sbtl's `nyaa`
  preset: no media indexer, nyaa probes media itself) instead of compiling
  sbtl's C on every clean build; the bundle ships it beside the exe. A
  mismatching DLL fails at session start (`sbtl_abi_version`), and
  `scripts/release.mjs` refuses a release whose `native/sbtl/VERSION` is not
  the pinned tag.
- **API changes** come in pairs, in one sitting: the sbtl tag, then the
  nyaa adapter commit (nyaa is sbtl's only consumer).

| sbtl | nyaa commit | what changed |
|---|---|---|
| v0.1.0 | `250d871` | first tag: rename tl -> sbtl, public repo; one owner per torrent (duplicate-add use-after-free fix), on-disk file ranges, per-torrent upload limit |
| v0.1.1 | `4c1e4bf` | magnet metadata no longer waits 10 s on a silent peer (engine_smoke on BBB: metadata 12.4 s -> 2.1 s median); MP4/MKV index and `media_probe` (nyaa leaves it off); shared-library build with an ABI check |

## sbtl_engine (v0.4.0 - replaced the vendored enginefs)

With sbtl the only engine, `vendor/enginefs` (~6k lines from
stream-server) is mostly unused: nyaa calls about a dozen entry points,
and under sbtl several are no-ops (watch hints, the seeding-disabled pause
loop, lease-expiry cleanup). The rest is libtorrent-shaped (leases named
after it, libtorrent state codes, `manages_playback_lifecycle()`
branches). **sbtl_engine** is a new workspace crate (`crates/sbtl-engine`,
lib `sbtl_engine`) holding only what nyaa needs, written against sbtl
directly. `torrent-engine` keeps the HTTP streaming server, HLS, probes
and resume buffers and switches from `enginefs` to `sbtl_engine`.
Afterwards stream-server is no build dependency at all: `vendor/enginefs`,
the `[patch]` and the `enginefs` git dependency go.

**API** (all logged per the logging rules):
- `Engine::start(download_dir)` - sbtl session (DHT state in
  `.sbtl/dht.dat`, `NYAA_SBTL_NO_DHT`), tracker manager, background loop.
- `Engine::add(Source)` (magnet / .torrent URL / bytes) -> `Arc<Torrent>`:
  default trackers + the daily-refreshed public list ranked by RTT
  (enginefs's `trackers.rs`/`tracker_prober.rs`, ported as-is, MIT) +
  the magnet's `tr=`; metadata cache `.sbtl/<hash>.torrent`; per-hash add
  lock across the metadata wait (as in the sbtl backend today).
- `Engine::get(hash)`, `Engine::remove(hash)`,
  `Engine::touch_playback(hash, file_idx)` (the lease that replaces
  `refresh_hls_playback`).
- `Torrent`: `info_hash`, `name`, `files()`, `stats()` ->
  `TorrentStats` (state as an enum `Checking / Metadata / Downloading /
  Finished` instead of libtorrent codes; rates, bytes, peers, per-file
  on-disk ranges), `open(file_idx, offset, ReadKind)` -> `Reader`
  (`AsyncRead + AsyncSeek`; `Foreground` = sequential + tail prefetch +
  SEQ_AHEAD + buffer probe, `Background` / `Probe` = window only),
  `buffer(file_idx)`, `set_preload_file`, `file_path`. (Planned
  `keep_downloading`, `clear_file` and `throttle_upload` were left out:
  nothing in nyaa calls them; seeding stays sbtl's default, on.)
- **Lifecycle** (what enginefs's loop did that still matters under sbtl):
  every 15 s, leases older than 15 s expire; a torrent with no open
  `Reader`, no lease and no access for 5 min is removed (files kept).

**Not ported** (unused by nyaa): enginefs's HLS module, subtitle
extraction, hwaccel, piece/disk/metadata caches, OpenSubtitles hash,
libtorrent priorities and coordinator, multi-file selection, the
seeding-disabled pause loop (sbtl's session switch covers it), and the
watch hint (a no-op under sbtl: `play_magnet`'s `watch` argument and
`torrent_engine::WatchHint` go too).

**torrent-engine changes:** `efs` -> `sbtl_engine::Engine` in `lib.rs`,
`direct_input.rs` and the resume-buffer path; `StreamStats` mapped from
`TorrentStats`; the load profiler's checking probe uses
`TorrentState::Checking`; `engine_smoke` ported.

**Verify:** unit tests (magnet parsing, trackers, lease/idle removal),
`cargo check --workspace`, `engine_smoke` on a local swarm against the
numbers in "sbtl backend", then the v0.4.0 sbtl live test (it runs on
sbtl_engine, so it covers this change).

**As built** (2026-10-04): `crates/sbtl-engine` (`lib.rs` Engine +
lifecycle, `torrent.rs` Torrent/Reader/stats, `magnet.rs`, ported
`trackers.rs`/`tracker_prober.rs` + `LICENSE-enginefs`); torrent-engine's
`TorrentEngine` holds `torrents: Arc<Engine>`; `torrent_state` returns
`TorrentState`; `vendor/enginefs`, the `[patch]` and the stream-server git
dependency are gone. Unit tests: magnet parsing, idle rule, lease expiry.

**engine_smoke A/B** (BBB, 8 local libtorrent seeders at 512 KiB/s, DHT
off, debug builds, old = enginefs + sbtl backend at `e0db1c3`, runs
alternated): 0 mismatches in every run. Range GETs: metadata 0.36 s, head
1.6-1.75 s, seeks 1.0-2.2 s, GET phase ~8.6-8.9 s (same as "sbtl
backend"). Playback phase, time to ready / full: old 1.96 s / 5.35 s and
2.28 s / 9.60 s; new 1.74 s / 5.86 s and 2.28 s / 4.13 s (two earlier new
runs: 4.2 s / 7.6 s, 3.2 s / 9.6 s, on a cold swarm). Both show brief
stalled dips before ready. No regression; the earlier 1.4 s / 4.1 s in
"sbtl backend" was a single run.

**Live test** (2026-10-04, dev builds, load profiler; play -> mpv
`file-loaded`; Fate/strange Fake, Kaleido-subs, same downloads dir):

| | libtorrent (`031d9c0`) | sbtl_engine (main) |
|---|---|---|
| cold start | 27.2 s, 13.4 s | 11.0 s, 10.8 s, 22.3 s (5.3 s of it metadata) |
| seek into undownloaded data | 2.4-10.6 s; 2 of 10 abandoned (7.7 s, 25.6 s) | mostly 1.5-3.6 s, outliers 5-11 s and one 17.9 s; 0 of ~118 abandoned |
| reopen of downloaded data | - | 0.14-1.06 s |
| resume (Ep 4, cached, 7:15) | 22.9 s (2.5 s re-check) | not repeated |

Poorly seeded (Mushoku Tensei Ep 10, 4 peers): start at 14:31 in 6.4 s,
seeks 2.1-9.1 s, none abandoned. A 1-peer release never delivered
metadata (120 s, twice) and Fate Ep 9 stalled at 0.04-0.64 Mbit/s with
9-20 peers (two abandoned starts) - no libtorrent run of either to
compare. mpv's MKV parse errors only appeared when a load was abandoned.
Found and fixed meanwhile: resume buffers never survived a save (a
`resume.rs` bug, in both builds - `8532f1d`). Not yet covered: a
libtorrent-era partial download (Ep 3/7) reopened on sbtl, and a
resume served from a resume buffer.

## Known gaps / not yet implemented

- (HLS fallback player only - mpv reads the file's real duration.) The
  HLS playlist's declared duration is only as good as the frontend's
  AniList-derived estimate (falls back to a generic 24-minute guess if
  even that's missing) - not the file's real, exact length. Good enough
  for a working seek bar; the last segment may be trimmed slightly short
  or read past EOF if the estimate is off by more than a few seconds.
- Watch progress, settings and preferred fansub groups live in
  `localStorage` (like the library) - no sync across machines.
- A batch whose episode range covers an episode isn't offered as a
  source on that episode's row; batches are played from their own "Batch"
  row, where the player's file menu picks the episode.
- Batches without an explicit episode range in their title (most of them)
  can't be attributed to specific episodes and stay in an undifferentiated
  per-season "Batch" bucket.
- Transcoding needs a capable machine: NVENC/QSV/AMF are auto-detected,
  otherwise OpenH264 (CPU, lower quality than x264, kept for LGPL) - slower
  CPUs may not keep real time for 1080p HEVC sources.
  Measured with NVENC (smoke test, release build): 1080p 10-bit HEVC ->
  H.264 at 12.9x realtime, 10-bit AV1 at 13.5x - far above download speed.
  A GPU-resident pipeline (decode -> convert -> encode without the frame
  round trip through system memory) was evaluated and skipped: 10-bit
  sources need `scale_cuda` for the GPU-side 8-bit conversion, which
  vcpkg's FFmpeg doesn't build (no cuda-llvm), so it would cost a custom
  overlay port for no user-visible gain.
- libmpv-2.dll (bundled, GPL build - the distributed app is therefore
  effectively GPL) is needed for playback (else the HLS fallback) and for
  torrent-captured thumbnails.
- The one-time media probe (`probe_media`, ffmpeg-next) still reads
  `stream_handler` over loopback HTTP; HLS and subtitle runs read the
  torrent directly (`direct_input.rs`).
- An sbtl reader fails after 60 s without data (no peer delivers a
  piece); `direct_input` reopens at the same byte offset (the HTTP probe
  relies on ffmpeg `-reconnect`).
- Torrent-captured thumbnails use the torrent's largest video file. The seek point targets roughly
  the episode's midpoint when a duration estimate is available (a fixed
  early point otherwise), but without a Matroska Cues index on a
  still-downloading torrent that seek is best-effort and can land short
  of the target or occasionally not resolve in time, same tradeoff as
  torrent-engine's HLS segment restarts.
See `PHASES.md` for the build order.
