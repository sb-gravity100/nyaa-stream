# PHASES.md — nyaa-stream

## Phase 0 — Scaffolding (done)
- [x] Tauri + Preact-TS project scaffolded
- [x] Cargo workspace with `torrent-engine`, `nyaa-client`, `anilist-client`,
      `mpv-ipc` crates
- [x] `reference/stremio-core` cloned for architecture reference
- [x] Tauri commands wired: `search_anime`, `search_torrents`,
      `play_magnet`, `set_pause`
- [x] `cargo check --workspace` passes clean

## Phase 1 — Prove the pipeline end-to-end
- [ ] Frontend: replace scaffold `App.tsx` with a search box + results list
      calling `search_anime` and `search_torrents`
- [ ] Frontend: "play" button calls `play_magnet` and shows basic
      play/pause controls wired to `set_pause`
- [ ] Manually verify: search a real anime title, pick a real nyaa.si
      release, confirm mpv opens and streams it
- [ ] Fix whatever librqbit/axum-range API mismatches `cargo check` turns up

## Phase 2 — Metadata-to-release matching
- [ ] Map AniList entry -> episode list
- [ ] Parse episode numbers out of nyaa.si release titles (fansub naming is
      inconsistent — needs a real parser, not just regex-and-hope)
- [ ] Group releases by fansub group / quality, let user pick a preferred
      group per-anime

## Phase 3 — Playback quality of life
- [ ] Surface torrent download/buffer progress in the UI while mpv is
      loading
- [ ] File selection for multi-file/batch torrents (currently hardcoded to
      file index 0)
- [ ] Persist watch history / continue-watching (needs a local store — TBD:
      sqlite vs. flat file)
- [ ] Handle mpv exit (user closes player) and clean up the torrent
      session/player state

## Phase 4 — Library and polish
- [ ] Local library / watchlist backed by AniList's own list if the user
      authenticates (optional OAuth — out of scope until asked for)
- [ ] Settings: mpv path override, download directory, preferred fansub
      groups, subtitle preferences
- [ ] Packaging: Tauri bundler for Windows installer; document the mpv
      system-dependency requirement clearly at install/first-run
