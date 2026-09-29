# PHASES.md — nyaa-stream

## Phase 0 — Scaffolding (done)
- [x] Tauri + Preact-TS project scaffolded
- [x] Cargo workspace with `torrent-engine`, `nyaa-client`, `anilist-client`,
      `mpv-player` crates
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
      absolute-vs-season-relative numbering gap (Jujutsu Kaisen/Kaizoku,
      Spy x Family, Slime/Doomdos) is a real, recurring pattern - later
      fixed for real, see the next entry.
- [x] Absolute-vs-season-relative episode numbering fixed via AniList's
      relations graph: `get_absolute_episode_offset`
      (`AniListClient::cumulative_prequel_episodes`) walks the PREQUEL
      chain backward from the browsed season, summing prior seasons'
      episode counts, and `App.tsx`'s `groupedSources` uses that to
      rewrite an absolute episode number back to its real season-relative
      one. Verified live against Slime Season 4 (user-confirmed ground
      truth: absolute episode 73 = season 4 episode 1, i.e. offset 72) -
      the naive walk undercounted to 48 at first because Slime's own
      relations graph routes Season 2's only PREQUEL edge through an OVA
      ("Visions of Coleus") rather than directly to Season 1, so the walk
      now follows through a non-TV prequel without counting it, only
      stopping when a *counted* (TV/TV_SHORT) prequel's own episode count
      is unknown.
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
- [x] Per-anime preferred fansub group memory (`releases.ts`): remembered
      once a release actually starts playing, preferred by the auto-pick
      when reasonably seeded, then preferred resolution, then seeders

## Phase 3 — Playback quality of life
- [x] Wire a video player to the media page: play button per episode row
      calls `play_magnet`, auto-picking the release with the most seeders
      out of that episode's group (`bestRelease` in `releases.ts`), and
      plays it in a full-viewport `PlayerView.tsx` with a PotPlayer-style
      hover-reveal bottom control bar (play/pause, seek, volume,
      fullscreen, time, source picker)
