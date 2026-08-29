# PHASES.md — nyaa-stream

## Phase 0 — Scaffolding (done)
- [x] Tauri + Preact-TS project scaffolded
- [x] Cargo workspace with `torrent-engine`, `nyaa-client`, `anilist-client`,
      `mpv-ipc` crates
- [x] `reference/stremio-core` cloned for architecture reference
- [x] Tauri commands wired: `search_anime`, `search_torrents`,
      `play_magnet`, `set_pause`
- [x] `cargo check --workspace` passes clean

## Phase 1 — Prove the pipeline end-to-end (done)
- [x] Frontend: search box + live-search dropdown calling `search_anime`
- [x] Frontend: media detail page (`MediaPage.tsx`) with real per-episode
      source counts from `search_torrents_for_anime`
- [x] Verbose `tracing` logging through the whole backend (used as the
      primary way to verify end-to-end behavior when the native window
      can't be driven by browser-automation tooling)
- [ ] "Play" button calling `play_magnet` — not built yet, see Phase 3
- [x] Fixed the librqbit/axum-range API mismatches `cargo check` turned up

## Phase 2 — Metadata-to-release matching (mostly done)
- [x] Deterministic nyaa-title parser (`episodeParser.ts`): season, episode,
      batch (including explicit-range batches spread across the episodes
      they cover), submitter (fansub group tag) — regex-only, verified
      against a 150-title real-world scrape (147/150 correct from title
      text alone)
- [x] View-page scrape (`get_torrent_details_batch`) as the deterministic
      backstop for the remaining ambiguous titles — real submitter + batch
      file-count ground truth, bounded concurrency, only run for titles the
      regex parser couldn't resolve
- [x] nyaa.si query fixes: curly-apostrophe sanitization, English→romaji
      title fallback, paginated HTML scrape (RSS was capped at 75 results)
- [ ] Per-anime preferred fansub group memory — not built

## Phase 3 — Playback quality of life
- [x] Wire a video player to the media page: play button per episode row
      calls `play_magnet`, auto-picking the release with the most seeders
      out of that episode's group (`bestRelease` in `releases.ts`), and
      plays it in a full-viewport `PlayerView.tsx` with a PotPlayer-style
      hover-reveal bottom control bar (play/pause, seek, volume,
      fullscreen, time, source picker)
- [x] Manual source-picker dropdown: `PlayerView.tsx` takes the full
      release list for an episode and lets the user switch sources
      mid-session via a `<select>` in the control bar (sorted by seeders) —
      `play_magnet`'s existing defensive cleanup on the backend tears down
      the previous torrent automatically
- [x] ~~Embed mpv into the app window~~ tried and abandoned: `--wid`
      embedding needed a transparent webview background, and `transparent:
      true` broke all click input app-wide on this Tauri/WebView2/Windows
      combination (upstream bug, not fixable from app code — see PLAN.md's
      Known gaps). Replaced with a plain HTML5 `<video>` element; `mpv`/
      `mpv-ipc` is kept only for headless thumbnail capture now
- [x] Raw torrent bytes aren't reliably playable in a browser `<video>`
      even with the right codecs/`Content-Type` (verified live - Matroska's
      seek index/duration commonly live near the file's *end*, which an
      incrementally-downloading torrent can't provide up front). Fixed by
      piping through `ffmpeg` into fragmented MP4 (`torrent_engine::
      remux_handler`/`remux_url`) - `-c:v copy`, audio always transcoded to
      AAC (several codecs real releases use aren't remux-able into fMP4).
      Seeking restarts the remux at a new `-ss`/`-copyts` offset rather
      than seeking within one stream; `probe_duration_seconds` (`ffprobe`)
      gets the real duration into the output header via `-t`, though this
      is unreliable on a fresh download for the same structural reason (see
      PLAN.md's Known gaps)
- [x] Surface torrent download/buffer progress in the UI while the video
      buffers: ported stremio-web's real Player UI (verified against
      `reference/stremio-web`/`reference/stremio-core`) rather than a
      from-scratch design — `Buffering.tsx`'s pulsing clip-path-filled mark
      uses `loadingProgress.ts`'s weighted peers/downloaded/speed readiness
      score (mirrors `useStatistics.ts`'s `getLoadingProgress`), and
      `StatisticsMenu.tsx` is a toggleable peers/speed/completed/info-hash
      card mirroring `StatisticsMenu.js` (`StreamStats` gained
      `downloadedBytes`/`totalBytes` to support the weighted score)
- [x] Seek bar shows a download-progress highlight (`.player-seek-downloaded`
      in `PlayerView.tsx`) alongside the played-position fill, approximated
      from the torrent's overall byte-download percent
- [x] Re-adding a torrent whose destination file already exists (replaying
      an episode, or a thumbnail capture colliding with a real download)
      used to fail outright with "allow_overwrite = false" - fixed via
      `AddTorrentOptions.overwrite`
- [ ] File selection for multi-file/batch torrents (currently hardcoded to
      file index 0)
- [x] Persist a saved-anime library (`src/library.ts`, `localStorage`) —
      resolves the "TBD: sqlite vs. flat file" note for *this* use case;
      watch-history/continue-watching persistence is still open
- [x] Handle the player closing and clean up the torrent session: closing
      `PlayerView` or navigating away calls `stop_playback`, which removes
      the active torrent; `play_magnet` also defensively runs the same
      cleanup before starting a new session

## Phase 4 — Library and polish
- [x] Home page: "Library" grid of saved anime + a "Latest Episodes" row
      (this calendar week + last week, sorted newest-first, built from
      AniList's batched `airingSchedules` query across the whole library)
- [x] Backdrop/episode-thumbnail fallback chain: Kitsu (`crates/kitsu-client`)
      → AniList `streamingEpisodes` → torrent-captured frame
      (`capture_torrent_thumbnail`, headless mpv + on-disk cache) for shows
      neither metadata source has art for
- [ ] Watch progress / continue-watching (distinct from the library
      save-list above)
- [ ] Local library backed by AniList's own list if the user authenticates
      (optional OAuth — out of scope until asked for)
- [ ] Settings: mpv path override, download directory, preferred fansub
      groups, subtitle preferences
- [ ] Packaging: Tauri bundler for Windows installer; document the mpv
      system-dependency requirement clearly at install/first-run
