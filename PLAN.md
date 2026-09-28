# PLAN.md — nyaa-stream

Anime-focused Stremio-like desktop client. Not a full Stremio addon-ecosystem
clone — a focused tool that searches nyaa.si for torrents, matches them
against AniList metadata, and plays them with the system `mpv` embedded in
the app window under a transparent webview that draws the controls (see
"Embedded mpv playback" below). An HLS `<video>` player remains as the
fallback when mpv isn't installed.

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
- **Player:** system `mpv` (must be on PATH — not bundled) embedded in the
  app window via `--wid` (`crates/mpv-ipc`'s `EmbeddedMpv`,
  `src-tauri/src/player.rs`), driven over JSON IPC by the HTML controls in
  `src/PlayerView.tsx` through `src/mpvVideo.ts`. Fallback without mpv:
  the HLS `<video>` + `hls.js` player (`src/HlsPlayerView.tsx`). mpv is
  also used headlessly for thumbnail capture
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
- The same `vcpkg install` also builds FFmpeg 7.1.2 (LGPL feature set, see
  the streaming-server section; ~6 minutes, cached afterward).
  `ffmpeg-sys-next` finds it through `FFMPEG_DIR` (set in
  `.cargo/config.toml` to `vcpkg_installed/<triplet>` - the `vcpkg` crate
  it would otherwise use only understands classic-mode installs), and
  `crates/torrent-engine/build.rs` links FFmpeg's static dependencies.
  bindgen needs LLVM's libclang (`LIBCLANG_PATH`, default
  `C:\Program Files\LLVM\bin` in the same config).
- `.cargo/config.toml` at the project root sets `target-cpu=x86-64-v3`
  (Haswell/2013+ CPUs) for both Rust and the vendored C++ code, matching
  stream-server's own build config - this is a real minimum CPU
  requirement for anyone building or running this app, not just a compiler
  hint.
- **Torrent source:** nyaa.si search, scraping its paginated HTML results
  table (its RSS feed was found to silently ignore the `p=` page param and
  always cap at 75 results — see `crates/nyaa-client`); responses are
  cached on disk (`nyaa-client/src/cache.rs`) because nyaa.si rate-limits:
  searches reused for 30 min and served stale when a refetch fails (a 429
  is an error, never an empty result), view-page details kept forever
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
  multi-minute midpoint isn't practical within the capture's timeout. Thumbnails are cached for good
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
  vcpkg.json                 vcpkg manifest (libtorrent + openssl, for torrent-engine)
  triplets/                  custom vcpkg triplet (x64-windows-v3-static-md-release)
  .cargo/config.toml         vcpkg env vars + target-cpu=x86-64-v3 rustflags
  src-tauri/                 Tauri app crate (commands, window, app state)
  crates/
    torrent-engine/          enginefs (libtorrent backend) wrapper + local streaming/HLS HTTP server
    nyaa-client/              nyaa.si search client (paginated HTML scrape)
    anilist-client/          AniList GraphQL client
    kitsu-client/            Kitsu API client (backdrop + episode thumbnails)
    mpv-ipc/                 embedded mpv (playback) + headless mpv (thumbnail capture) over JSON IPC
  src/                       Preact + TypeScript frontend
  reference/stremio-core/    reference-only clone, gitignored
  reference/stremio-web/     reference-only clone, gitignored
  reference/stream-server/   reference-only clone, gitignored - source of the vendored
                              `enginefs` git dependency + vcpkg.json/triplets/ above
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
   release list) that appears on any mouse movement and fades out (with
   the cursor) after the mouse sits idle, unless the pointer rests over
   the top or bottom control areas - re-checked on every pointer move
   against the bars and their invisible hover zones, see PlayerView.tsx's
   CONTROLS_AREA_SELECTOR), plus PotPlayer/YouTube-style keybinds (space/K
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
   at display resolution with a slight CSS blur on its canvas (replaced 2x
   supersampling). Fonts are dumped once (`-dump_attachment`) and served at
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

   **enginefs is vendored** (`vendor/enginefs`, MIT, `[patch]` in the
   workspace manifest - see its `VENDORED.md`): its disk reader could hand
   out zero bytes for pieces libtorrent had verified but not yet made
   visible on disk, corrupting demux ("0x00 at pos N") and producing
   pixelated frames. It now waits up to 8s per piece for real bytes,
   preferring libtorrent's own `read_piece` copy. Video never waits on
   the probe: a run that starts before it finishes gets a subtitle-only
   ffmpeg attached (`HlsJobs::attach_subtitles`). A separate full-file
   subtitle pass (`sub_<index>_bg.ass`) reads `stream_handler` with
   `?intent=background` (no playback-lease refresh, libtorrent piece
   priority 1) so the whole track fills in as the torrent downloads
   without pulling priority from the playhead. hls.js `initPTS` is a raw
   33-bit PTS and is unwrapped before use (B-frame DTS just below zero
   wraps to ~95443s). A transcode whose ffmpeg exited is restarted on the
   next request, and every ffmpeg/ffprobe read uses `-reconnect` flags:
   enginefs' disk reader ends the HTTP body early when a piece isn't ready
   (and has been seen returning zero bytes for a not-yet-flushed piece) -
   both upstream issues in the vendored crate, worked around here.

   **Embedded mpv playback (the default player).** The approach
   stremio-shell-ng uses: `mpv_start` spawns the system mpv with
   `--wid=<app window>` (`--idle --force-window --no-config`, no OSC/input,
   `--hwdec=auto-safe`), finds the child window mpv creates (by process
   id) and pushes it to `HWND_BOTTOM` so the webview stays on top, then sets
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

   **Torrent downloads now live under AppData/cache** (`dirs::cache_dir()`
   joined with `nyaa-stream/downloads`), not the user's actual Downloads
   folder - this is app-owned scratch data cleaned up via `stop_playback`,
   not something the user is meant to keep or browse to directly.

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
- mpv must be on PATH for playback (else the HLS fallback) and for
  torrent-captured thumbnails - the only external binary, not bundled.
- The one-time media probe (`probe_media`, ffmpeg-next) still reads
  `stream_handler` over loopback HTTP; HLS and subtitle runs read the
  torrent directly (`direct_input.rs`).
- enginefs' playback coordinator can end a file stream early (permit
  cancellation/lease expiry); `direct_input` reopens at the same byte
  offset (the HTTP probe relies on ffmpeg `-reconnect`), not fixed
  upstream.
- Torrent-captured thumbnails use the torrent's largest video file. The seek point targets roughly
  the episode's midpoint when a duration estimate is available (a fixed
  early point otherwise), but without a Matroska Cues index on a
  still-downloading torrent that seek is best-effort and can land short
  of the target or occasionally not resolve in time, same tradeoff as
  torrent-engine's HLS segment restarts.
See `PHASES.md` for the build order.
