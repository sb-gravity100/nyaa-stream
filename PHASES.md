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
      out of that episode's group (`bestRelease` in `MediaPage.tsx`) —
      a manual source-picker dropdown is still not built, so this doesn't
      yet let the user override the auto-picked release
- [x] Surface torrent download/buffer progress in the UI while mpv is
      loading: `get_stream_stats`/`StreamStats` (torrent-engine + Tauri
      command), polled every 1s by `MediaPage.tsx` into a fixed playback
      overlay (progress %, speed, peers)
- [ ] File selection for multi-file/batch torrents (currently hardcoded to
      file index 0)
- [x] Persist a saved-anime library (`src/library.ts`, `localStorage`) —
      resolves the "TBD: sqlite vs. flat file" note for *this* use case;
      watch-history/continue-watching persistence is still open
- [x] Handle mpv exit (user closes player) and clean up the torrent
      session/player state: `watch_mpv_exit` polls the active mpv process
      (`MpvPlayer::try_wait`) every 500ms and runs the same cleanup
      (`cleanup_playback`) as the in-app Stop button when it exits on its
      own; `play_magnet` also defensively cleans up any leftover session
      before starting a new one

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
