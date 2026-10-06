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
- [ ] Season-aware source matching - v0.4.2 (patch: fix), see PLAN.md
      "Season-aware source matching". One commit each:
  - [x] Fixtures: crawled titles + expected entry/season
        (`src-tauri/tests/fixtures/nyaa_titles/`)
  - [x] Title format survey (`TITLE_ANALYSIS.md`)
  - [ ] `title_match.rs`: apostrophe-removing normalization
  - [ ] `title_match.rs`: release season markers (raw-title ranges,
        multi-season packs, `SxxEyy` precedence) + entry season numbers
        from titles
  - [ ] `title_match.rs`: reject other-season releases; base name +
        matching marker accepted
  - [ ] TVDB season numbers: fetch + cache Fribb/anime-lists, entry
        season set
  - [ ] Entry season fallback + sibling names from the AniList prequel
        chain; longest-name-wins assignment, kinds (movie/OVA/special)
        routed to their own entries
  - [ ] `build_candidates`: strip synonyms (roman numerals too), base-name
        query for season ≥2
  - [ ] `crates/release-parse` (MPL-2.0 anitopy port): faithful port
        first (tokenizer, keywords, number parsing), then elements (title,
        seasons, episodes, kind, group, version) with the TITLE_ANALYSIS §1-§2 fixes (roman seasons, multi-season packs, `~`
        ranges, `Nth Season`, `SxxEaa-bb`, CJK episodes, `Sx - ep`, `#ep`,
        `.5`); fixture-tested
  - [ ] Parsed elements on `NyaaResult`; frontend uses them instead of
        `episodeParser.ts` parsing
  - [ ] No-episode matched release → season batch of the matched entry
  - [ ] `TorrentDetails` file list (paths, folders, sizes) + per-release
        cache in `store.rs`
  - [ ] Unsure releases resolved from their file names (season union,
        matching file indexes)
  - [ ] Live check (Mushoku S1-S3, Slime S2-S4, Food Wars S1/S2) via
        `search_debug`
  - [ ] `chore: release v0.4.2` + tag (publish decision: see Release plan)

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
      - [x] enginefs: startup baseline priority 0, raised to 1 once the
            startup buffer verifies (or 15s fallback)
      - [x] enginefs: ~4 MB startup window at priority 7 + staggered
            deadlines; ~4 MB read-ahead at 7 after the first byte
      - [x] `play_magnet` `watch: first|resume` hint → engine, per file
      - [x] enginefs: first watch - sequential download from piece 0
      - [x] enginefs: continue watch - pieces before the resume point at 0,
            sequential from it, earlier pieces back to 1 when the rest is
            done or on a seek back
      - [x] enginefs: a seek into undownloaded data re-anchors
            continue-watch mode at the target (debounced ~300ms blocked
            read); never for background/probe/preload reads
      - [x] enginefs: waiting-piece log reports the effective window
      - [x] `VENDORED.md` notes for the three enginefs changes
      - [x] Spawn embedded mpv at app launch instead of first play
      - [x] `mpv_stop` resets per-file mpv state (A-B loop, speed, delays)
      - [x] libmpv: forward `playlist_entry_id` on `start-file`/`end-file`
      - [x] `MpvVideo` ignores `end-file` errors from a previous entry
      - [x] Source switch: capture the playhead, mpv `stop` (flush) at
            once, then `play_magnet`, load at the captured time with the
            `resume` hint
      - [x] enginefs: file-priority ack timeout no longer leaves the file
            unknown to the stream server (Kaleido-subs case)
      - [x] `StreamStats.downloaded_ranges`: verified pieces → byte runs →
            seconds via the cached container keyframe index (linear
            fallback), tiny runs merged
      - [x] Seek bar: dim downloaded layer under mpv's buffer layer
      - [x] mpv `--demuxer-max-back-bytes=150MiB`
      - [x] enginefs: reads/seeks into the file tail are ContainerMetadata
            (priority 7, pinned)
      - [x] enginefs: prefetch the file tail (4 MB) at playback start
      - [x] enginefs: resume anchor on the first non-contiguous,
            non-metadata forward jump
      - [x] enginefs: resume holds the baseline until the anchor's buffer
            verifies (or 15s)
      - [x] enginefs: read-ahead window >= 30s of playback by bitrate, cap
            64 MB
      - [x] mpv `--cache-pause-initial=yes` + `--cache-pause-wait=10`
            (start/resume once 10s is buffered)
      - [x] Seek re-anchor debounce 300ms → 100ms
      - [x] Capture mpv warn/error log messages (rate limited)
      - [x] Settings: hardware decoding Auto / Off (mpv `hwdec`)
      - [x] Tail prefetch 1% of the file, clamped 4-16 MB
      - [x] Vendor libtorrent-sys: whole_pieces_threshold 2, strict end
            game off, request_queue_time 1
      - [x] enginefs: serve pieces verified < 30s ago from libtorrent's own
            copy only (stale-bytes corruption)
      - [x] Home hero: grow to fit, clamp long titles to 2 lines
      - [x] libmpv: corruption log lines → `stream-corrupt` event
      - [x] MpvVideo: drop-buffers + exact seek to the current time on
            `stream-corrupt` (5s debounce, max 3 per 60s)
      - [x] What's new dialog after an in-app update (`changelog.ts`,
            `updatedFrom` marker set before install, `WhatsNew.tsx`)
      - [x] Splash screen: animated logo inline in index.html, faded out
            after the first render
      - [x] Logo polish: new mark in BrandMark, splash, favicon, icon.svg;
            app icons regenerated
      - [ ] Verify live: time from click to first frame, logged before/after
      - [x] PLAN.md/FILE_INDEX.md sync, `npm run bump -- patch`, tag
            `v0.3.2`, `npm run release -- --notes "..." --dry-run`, then
            publish (see PLAN.md "Release plan")
