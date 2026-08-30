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
      text alone). Re-verified against live "That Time I Got Reincarnated
      as a Slime" data and fixed three real mismatches it surfaced: a
      title carrying both a season-relative and an absolute episode number
      (fixed with leftmost-match-wins instead of a fixed pattern-priority
      order), "Season N - M" being misread as a batch range instead of a
      season/episode pair, and a pipe-terminated dash-number falling
      through to no match. No regression against a live 160-title Frieren
      re-check. `crates/nyaa-client/examples/search_debug.rs` reruns this
      kind of check against live data without the full app running.
      Further audit across Attack on Titan/Shingeki no Kyojin, Jujutsu
      Kaisen, and Spy x Family found and fixed a fourth, more severe
      mismatch: "Final Season" (Attack on Titan's real season 4, and used
      the same way by other franchises) has no digit for
      `extractSeasonNumber` to find, so it silently defaulted to season 1
      - colliding every "Final Season" release's episode number with the
      real season 1's same-numbered episode in the same "Episode N"
      bucket. Fixed with a dedicated sentinel season value + "Final
      Season" display text (`FINAL_SEASON_PATTERN`/`FINAL_SEASON_NUMBER`)
      instead of defaulting to 1. The audit also confirmed the
      absolute-vs-season-relative numbering gap already documented in
      PLAN.md's Known gaps is a real, recurring pattern (not just the one
      Slime/Doomdos case) - left unfixed, see there for why.
- [x] View-page scrape (`get_torrent_details_batch`) as the deterministic
      backstop for the remaining ambiguous titles — real submitter + batch
      file-count ground truth, bounded concurrency, only run for titles the
      regex parser couldn't resolve
- [x] nyaa.si query fixes: curly-apostrophe sanitization, paginated HTML
      scrape (RSS was capped at 75 results). Two further gaps found via
      live search against "That Time I Got Reincarnated as a Slime" (950
      real results): the per-search page cap (`MAX_SEARCH_PAGES`) was 5
      (375 results), silently dropping ~60% of that show's releases -
      raised to 20; and the English-title-first-with-romaji-fallback
      scheme almost never actually fell back to romaji (nyaa.si's search
      returns "enough" English-title results long before running out),
      so romaji-only-titled releases (489 of them for this show, from
      major groups like SubsPlease/Erai-raws/Ironclad) were always missed
      regardless of the page cap - fixed by always searching both titles
      and merging/deduplicating the results instead of falling back
- [ ] Per-anime preferred fansub group memory — not built

## Phase 3 — Playback quality of life
- [x] Wire a video player to the media page: play button per episode row
      calls `play_magnet`, auto-picking the release with the most seeders
      out of that episode's group (`bestRelease` in `releases.ts`), and
      plays it in a full-viewport `PlayerView.tsx` with a PotPlayer-style
      hover-reveal bottom control bar (play/pause, seek, volume,
      fullscreen, time, source picker)
- [x] Solid-black control bar shown only on direct hover (a dedicated
      `.player-controls-hover-zone` the same footprint as the bar, since
      the bar itself has `pointer-events: none` while hidden and so can't
      receive the hover that would reveal it) instead of the old "any
      mouse movement over the video, then auto-hide after an idle timer"
      behavior, plus a mute button and PotPlayer/YouTube-style keybinds
      (space/K play-pause, arrows ±5s / J/L ±10s seek, up/down ±5 volume,
      M mute, F fullscreen, Esc close) that work whether or not the mouse
      is anywhere near the bar - a keybind press flashes the bar briefly
      so its effect (new time/volume/pause state) is visible without
      requiring the mouse to be there too.
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
      incrementally-downloading torrent can't provide up front). First
      fixed with a whole-episode `ffmpeg` remux restarted on every seek -
      worked, but leaked `ffmpeg.exe` processes under rapid seeking
      (closing the client side alone doesn't reliably kill it on Windows).
      Replaced with real HLS: `torrent_engine::hls_playlist_handler` serves
      a VOD `.m3u8` (duration supplied by the frontend, from AniList - see
      PLAN.md's Known gaps for why the backend can't determine this
      reliably itself); the frontend plays it via `hls.js`. Seeking is a
      plain `video.currentTime` set - hls.js fetches whichever segment
      covers it
- [x] First HLS segment-serving design (one `ffmpeg -ss/-t` invocation per
      segment) caused audible audio artifacts at every segment boundary and
      occasional dropped segments (`Error submitting a packet to the muxer:
      Invalid argument`) - each independent invocation reinitialized its
      own AAC encoder/timestamp timeline, and `-ss` before `-i` only
      approximately seeks without a Matroska Cues index. Replaced with
      `HlsJobs`: a single continuous `ffmpeg` process per torrent file
      (one AAC encoder/timestamp timeline for the whole episode) that
      writes real segment files to disk (`-f hls -hls_flags temp_file` -
      atomic rename on completion, never serves a partial file); segment
      requests wait for the job's sequential progress to reach them, or
      restart the job at a new offset only for a real forward/backward
      seek (more than `RESTART_LOOKAHEAD_SEGMENTS` away from current
      progress), not for ordinary buffering. Also stopped ffmpeg from
      auto-including embedded subtitle/attachment streams (`-map 0:v:0
      -map 0:a:0 -sn`) - MPEG-TS can't carry them and leaving them in
      produced non-monotonic-DTS spam and was implicated in the dropped
      segments above
- [x] Latest Episodes row jumps straight into the player for the clicked
      episode (`App.tsx`'s `autoplayEpisode`/`selectEpisode`, `MediaPage.tsx`
      auto-triggers play once that episode's sources have loaded) instead
      of leaving the user on the episode list to click play themselves
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
      in `PlayerView.tsx`) alongside the played-position fill. Originally
      approximated from the torrent's overall byte-download percent, which
      was verified live to actively mislead: a seek target could sit well
      inside the "downloaded" region and still take 20+ seconds, because
      that region's specific byte range hadn't actually been read/muxed
      into an HLS segment yet (raw download and HLS transcode progress can
      diverge either way). Replaced with `StreamStats.readySeconds`
      (`HlsJobs::ready_seconds` in torrent-engine) - the real "how far have
      segments actually been produced" mark, which is what determines
      whether a seek is instant.
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