- [x] Solid-black control bar shown on mouse movement, faded out (with
      the cursor) after an idle timer unless the pointer rests over the
      top/bottom control areas (the bars or their always-present
      `.player-controls-hover-zone` strips, re-checked on every pointer
      move), plus a mute button and PotPlayer/YouTube-style keybinds
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
- [x] Embed mpv into the app window: first attempt abandoned (`transparent:
      true` on the window broke all click input app-wide), then done the
      stremio-shell-ng way - only the webview background transparent, mpv's
      child window pushed below it (see PLAN.md's "Embedded mpv playback").
      Now the default player; the HLS `<video>` player is the fallback
      without mpv
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
      seek (the job wouldn't reach the target within ~4s at its measured
      rate - see `job_will_reach_soon`), not for ordinary buffering. Also stopped ffmpeg from
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
- [x] hls.js configured with longer fragment-load timeouts/retries
      (`fragLoadingTimeOut`/`fragLoadingMaxRetry` in `PlayerView.tsx`) and
      a fatal-error recovery handler (`hls.startLoad()`/
      `recoverMediaError()`, hls.js's own recommended pattern, capped at 8
      attempts) - verified live: a slow segment (see the entry above)
      triggered hls.js's own default, shorter-than-our-server's-budget
      fragment timeout, which it treated as *fatal* and killed playback
      outright rather than just needing one more retry.
- [x] Re-adding a torrent whose destination file already exists (replaying
      an episode, or a thumbnail capture colliding with a real download)
      used to fail outright with "allow_overwrite = false" - fixed via
      `AddTorrentOptions.overwrite`
- [x] File selection for multi-file/batch torrents: `play_magnet` waits
      for metadata and returns every file with its own HLS URL; the player
      picks the file whose name parses to the episode (`episodeParser.ts`),
      else the largest video, and a file menu switches between them
- [x] Subtitles rework: ASS extraction inside the HLS job at the playhead,
      merged per-run scripts, libass rendering via JASSUB with embedded
      fonts, retrying probe, initPTS-corrected timing, Z/X delay
- [x] Streaming: `-copyts` shared timeline across seek restarts,
      rate-based restart decision, smaller probe windows, ready ranges on
      the seek bar
- [x] Persist a saved-anime library (`src/library.ts`, `localStorage`) —
      resolves the "TBD: sqlite vs. flat file" note for *this* use case;
      watch progress is `watchProgress.ts`, also `localStorage`
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
- [x] Torrent-captured thumbnails seek to roughly the episode's midpoint
      (mpv-player's `spawn_headless` gained a `start_seconds`/`--start` param)
      instead of a fixed early point, using the same AniList duration
      estimate the HLS playlist uses - avoids grabbing an early cold-open/
      logo frame. Falls back to the old fixed-early-point behavior when no
      duration estimate is available, since waiting for real-time playback
      to reach an actual multi-minute midpoint isn't practical within the
      capture's short timeout
- [x] Latest Episodes cards sized 16:9 (`.latest-episode-thumbnail` in
      App.css) instead of a poster's 2:3 - unlike the Library grid below,
      these show an actual episode still (that same fallback chain), not
      poster art, so they should be sized for that kind of image
- [x] Watch progress / continue-watching (`watchProgress.ts`): resume,
      per-episode progress bars and watched toggles, Continue watching row,
      next-episode button + autoplay countdown
- [x] Player side playlist (control-bar button), ±10s,
      center action flash, buffered seek layer, remaining-time toggle;
      episode cards open the player directly and closing returns home
- [x] Live-debug fixes: dead-transcode restart, ffmpeg reconnects, probe
      no longer gates video, 33-bit initPTS unwrap, group memory only on
      real playback, undecodable-codec ranking, background full-file
      subtitle pass, pre-warmed libass renderer
- [x] Vendor enginefs; its disk reader no longer passes unflushed zero
      bytes on (was the source of pixelated/broken frames)
- [x] Transcode anything the WebView can't decode (Hi10P H.264, HEVC,
      AV1, ...) to H.264 with auto-detected NVENC/QSV/AMF or libx264;
      forced 6s IDR keyframes for grid-exact segments
- [x] Player polish: rAF-driven seek bar/clock, animated chrome, ambient
      glass dock, paused glyph, Ctrl+C copies the current frame
- [x] Thumbnail capture deferred while a stream plays
- [x] Release build no longer opens console windows (CREATE_NO_WINDOW on
      every remaining child process)
- [x] In-process FFmpeg: ez-ffmpeg pipelines + ffmpeg-next probe/fonts,
      FFmpeg 7.1.2 statically linked from vcpkg (LGPL; OpenH264 CPU
      fallback instead of x264). No ffmpeg/ffprobe on PATH needed
- [ ] Read torrent bytes directly (ez-ffmpeg read/seek callbacks over the
      enginefs file handle) instead of the loopback HTTP stream
- [ ] Bundle mpv (thumbnail capture) as a sidecar, or capture thumbnails
      in-process too and drop mpv entirely
- [x] Subtitles: only dock-covered bottom lines lift while controls show;
      native-resolution libass rendering softened with a CSS blur
- [x] Subtitles: append-only event log + delta polling (`processData`),
      `ignore_readorder` for out-of-order scripts (Kaleido-subs), English
      tracks first, Crunchyroll default style (bundled Gandhi Sans)
- [x] UI redesign: dusk-indigo token system, bundled Zen Kaku Gothic New,
      app bar, key-art media page, SVG player controls and menus
- [ ] Local library backed by AniList's own list if the user authenticates
      (optional OAuth — out of scope until asked for)
- [x] Settings panel (`SettingsPanel.tsx`/`settings.ts`): resume, autoplay
      next, sticky fansub group, preferred quality, subtitle default
      on/language, default subtitle style with live preview
- [x] Settings: preferred fansubber text (e.g. "ToonsHub CR", every word must appear in the release title), libmpv path override (`set_mpv_path`), storage section (cache
      sizes + clear per cache), library/progress/settings backup
      export/import (Documents/nyaa-stream-backup.json)
- [ ] Settings still missing: ffmpeg path override, download directory
- [x] Player: playback speed (`[`/`]`/`\`), chapter marks on the seek bar,
      skip opening (Shift; chapter-aware, else +85s), audio track + audio
      delay in the subtitle menu, A-B loop (`A`) with MP4 clip export
      (`E`; NVENC H.264 + AAC via `export_clip`), media keys (global
      shortcuts while the player is open), Windows taskbar progress
- [x] HLS segment cache purged on startup and exit
- [x] Search/sources: lazy-loaded dropdown results, recent searches,
      release badges (batch/dual audio/source/codec/10-bit), seeder health,
      per-show quality override, no-seeders warning
- [x] Home/library: hero cycles through candidates, library sort/filter,
      watched-count badge + new-episode dot, dismiss from Continue
      watching, airing countdown, mark-all-previous-watched
- [ ] mpv player: restore the ambient dock tint and lifting bottom
      subtitles above the visible dock (both depended on reading/moving
      the `<video>` picture)
- [ ] Packaging: Tauri bundler for Windows installer; document the mpv
      system-dependency requirement clearly at install/first-run
- [x] Player chrome: full-width control strip over a bottom gradient (no
      glass dock), controls also revealed by hovering the top edge;
      verbose statistics popup (torrent, streaming run, playback,
      thumbnail)
- [ ] Fast playback start (v0.3.2, see PLAN.md "Fast playback start"): one
      commit each -
      - [ ] enginefs: startup baseline priority 0, raised to 1 once the
            startup buffer verifies (or 15s fallback)
      - [ ] enginefs: ~4 MB startup window at priority 7 + staggered
            deadlines; ~4 MB read-ahead at 7 after the first byte
      - [ ] `play_magnet` `watch: first|resume` hint → engine, per file
      - [ ] enginefs: first watch - sequential download from piece 0
      - [ ] enginefs: continue watch - pieces before the resume point at 0,
            sequential from it, earlier pieces back to 1 when the rest is
            done or on a seek back
      - [ ] enginefs: a seek into undownloaded data re-anchors
            continue-watch mode at the target (debounced ~300ms blocked
            read); never for background/probe/preload reads
      - [ ] enginefs: waiting-piece log reports the effective window
      - [ ] `VENDORED.md` notes for the three enginefs changes
      - [ ] Spawn embedded mpv at app launch instead of first play
      - [ ] `mpv_stop` resets per-file mpv state (A-B loop, speed, delays)
      - [ ] libmpv: forward `playlist_entry_id` on `start-file`/`end-file`
      - [ ] `MpvVideo` ignores `end-file` errors from a previous entry
      - [ ] Source switch: capture the playhead, mpv `stop` (flush) at
            once, then `play_magnet`, load at the captured time with the
            `resume` hint
      - [ ] enginefs: file-priority ack timeout no longer leaves the file
            unknown to the stream server (Kaleido-subs case)
      - [ ] Verify live: time from click to first frame, logged before/after
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- patch`, tag
            `v0.3.2`
- [ ] Continue-watching resume buffer (v0.4.0, see PLAN.md "Continue-watching
      resume buffer"): one commit each -
      - [ ] Stream handler records the byte ranges mpv reads until
            `file-loaded`, per torrent file
      - [ ] `media.rs`: time → byte offset from the container index
      - [ ] `save_resume_buffer` in `stop_playback`: open ranges + ~16 MB
            from the resume keyframe, whole verified pieces, before removal
      - [ ] `ProgressEntry.source` (magnet, file index, name); Resume
            prefers that source when still listed
      - [ ] Stream handler serves ranges from the buffer, engine reader
            (continue-watch anchored at the buffer's end) past it
      - [ ] `drop_resume_buffer` on dismiss / watched / replaced entry,
            Clear cache, startup sweep, 25 buffers / ~600 MB cap
      - [ ] Verify live: Resume from Continue watching → first frame time
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`, tag
            `v0.4.0`
- [ ] Context menus (v0.5.0, see PLAN.md "Context menus"): one commit each -
      - [ ] `tauri-plugin-clipboard-manager` (Rust, capability, JS)
      - [ ] `contextMenu.ts` store + `ContextMenu.tsx` component
      - [ ] Global listener: block native menu (dev Shift+right-click
            exempt), text-input Cut/Copy/Paste/Select all, selection Copy
      - [ ] Anime cards/posters + search dropdown + Continue watching
      - [ ] Episode rows + source/torrent rows
      - [ ] Player surface (flat, no Subtitles/Speed)
      - [ ] FILE_INDEX.md/PLAN.md sync, `npm run bump -- minor`, tag
            `v0.5.0`
- [ ] Build thumbnails button (v0.6.0, see PLAN.md "Build thumbnails
      button"): one commit each -
      - [ ] nyaa view-page scrape collects description image URLs
      - [ ] Frame thumbnail from a view-page image: frame filter, backend
            fetch, letterbox crop, 640px JPEG into the thumb cache; auto
            for rows without art
      - [ ] Screenshot groups: built-in yes/no lists (survey in PLAN.md),
            unknown groups checked once and remembered
      - [ ] `capture_torrent_thumbnail`: optional `file_idx` and `force`
      - [ ] Batch captures share one torrent across their episodes
      - [ ] Capture source pick: SubsPlease 480p (>= 1 seeder) first, else
            smallest file with >= 3, then >= 1
            seeders (batch = size / episodes, single episodes first); never
            0 seeders - row skipped as "no seeded source"
      - [ ] Live swarm check: no peer within 20s → next candidate (max 3)
      - [ ] Capture queue (list order, pause while playing, cancel)
      - [ ] Episodes header button with count/progress/failed state,
            spinner on rows being captured
      - [ ] Verify live on a show with no Kitsu/AniList thumbnails
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`, tag
            `v0.6.0`
- [ ] HD banner sources (v0.7.0, see PLAN.md "HD banner sources"): one
      commit each -
      - [ ] Fribb anime-lists id map: download, weekly refresh, offline
            lookup, arm-server fallback
      - [ ] `crates/artwork-client`: TMDB backdrops + logos
      - [ ] fanart.tv `showbackground`/`clearlogo`
      - [ ] Simkl fanart
      - [ ] `get_artwork` command + disk cache; frontend backdrop chain
            (media page, home hero, Continue watching)
      - [ ] TMDB episode stills as the first episode-thumbnail fallback
            after Kitsu/AniList (split-cour offset correction)
      - [ ] Title logos on the home hero and media page
      - [ ] Settings: user-supplied TMDB read access token (Bearer, never
            logged, not bundled); TMDB steps skipped without one
      - [ ] About attribution: TMDB logo (`src/assets/tmdb-logo.svg`) +
            notice, fanart.tv, Simkl; TMDB cache max 6 months
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`, tag
            `v0.7.0`
- [ ] Seek-bar thumbnail preview (v0.8.0, see PLAN.md "Seek-bar thumbnail
      preview"): one commit each -
      - [ ] `media.rs`: keyframe-only storyboard job (5s buckets, 240x135
            JPEG) over verified pieces only, revisiting as pieces arrive
      - [ ] Paced/low priority, pauses while buffering, stops with player
      - [ ] `storyboard_frame` command + cleanup in `stop_playback`
            (kept with a resume buffer)
      - [ ] Seek-bar hover shows the frame above the time tooltip
            (debounced, cached; time only when not downloaded)
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`, tag
            `v0.8.0`