- [ ] Native splash window + custom title bar (v0.4.0, see PLAN.md "Native
      splash window and custom title bar"): one commit each -
      - [x] Splash window (`public/splash.html`), main window hidden until
            `app_ready`, 20s fallback; drop the inline index.html splash
      - [x] Frameless main window + title bar strip blended with the app
            bar: drag, double-click maximize, min / max / close, resize edges
      - [x] Player: strip fades with the top controls; hidden in fullscreen
            and PiP
      - [x] Window capability permissions
- [ ] Download cache (v0.4.0, see PLAN.md "Download cache"): one commit each -
      - [x] `downloads/.cache-index.json`: files, bytes, last_used, episode
      - [x] LRU eviction to the cap after stop and at startup (never the
            playing torrent)
      - [x] Continue-watching episodes kept past the cap until they leave
            the row
      - [x] Settings "Download cache" size (default 10 GB, 0 = delete on
            stop) + usage; Clear cache empties downloads
      - [x] Fast resume: comes with sbtl (`downloads/.sbtl/<hash>.resume`),
            no libtorrent-sys work
      - [ ] Verify live: a cached re-open skips the re-check
- [ ] Continue-watching resume buffer (v0.4.0, see PLAN.md "Continue-watching
      resume buffer"): one commit each -
      - [x] Stream handler records the byte ranges mpv reads until
            `file-loaded`, per torrent file
      - [x] `media.rs`: time → byte offset from the container index
      - [x] `save_resume_buffer` in `stop_playback`: open ranges + ~16 MB
            from the resume keyframe, whole verified pieces, before removal
      - [x] `ProgressEntry.source` (magnet, file index, name); Resume
            prefers that source when still listed
      - [x] Stream handler serves ranges from the buffer, engine reader
            (continue-watch anchored at the buffer's end) past it
      - [x] `drop_resume_buffer` on dismiss / watched / replaced entry,
            Clear cache, startup sweep, 25 buffers / ~600 MB cap
      - [ ] Verify live: Resume from Continue watching → first frame time
- [ ] Subtitle preloading (v0.4.0, see PLAN.md "Subtitle preloading"): one
      commit each -
      - [x] Off = `sub-visibility=no` with `sid` kept (no re-demux buffering
            when cycling EN / off)
      - [x] Preload: default `sid` set before `loadfile`; re-assert once after
            first `playing` if `sub-text` is empty
      - [ ] If a different-track switch still stalls: enginefs subtitle-range
            metadata reads
      - [ ] Verify live: subs visible on first play without toggling; EN / off cycling no
            longer shows the buffering spinner
- [ ] Load profiler (v0.4.0, see PLAN.md "Load profiler"): one commit each -
      - [x] `load_trace.rs`: trace registry, `trace_begin` / `trace_mark`,
            stage debug lines, completion/abandon summary
      - [x] Backend stages in `play_magnet`: torrent_add, metadata, checking
            (250ms state poll), resume_buffer
      - [x] Stream readers time engine Pending reads; per-file counters
            snapshotted into the trace (+ buffer vs torrent bytes)
      - [x] Frontend marks: player open, loadfile, file-loaded, first frame;
            seek / Resume traces from `mpvVideo`
      - [ ] Verify live: cold start, cached re-open, resume, seek into
            undownloaded data; decide fast resume from the numbers
- [ ] sbtl torrent engine (v0.4.0, see PLAN.md "sbtl backend" and "Release
      plan"): one commit each -
      - [x] sbtl backend in enginefs; review fixes; renamed tl -> sbtl,
            git dependency on github.com/sb-gravity100/sbtl tag `v0.1.0`
      - [x] sbtl is the only engine: libtorrent/librqbit backends,
            `vendor/libtorrent-sys` and engine features removed; vcpkg
            builds only FFmpeg
      - [x] Download cache keeps `downloads/.sbtl/`, evicts a torrent's
            sbtl resume/metadata files
      - [x] `release.mjs` refuses a build without a tagged sbtl;
            `npm run dev:sbtl-local`
      - [x] Baseline live test on the last libtorrent commit (main at
            `031d9c0`): cold start, cached re-open, resume, seek into
            undownloaded data; record the numbers in PLAN.md
      - [x] Merge `sbtl-backend` into main (`--no-ff`, `1f95811`)
      - [x] sbtl_engine (see PLAN.md "sbtl_engine"): crate skeleton with
            `Engine`/`Torrent`/`Reader`, sbtl backend logic moved in
      - [x] sbtl_engine: trackers (defaults, daily list, RTT ranking) and
            the lease / idle-removal loop, with unit tests
      - [x] torrent-engine on sbtl_engine (`lib.rs`, `direct_input.rs`,
            resume buffers, `StreamStats`, checking probe, engine_smoke);
            watch hint removed end to end
      - [x] Remove `vendor/enginefs`, the `[patch]` and the enginefs git
            dependency; docs (PLAN, FILE_INDEX, CLAUDE, README)
      - [x] engine_smoke on a local swarm: numbers vs "sbtl backend"
            (no regression, see PLAN.md "sbtl_engine" As built)
      - [x] Same live test on sbtl, a poorly seeded torrent, `engine_smoke`
            (PLAN.md "sbtl_engine" Live test)
      - [x] Fix: resume buffers never survived a save (`list()` included the
            save's own `.tmp`)
      - [x] Fix: one fixed progress key per player session (MediaPage's
            route label) for play_magnet, watch progress and the resume
            request - group relabeling ("Season 2 Episode 10" -> "Episode
            10") re-ran play_magnet and saved the buffer under a key
            Continue watching didn't know, so it was swept; Resume matches
            the saved source by info-hash and plays its saved magnet when
            not listed (yet)
      - [x] Live: resumes served from resume buffers (Fate Ep 11 41 MB,
            Arknights 38 MB, Fate Ep 7 49 MB, Mushoku S2 Ep 10 24 MB with
            no torrent bytes) in 134-415 ms; one play_magnet per click;
            unlisted saved sources played from their magnet
      - [ ] Known issue: a dropped buffer (episode left Continue watching)
            sometimes fails to delete with os error 32 seconds after it
            was written, leaving an empty dir that clears after a restart
            (likely an antivirus scan holding the new data.bin); re-saving
            that episode in the same session may fail. Confirm the holder
            (handle.exe / Process Monitor), then retry or rename-aside
      - [~] libtorrent-era partial download reopened on sbtl: not testable
            (data cleared); accepted - sbtl re-checks data on add, keeps
            verified pieces only
      - [ ] Run `npm run dev:sbtl-local` once (confirms the patch path
            resolves from cargo's working directory under `tauri dev`)
- [ ] Contact and send logs (v0.4.0, see PLAN.md "Contact and send
      logs"): one commit each -
      - [x] `export_logs`: zip logs + `system.txt` into Downloads, reveal;
            Windows profile path/username replaced in the zipped copies
      - [x] Settings Help section: Contact modal (repo + Discord profile)
      - [x] Send logs modal: contents notice, Discord / GitHub (prefilled
            issue, public notice) buttons
      - [x] Discord contact: username `__sb______` shown with a Copy button
            (a profile link needs the numeric user id)
      - [x] PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`, tag
            `v0.4.0`, push the tag, build + publish the release (see PLAN.md
            "Release plan") - published 2026-10-04; notes fixed up after npm
            cut them to one line (`release.mjs --notes-file` since)
- [ ] Context menus (v0.5.0, see PLAN.md "Context menus"): one commit each -
      - [ ] `tauri-plugin-clipboard-manager` (Rust, capability, JS)
      - [ ] `contextMenu.ts` store + `ContextMenu.tsx` component
      - [ ] Global listener: block native menu (dev Shift+right-click
            exempt), text-input Cut/Copy/Paste/Select all, selection Copy
      - [ ] Anime cards/posters + search dropdown + Continue watching
      - [ ] Episode rows + source/torrent rows
      - [ ] Player surface (flat, no Subtitles/Speed)
      - [ ] FILE_INDEX.md/PLAN.md sync, `npm run bump -- minor`, tag
            `v0.5.0` locally (not pushed/published - see PLAN.md "Release plan")
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
            `v0.6.0` locally (not pushed/published - see PLAN.md "Release plan")
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
            `v0.7.0` locally (not pushed/published - see PLAN.md "Release plan")
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
            `v0.8.0` locally (not pushed/published - see PLAN.md "Release plan")
- [ ] Navigation and home rehaul (v0.9.0, see PLAN.md "Navigation and home
      rehaul + Discover"): one commit each -
      - [ ] Left rail component + layout shell (hidden in the player)
      - [ ] Routes `#/library`, `#/discover`, `#/discover/browse`
      - [ ] Library page; Home "Your library" row
      - [ ] Search moved into the rail (+ global shortcut)
      - [ ] Home restyled for the rail layout
      - [ ] Verify live; PLAN.md/FILE_INDEX.md sync, `npm run bump -- minor`,
            tag `v0.9.0` locally (not pushed/published)
- [ ] Discover (v0.9.1, see PLAN.md "Navigation and home rehaul + Discover"):
      one commit each -
      - [ ] `anilist-client` queries: trending, season, upcoming, popular,
            filtered browse, recommendations, airing schedules (+ tests)
      - [ ] Tauri commands + session/disk cache; Kitsu fallback where it exists
      - [ ] Discover page rows (trending / season / upcoming / popular)
      - [ ] Browse page: filters, sort, infinite scroll, URL state
      - [ ] Recommendations rows from library/watch history
      - [ ] Airing schedule calendar
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- patch`, tag
            `v0.9.1` locally (not pushed/published)
- [ ] 32-bit Windows (v1.0.0, see PLAN.md "32-bit Windows support"): one
      commit each -
      - [ ] Audit 64-bit assumptions (`usize` casts, sizes/offsets as `u64`,
            memory caps) across crates
      - [ ] `i686-pc-windows-msvc` target + vcpkg `x86-windows-static-md`
            FFmpeg build, sbtl's C sources for i686; Build prerequisites
            section
      - [ ] 32-bit `libmpv-2.dll` in `src-tauri/lib/x86/`; arch-aware loader
      - [ ] Per-arch memory caps (mpv cache, read-ahead, resume buffers)
      - [ ] Tauri bundle per arch; `latest.json` `windows-i686`;
            `scripts/release.mjs` builds and uploads both
      - [ ] Verify live on a 32-bit build (WoW64 or VM): playback, subs,
            resume, HLS fallback, updater
- [ ] Stable release (v1.0.0, see PLAN.md "Release plan"): one commit each -
      - [ ] Stabilization pass over v0.4.0-v0.8.0: open bugs, live verify
            items still unchecked, README/What's new
      - [ ] PLAN.md/FILE_INDEX.md sync, `npm run bump -- major`, tag
            `v1.0.0`, push the tag, build + publish the release
- [ ] Local release tooling (see PLAN.md "Release plan"): one commit each -
      - [x] `scripts/release.mjs` + `npm run release` (preflight, signed
            build, `latest.json`, `gh release create`; `--dry-run`,
            `--skip-build`)
      - [x] `release.yml`: manual `workflow_dispatch` trigger only
      - [x] CLAUDE.md versioning section + FILE_INDEX.md
- [ ] Dev build speed (see PLAN.md "Dev build speed"): one commit each -
      - [x] Dev profile debug info: line-tables-only, dependencies 0
      - [x] rust-lld for dev links (verified + timed)
      - [x] rust-analyzer separate target dir (`.vscode/settings.json`)
