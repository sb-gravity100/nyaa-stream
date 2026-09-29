# commits.md

Commit log, newest first. Prepend a new entry after every commit.

### `4680382` · 2026-09-29 · docs: note v0.3.2 is published in CLAUDE.md
- `CLAUDE.md`

### `8092cd0` · 2026-09-29 · docs: publish v0.3.2 as its own release
- `PLAN.md`
- `PHASES.md`

### `9870116` · 2026-09-29 · fix: let a release dry run proceed without the release tag
- `scripts/release.mjs`
- `PLAN.md`

### `9da2a4d` · 2026-09-29 · docs: point CLAUDE.md publishing at the local release script
- `CLAUDE.md`
- `PHASES.md`

### `e506b39` · 2026-09-29 · docs: index the local release script in FILE_INDEX.md
- `FILE_INDEX.md`
- `PHASES.md`

### `cd74354` · 2026-09-29 · ci: run the release workflow manually only
- `.github/workflows/release.yml`

### `ae8e139` · 2026-09-29 · chore: add local release script
- `scripts/release.mjs`
- `package.json`

### `bac1f3a` · 2026-09-29 · docs: plan local release script
- `PLAN.md`
- `PHASES.md`

### `963a00c` · 2026-09-29 · docs: record fast playback start implementation decisions
- `PLAN.md`

### `bcabb4f` · 2026-09-29 · docs: sync FILE_INDEX.md with the fast playback start changes
- `FILE_INDEX.md`

### `4ca51da` · 2026-09-29 · fix: stream anyway when file priorities miss their ack timeout
- `PHASES.md`
- `vendor/enginefs/VENDORED.md`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`
- `vendor/enginefs/src/engine.rs`

### `af7d8f3` · 2026-09-29 · fix: flush mpv and resume at the playhead on a source switch
- `PHASES.md`
- `src/PlayerView.tsx`
- `src/mpvVideo.ts`

### `80e71ff` · 2026-09-29 · fix: ignore end-file errors from a previous mpv entry
- `PHASES.md`
- `src/mpvVideo.ts`

### `e7236c9` · 2026-09-29 · fix: forward mpv's playlist_entry_id on start-file and end-file
- `PHASES.md`
- `crates/mpv-player/src/libmpv.rs`

### `2a774cc` · 2026-09-29 · fix: reset per-file mpv state in mpv_stop
- `PHASES.md`
- `src-tauri/src/player.rs`

### `d1be839` · 2026-09-29 · fix: spawn the embedded mpv at app launch
- `PHASES.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `9ae82c6` · 2026-09-29 · docs: note the fast-start enginefs changes in VENDORED.md
- `PHASES.md`
- `vendor/enginefs/VENDORED.md`

### `cd1b10e` · 2026-09-29 · fix: log the effective window in the waiting-piece diagnostic
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `27ed9a8` · 2026-09-29 · fix: re-anchor continue-watch mode at a seek into undownloaded data
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`
- `vendor/enginefs/src/backend/priorities.rs`

### `6beced4` · 2026-09-29 · fix: download a continue-watch file in order from its resume point
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`

### `c60fd83` · 2026-09-29 · fix: download a first-watch file sequentially from piece 0
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`

### `488943f` · 2026-09-29 · docs: document watch hint delivery
- `FILE_INDEX.md`
- `PLAN.md`

### `555c5c6` · 2026-09-29 · feat: pass a first/resume watch hint from play_magnet to the engine
- `PHASES.md`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`
- `src/playback.ts`
- `vendor/enginefs/src/backend/libtorrent/handle.rs`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/priorities.rs`

### `14ecb3e` · 2026-09-29 · fix: widen the startup window and read-ahead to ~4 MB at priority 7
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/src/backend/priorities.rs`

### `542f1ee` · 2026-09-29 · fix: hold the playing file at priority 0 until its startup buffer verifies
- `PHASES.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`
- `vendor/enginefs/src/backend/priorities.rs`

### `efa6aa9` · 2026-09-29 · docs: note unpublished milestone tags in CLAUDE.md
- `CLAUDE.md`

### `75dcc97` · 2026-09-29 · docs: plan local-only milestone tags, publish only v0.9.0
- `PLAN.md`
- `PHASES.md`

### `dcd2fc0` · 2026-09-29 · docs: hide Windows username in exported logs
- `PLAN.md`
- `PHASES.md`

### `a93e779` · 2026-09-29 · docs: plan contact and send logs for v0.9.0
- `PLAN.md`
- `PHASES.md`

### `5615357` · 2026-09-29 · docs: plan user-supplied TMDB token for v0.7.0
- `PLAN.md`
- `PHASES.md`

### `14f6a12` · 2026-09-29 · docs: plan TMDB episode stills and seek-bar thumbnail preview
- `PLAN.md`
- `PHASES.md`

### `b9e7832` · 2026-09-29 · docs: drop AniDB research from PLAN.md
- `PLAN.md`

### `ee97383` · 2026-09-29 · docs: record AniDB API research
- `PLAN.md`

### `911aef4` · 2026-09-29 · docs: note nyaa screenshot groups for episode thumbnails
- `PLAN.md`
- `PHASES.md`

### `7bbe309` · 2026-09-29 · docs: plan nyaa view-page screenshots as thumbnails
- `PLAN.md`
- `PHASES.md`

### `c496a57` · 2026-09-29 · docs: plan HD banner sources for v0.7.0
- `PLAN.md`
- `PHASES.md`

### `5272044` · 2026-09-29 · docs: prefer SubsPlease 480p for thumbnail captures
- `PLAN.md`
- `PHASES.md`

### `536e008` · 2026-09-29 · docs: require seeders for thumbnail capture sources
- `PLAN.md`
- `PHASES.md`

### `62a7824` · 2026-09-29 · docs: plan build thumbnails button for v0.6.0
- `PLAN.md`
- `PHASES.md`

### `ca6ed50` · 2026-09-29 · docs: plan continue-watching resume buffer for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `bc65313` · 2026-09-29 · docs: plan far seeks re-anchoring continue-watch mode
- `PLAN.md`
- `PHASES.md`

### `fe731c2` · 2026-09-29 · docs: plan source switch flush and seek back for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `6dc6182` · 2026-09-29 · docs: plan stale source-switch error fix for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `7c3cd69` · 2026-09-29 · docs: split sequential download plan into first and continue watch
- `PLAN.md`
- `PHASES.md`

### `73850e7` · 2026-09-29 · docs: plan sequential download for from-start playback
- `PLAN.md`
- `PHASES.md`

### `7eefcf4` · 2026-09-29 · docs: plan fast playback start for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `3e7293d` · 2026-09-29 · chore: release v0.3.1
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `6f37211` · 2026-09-29 · docs: note hero slide in FILE_INDEX.md
- `FILE_INDEX.md`

### `f37c0ef` · 2026-09-29 · feat: slide the home hero between featured shows
- `src/App.css`
- `src/HomePage.tsx`

### `3bacb2e` · 2026-09-29 · fix: give the home hero a fixed, shorter height
- `src/App.css`

### `11a1aff` · 2026-09-29 · docs: plan custom context menus for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `978f077` · 2026-09-29 · docs: correct signing key env var and add manual release steps
- `CLAUDE.md`

### `4a26a8b` · 2026-09-29 · ci: build and publish signed releases on version tags
- `.github/workflows/release.yml`
- `FILE_INDEX.md`

### `4d3c28b` · 2026-09-29 · ci: build and publish signed releases on version tags
- `.github/workflows/release.yml`
- `FILE_INDEX.md`
- `Cargo.lock`

### `0295aac` · 2026-09-29 · chore: release v0.3.0
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `9234c19` · 2026-09-29 · docs: rewrite README as a full user and developer guide
- `README.md`

### `27ea4ea` · 2026-09-29 · docs: document npm run setup in README and FILE_INDEX
- `README.md`
- `FILE_INDEX.md`

### `1924b71` · 2026-09-29 · build: fetch prebuilt native libs instead of building with vcpkg
- `scripts/fetch-native-deps.mjs`
- `package.json`
- `README.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `ceff8b6` · 2026-09-29 · docs: rewrite README, remove releasing section
- `README.md`

### `9e510a1` · 2026-09-29 · docs: add README
- `README.md`

### `093d71b` · 2026-09-29 · chore(updater): point endpoint at sb-gravity100/nyaa-stream
- `src-tauri/tauri.conf.json`

### `1e9cd69` · 2026-09-29 · feat(updater): in-app auto-update via GitHub Releases
- `src/updater.ts`
- `src/SettingsPanel.tsx`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/capabilities/default.json`
- `vite.config.ts`
- `FILE_INDEX.md`

### `2597a05` · 2026-09-29 · chore: remind to bump the version on every change
- `.githooks/pre-commit`
- `CLAUDE.md`

### `ae9e472` · 2026-09-29 · feat(player): picture-in-picture mini always-on-top window (P / button)
- `src/pip.ts`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/icons.tsx`
- `src/SettingsPanel.tsx`
- `src-tauri/capabilities/default.json`
- `FILE_INDEX.md`

### `1fb8bc8` · 2026-09-29 · chore: release v0.2.0
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `0b76a74` · 2026-09-29 · feat(logging): write a rolling daily log file (release has no console)
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`
- `FILE_INDEX.md`

### `305b5cf` · 2026-09-29 · fix(build): pin @tauri-apps/plugin-dialog to 2.7.3 to match the crate
- `package.json`
- `package-lock.json`

### `01479ba` · 2026-09-29 · feat(installer): branded MSI banner and welcome-dialog art
- `scripts/make-installer-art.ps1`
- `src-tauri/installer/banner.bmp`
- `src-tauri/installer/dialog.bmp`
- `src-tauri/tauri.conf.json`
- `FILE_INDEX.md`

### `4446025` · 2026-09-29 · feat(release): add bump script keeping app version in sync
- `scripts/bump-version.mjs`
- `package.json`
- `FILE_INDEX.md`

### `574dd5e` · 2026-09-29 · perf(frontend): drop unused Zen Kaku 500 weight (CSS 467kB -> 329kB)
- `src/main.tsx`

### `0cce65e` · 2026-09-29 · perf(build): add release profile (thin LTO, strip symbols)
- `Cargo.toml`

### `bef7eca` · 2026-09-29 · perf(frontend): lazy-load HLS fallback player (main chunk 794kB -> 161kB)
- `src/PlayerView.tsx`

### `edae935` · 2026-09-29 · fix(player): subtitle button tooltip lists the real mpv shortcuts
- `src/PlayerView.tsx`

### `9e199a5` · 2026-09-29 · feat: upscale and sharpen hero and backdrop art
- `src/enhanceImage.ts`
- `src/HomePage.tsx`
- `src/MediaPage.tsx`
- `FILE_INDEX.md`

### `89c310e` · 2026-09-29 · feat(player): preload the next episode of a batch at low priority and keep playing from that batch
- `src/PlayerView.tsx`
- `FILE_INDEX.md`
- `PLAN.md`

### `f8453f8` · 2026-09-29 · feat: preload_next_file command; same-torrent episode changes keep the torrent
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`

### `737ce7f` · 2026-09-29 · feat(enginefs): low-priority preload file next to the playing one
- `vendor/enginefs/`

### `2cae1b1` · 2026-09-29 · fix(player): a late mpv_stop from the previous episode no longer hides the new video
- `src-tauri/src/player.rs`

### `0788f6b` · 2026-09-29 · feat: AniList bannerImage as backdrop fallback when Kitsu has no cover
- `crates/anilist-client/src/lib.rs`
- `src-tauri/src/metadata_fallback.rs`
- `src/types.ts`
- `src/browserFallback.ts`
- `src/watchProgress.ts`
- `src/HomePage.tsx`
- `src/MediaPage.tsx`

### `a712dec` · 2026-09-29 · style(home): shorten hero height
- `src/App.css`

### `631837f` · 2026-09-29 · feat: blank episode thumbnails fall back to the local frame cache
- `src/MediaPage.tsx`

### `d2ffba2` · 2026-09-29 · fix(player): restyle ASS subtitles once mpv has loaded the track header
- `src/mpvVideo.ts`
- `src/PlayerView.tsx`

### `b520776` · 2026-09-29 · feat(player): remove the pause dimming
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/App.css`

### `7a26759` · 2026-09-29 · feat(player): remove the centre paused glyph overlay
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/App.css`

### `d38ff57` · 2026-09-29 · feat(player): , and . step one frame back/forward; subtitle delay moves to - and =
- `src/mpvVideo.ts`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `4961d54` · 2026-09-29 · feat(player): remove the centre play/pause/skip icon overlays
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/App.css`

### `32d3eb5` · 2026-09-29 · fix(player): restore STATS_POLL_MS and drop the unused flash constant
- `src/PlayerView.tsx`

### `b9925ec` · 2026-09-29 · fix(player): keyboard gestures no longer bring up the controls while the mouse is idle
- `src/PlayerView.tsx`

### `b981b6e` · 2026-09-29 · feat(player): C toggles subtitles, hold C + scroll steps through tracks
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `d3bc670` · 2026-09-29 · feat(player): X saves the current frame; subtitle delay moves to , and .
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/settings.ts`
- `FILE_INDEX.md`

### `64faaf1` · 2026-09-29 · feat: mpv_save_frame saves the current frame as a PNG in a folder
- `src-tauri/src/player.rs`
- `src-tauri/src/lib.rs`

### `5723f87` · 2026-09-29 · feat(player): Include subtitles option in the export dialog
- `src/subtitles.ts`
- `src/ExportDialog.tsx`
- `src/PlayerView.tsx`
- `src/App.css`
- `FILE_INDEX.md`

### `4f9a0c8` · 2026-09-29 · feat: export_clip can burn the active subtitle track into the clip
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `b614fc9` · 2026-09-29 · feat(mpv-player): encode clips with burned-in subtitles through libmpv
- `crates/mpv-player/src/encode.rs`
- `crates/mpv-player/src/lib.rs`
- `crates/mpv-player/examples/encode_smoke.rs`

### `49099cf` · 2026-09-29 · feat(player): export dialog disables all controls while exporting
- `src/ExportDialog.tsx`
- `src/App.css`

### `772c709` · 2026-09-29 · fix(player): fallback opening skip is 90s, not 85s
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `7803545` · 2026-09-29 · feat(player): batch sources play their matched episode file without a file picker
- `src/PlayerView.tsx`
- `src/App.css`
- `FILE_INDEX.md`

### `1bd0413` · 2026-09-29 · fix: first-season pages no longer list later seasons' episodes
- `src/App.tsx`

### `30b2714` · 2026-09-29 · feat(player): export dialog for A-B clips
- `src/ExportDialog.tsx`
- `src/PlayerView.tsx`
- `src/App.css`
- `FILE_INDEX.md`

### `8332730` · 2026-09-29 · feat: export_clip takes a destination folder (dialog plugin, reveal permission)
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/capabilities/default.json`
- `src/settings.ts`
- `package.json`, `package-lock.json`

### `3c3d255` · 2026-09-29 · fix(player): Shift skips past the whole opening, even from before it starts
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `16ec8da` · 2026-09-29 · feat(player): Shift skips to the end of the current chapter, not a fixed 85s
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `27e7a1e` · 2026-09-29 · fix: keep season packs, packs with extras and single episodes apart
- `src/episodeParser.ts`
- `src/App.tsx`
- `src/releases.ts`

### `2bc44ed` · 2026-09-29 · docs: describe the release database and phased search
- `FILE_INDEX.md`
- `PLAN.md`

### `15839be` · 2026-09-29 · feat: show pages paint from the local database and preferred fansubber before the full search
- `src/App.tsx`
- `src/SettingsPanel.tsx`

### `dba15c7` · 2026-09-29 · feat: local-first show search, fansubber-only search and popular fansubbers commands
- `src-tauri/src/lib.rs`
- `crates/nyaa-client/Cargo.toml`
- `crates/nyaa-client/examples/store_debug.rs`

### `28205cf` · 2026-09-29 · feat(nyaa-client): permanent release database keyed by nyaa release id
- `crates/nyaa-client/Cargo.toml`
- `crates/nyaa-client/src/store.rs`
- `crates/nyaa-client/src/lib.rs`
- `FILE_INDEX.md`

### `7403cde` · 2026-09-29 · feat(nyaa-client): per-uploader search and cached popular fansubbers
- `crates/nyaa-client/src/fansubbers.rs`
- `crates/nyaa-client/src/lib.rs`
- `FILE_INDEX.md`

### `8992f5d` · 2026-09-29 · feat: smarter nyaa title matching - fewer queries run in parallel, false positives dropped
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`
- `FILE_INDEX.md`

### `32d4154` · 2026-09-29 · feat(anilist): carry AniList synonyms as extra release-search names
- `crates/anilist-client/src/lib.rs`
- `src-tauri/src/metadata_fallback.rs`
- `src/types.ts`
- `src/App.tsx`

### `9765d15` · 2026-09-29 · feat(nyaa-client): throttle requests and back off on 429/503
- `crates/nyaa-client/src/throttle.rs`
- `crates/nyaa-client/src/lib.rs`
- `FILE_INDEX.md`

### `e844730` · 2026-09-29 · feat(settings): preferred fansubber text input for release picking
- `src/settings.ts`
- `src/releases.ts`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/SettingsPanel.tsx`
- `FILE_INDEX.md`, `PHASES.md`

### `2e4f465` · 2026-09-29 · fix: search dropdown had two nested scrollbars
- `src/App.css`

### `8e74f35` · 2026-09-29 · docs: describe in-process libmpv playback
- `CLAUDE.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `ee3c6f8` · 2026-09-29 · feat(settings): mpv path override now points at libmpv-2.dll
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/settings.ts`

### `1c1c758` · 2026-09-29 · build: bundle libmpv-2.dll as a Tauri resource
- `.gitignore`
- `src-tauri/tauri.conf.json`
- `src-tauri/lib/README.md`

### `dd6e7ad` · 2026-09-29 · feat(mpv-player): play through in-process libmpv instead of spawned mpv
- `crates/mpv-player/Cargo.toml`
- `crates/mpv-player/src/libmpv.rs`
- `crates/mpv-player/src/embedded.rs`
- `crates/mpv-player/src/lib.rs`
- `crates/mpv-player/examples/smoke.rs`
- `src-tauri/src/player.rs`
- `Cargo.lock`

### `3aabc6c` · 2026-09-29 · refactor: rename mpv-ipc crate to mpv-player
- `crates/mpv-player/`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`
- `Cargo.lock`
- `CLAUDE.md`, `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`

### `bad40af` · 2026-09-29 · docs: sync PHASES and FILE_INDEX with new features
- `FILE_INDEX.md`
- `PHASES.md`

### `2f86de2` · 2026-09-29 · feat: library/progress/settings backup export and import
- `src-tauri/src/cache.rs`
- `src-tauri/src/lib.rs`
- `src/SettingsPanel.tsx`
- `src/backup.ts`

### `e84eae8` · 2026-09-29 · feat: home hero cycle, library sort/filter/badges, dismiss continue watching
- `src/App.css`
- `src/HomePage.tsx`
- `src/watchProgress.ts`

### `47fddf4` · 2026-09-29 · feat: airing countdown, mark-previous-watched and no-seeder row flag on the media page
- `crates/anilist-client/src/lib.rs`
- `src-tauri/src/metadata_fallback.rs`
- `src/App.css`
- `src/MediaPage.tsx`
- `src/types.ts`

### `e72d44e` · 2026-09-29 · feat: recent searches in the search dropdown
- `src/App.css`
- `src/App.tsx`
- `src/recentSearches.ts`

### `711714f` · 2026-09-29 · feat: release badges, seeder health, per-show quality and no-seeder warning
- `src/App.css`
- `src/PlayerView.tsx`
- `src/releases.ts`

### `2c438a7` · 2026-09-29 · feat: media keys and taskbar progress in the player
- `src-tauri/Cargo.toml`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/lib.rs`
- `src-tauri/src/media_keys.rs`
- `src/PlayerView.tsx`

### `37f4c3e` · 2026-09-29 · feat: settings storage section, mpv path field, new shortcuts listed
- `src/App.css`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/settings.ts`

### `98c75d8` · 2026-09-29 · feat: cache size/clear commands, HLS purge on start/exit, mpv path override
- `crates/mpv-ipc/src/embedded.rs`
- `crates/mpv-ipc/src/lib.rs`
- `src-tauri/src/cache.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `ed6899b` · 2026-09-29 · feat: player A-B loop with clip export
- `src/App.css`
- `src/PlayerView.tsx`

### `129b388` · 2026-09-29 · feat: lazy-load search dropdown results and covers
- `src/App.css`
- `src/App.tsx`

### `566c617` · 2026-09-29 · feat: export_clip backend (A-B cut, NVENC H.264 + AAC)
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`
- `src-tauri/src/lib.rs`

### `fac9a87` · 2026-09-29 · feat: player speed, chapter marks, skip-intro (Shift) and audio menu
- `src/App.css`
- `src/PlayerView.tsx`
- `src/mpvVideo.ts`

### `5efcbc6` · 2026-09-29 · feat: brand-mark favicon
- `index.html`
- `public/favicon.svg`
- `public/tauri.svg`
- `public/vite.svg`

### `32554f3` · 2026-09-29 · docs: note Outfit in FILE_INDEX
- `FILE_INDEX.md`

### `e5c1138` · 2026-09-29 · style: Outfit display face for titles and labels
- `package-lock.json`
- `package.json`
- `src/App.css`
- `src/main.tsx`

### `616fff8` · 2026-09-29 · feat(icons): app icons from the brand mark
- `src-tauri/icons/128x128.png`
- `src-tauri/icons/128x128@2x.png`
- `src-tauri/icons/32x32.png`
- `src-tauri/icons/Square107x107Logo.png`
- `src-tauri/icons/Square142x142Logo.png`
- `src-tauri/icons/Square150x150Logo.png`
- `src-tauri/icons/Square284x284Logo.png`
- `src-tauri/icons/Square30x30Logo.png`
- `src-tauri/icons/Square310x310Logo.png`
- `src-tauri/icons/Square44x44Logo.png`
- `src-tauri/icons/Square71x71Logo.png`
- `src-tauri/icons/Square89x89Logo.png`
- `src-tauri/icons/StoreLogo.png`
- `src-tauri/icons/icon.icns`
- `src-tauri/icons/icon.ico`
- `src-tauri/icons/icon.png`
- `src-tauri/icons/icon.svg`

### `f7b1269` · 2026-09-29 · style(media): widen the info column and its description
- `src/App.css`

### `dda8552` · 2026-09-29 · style: always play animations, ignoring the OS reduced-motion setting
- `src/App.css`

### `b97d691` · 2026-09-29 · fix(settings): dark native dropdown lists for selects
- `src/App.css`

### `36cd574` · 2026-09-29 · docs: note home hero and brand mark in FILE_INDEX
- `FILE_INDEX.md`

### `6afcf30` · 2026-09-29 · fix(a11y): stop looping animations under reduced motion
- `src/App.css`

### `5478cab` · 2026-09-29 · style: glass settings drawer, page transitions and softer empty states
- `src/App.css`

### `1b1c3d3` · 2026-09-29 · feat(media): glass detail page with drifting key art and staggered reveals
- `src/App.css`
- `src/HomePage.tsx`
- `src/MediaPage.tsx`

### `0e82b20` · 2026-09-29 · fix(home): fade hero scrim into the page and share one page gutter
- `src/App.css`

### `445c826` · 2026-09-29 · feat(home): featured hero with ambient art wash and lifted cards
- `src/App.css`
- `src/HomePage.tsx`

### `d820508` · 2026-09-29 · style: frosted-glass app bar, search and buttons with brand mark
- `src/App.css`
- `src/App.tsx`
- `src/icons.tsx`

### `6c7400f` · 2026-09-29 · docs: document the hide-unlisted-sources setting
- `PLAN.md`

### `5acc88d` · 2026-09-29 · feat(settings): hide unlisted sources (on by default)
- `src/MediaPage.tsx`
- `src/SettingsPanel.tsx`
- `src/settings.ts`

### `2852e24` · 2026-09-29 · docs: document movie handling
- `PLAN.md`

### `d4484ac` · 2026-09-29 · feat: handle movies as one "Movie" group instead of Unknown/Batch rows
- `src/App.tsx`
- `src/HlsPlayerView.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/types.ts`

### `2dfc1d5` · 2026-09-29 · docs: document the Kitsu metadata fallback
- `FILE_INDEX.md`
- `PLAN.md`

### `f991579` · 2026-09-29 · feat: fall back to Kitsu for anime search/details when AniList is unavailable
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/metadata_fallback.rs`

### `32225ee` · 2026-09-29 · feat(kitsu-client): anime search and by-AniList-id lookup
- `crates/kitsu-client/examples/fallback_debug.rs`
- `crates/kitsu-client/src/anime.rs`
- `crates/kitsu-client/src/lib.rs`

### `609864c` · 2026-09-29 · docs: index router, fullscreen and keyboard modules
- `FILE_INDEX.md`

### `772737c` · 2026-09-29 · fix(player): pointer-driven seek bar commits once and never shows stale time
- `src/App.css`
- `src/PlayerView.tsx`

### `d22c325` · 2026-09-29 · fix(player): hotkeys stay live after clicking sliders or buttons
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`
- `src/fullscreen.ts`
- `src/keyboard.ts`

### `04a2a24` · 2026-09-29 · chore(player): debug-log seek commits and restarts
- `src/PlayerView.tsx`
- `src/mpvVideo.ts`

### `6194199` · 2026-09-29 · feat(player): loading overlay while seeks wait, with smoother animation
- `src/App.css`
- `src/Buffering.tsx`
- `src/PlayerView.tsx`
- `src/mpvVideo.ts`

### `a400193` · 2026-09-29 · feat: hash routing with back/forward for home, anime pages and the player
- `src/App.css`
- `src/App.tsx`
- `src/MediaPage.tsx`
- `src/router.ts`

### `6cd1bde` · 2026-09-29 · fix(player): keep the seek bar on the target until mpv restarts playback
- `src/mpvVideo.ts`

### `0e440f0` · 2026-09-29 · fix(fullscreen): adopt the window's real fullscreen state on load
- `src-tauri/capabilities/default.json`
- `src/fullscreen.ts`

### `4856d66` · 2026-09-29 · style(player): fluid buffering ring fill and breathing
- `src/App.css`
- `src/Buffering.tsx`

### `e7b4964` · 2026-09-29 · style(player): slower, desynced now-playing meter bars
- `src/App.css`

### `88a3dc8` · 2026-09-29 · feat: app-wide F fullscreen hotkey, shared window fullscreen state
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/fullscreen.ts`
- `src/main.tsx`

### `0153220` · 2026-09-29 · feat(player): clicking the video no longer toggles pause
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`

### `6dc5711` · 2026-09-29 · perf(build): optimize image decoding crates in dev builds
- `Cargo.toml`

### `03b5131` · 2026-09-29 · fix(player): mpv pause state, seek coalescing, instant close
- `src/PlayerView.tsx`
- `src/mpvVideo.ts`

### `63d3a65` · 2026-09-28 · docs: document embedded mpv playback and HLS fallback
- `CLAUDE.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `e621dae` · 2026-09-28 · docs(code): update comments for mpv playback
- `crates/mpv-ipc/src/lib.rs`
- `src-tauri/src/lib.rs`

### `5ca4ac9` · 2026-09-28 · fix(player): keep mpv when the availability check itself fails
- `src/PlayerView.tsx`

### `6d6e48a` · 2026-09-28 · feat(player): fall back to the HLS player when mpv isn't installed
- `src/App.css`
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`

### `bc9289d` · 2026-09-28 · feat(player): add mpv_available check
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `92413e2` · 2026-09-28 · chore(player): log mpv property polling at trace level
- `src-tauri/src/player.rs`

### `43a0e81` · 2026-09-28 · fix(player): round mpv sub-margin-y to an integer
- `src/subtitles.ts`

### `a6aef81` · 2026-09-28 · feat(player): play through embedded mpv instead of hls.js <video>
- `src-tauri/capabilities/default.json`
- `src/App.css`
- `src/PlayerView.tsx`
- `src/StatisticsMenu.tsx`
- `src/mpvVideo.ts`
- `src/subtitles.ts`
- `src/types.ts`

### `b82564b` · 2026-09-28 · feat(player): mpv frame grabs, clipboard copy, bundled sub fonts, raw stream URLs
- `crates/mpv-ipc/src/embedded.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `9b3721d` · 2026-09-28 · feat(player): embed mpv under a transparent webview via mpv_start/command/stop
- `crates/mpv-ipc/src/embedded.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `7aba654` · 2026-09-28 · feat(mpv-ipc): add EmbeddedMpv for in-window playback over async IPC
- `crates/mpv-ipc/src/embedded.rs`
- `crates/mpv-ipc/src/lib.rs`

### `9f0e360` · 2026-09-28 · perf(mpv-ipc): downscale headless captures, skip audio/sub decode
- `crates/mpv-ipc/src/lib.rs`

### `b0ca0b3` · 2026-09-28 · perf(thumbnails): serve by URL, save raw frames, serialize captures
- `src-tauri/src/lib.rs`
- `src/torrentThumbnail.ts`
- `src/PlayerView.tsx`
- `src/HomePage.tsx`
- `src/App.tsx`
- `PLAN.md`
- `FILE_INDEX.md`

### `b1dae32` · 2026-09-28 · fix(player): capture the last frame before hls.js tears down the video
- `src/PlayerView.tsx`

### `288896f` · 2026-09-28 · fix(player): seek tooltip tracks the cursor without re-rendering
- `src/PlayerView.tsx`
- `src/App.css`

### `0c735c0` · 2026-09-28 · feat(player): arrow-key skips no longer reveal the controls
- `src/PlayerView.tsx`

### `67be4db` · 2026-09-28 · perf(player): memoize episode list, drop per-open row cascade
- `src/PlayerPlaylist.tsx`
- `src/PlayerView.tsx`
- `src/MediaPage.tsx`
- `src/App.css`

### `dc49fb4` · 2026-09-28 · feat(player): fade controls on mouse idle unless over top/bottom bars
- `src/PlayerView.tsx`
- `src/App.css`
- `PLAN.md`
- `PHASES.md`

### `f2f8c33` · 2026-09-28 · feat(player): drop right-edge hover for the episode list
- `src/PlayerView.tsx`
- `src/PlayerPlaylist.tsx`
- `src/App.css`
- `FILE_INDEX.md`
- `PHASES.md`

### `eecd13f` · 2026-09-28 · feat(stats): verbose statistics popup
- `src/App.css`
- `src/PlayerView.tsx`
- `src/StatisticsMenu.tsx`
- `src/types.ts`

### `2a77529` · 2026-09-28 · feat(stats): report swarm, upload and HLS run details in StreamStats
- `crates/torrent-engine/src/lib.rs`

### `ca42e3e` · 2026-09-28 · feat(player): gradient control strip, reveal controls from the top edge
- `src/App.css`
- `src/PlayerView.tsx`

### `3c70de4` · 2026-09-28 · feat(thumbnails): paint card art on the first frame after launch
- `src/kitsu.ts`
- `src/App.tsx`

### `fe500c7` · 2026-09-28 · feat(thumbnails): save the player's last frame as the episode thumbnail
- `src-tauri/src/lib.rs`
- `src/torrentThumbnail.ts`
- `src/PlayerView.tsx`
- `src/HomePage.tsx`

### `3f3d8b1` · 2026-09-28 · feat(thumbnails): show cached frames as soon as cards are listed
- `src/App.tsx`
- `src/torrentThumbnail.ts`

### `9ec8b40` · 2026-09-28 · docs: thumbnail and Kitsu metadata caching
- `PLAN.md`
- `FILE_INDEX.md`

### `45421b8` · 2026-09-28 · feat(thumbnails): reuse cached thumbnails without refetching
- `src-tauri/src/lib.rs`
- `src/torrentThumbnail.ts`

### `ed55599` · 2026-09-28 · docs: nyaa.si response cache
- `PLAN.md`
- `FILE_INDEX.md`

### `48fddc2` · 2026-09-28 · feat(nyaa): disk cache for searches and view pages
- `crates/nyaa-client/src/cache.rs`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/Cargo.toml`
- `src-tauri/src/lib.rs`

### `742ef33` · 2026-09-28 · docs: subtitle event log, ignore_readorder, English-first, CR style
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `31a7d12` · 2026-09-28 · fix(subtitles): write ASS events as they arrive (ignore_readorder)
- `crates/torrent-engine/src/media.rs`

### `257b296` · 2026-09-28 · feat(subtitles): Crunchyroll style as the default for unstyled tracks
- `src/App.css`
- `src/SettingsPanel.tsx`
- `src/assRenderer.ts`
- `src/assets/fonts/GandhiSans-Bold.otf`
- `src/assets/fonts/GandhiSans-BoldItalic.otf`
- `src/settings.ts`
- `src/subtitles.ts`

### `cee8223` · 2026-09-28 · perf(subtitles): playback runs extract English tracks only
- `crates/torrent-engine/src/lib.rs`

### `b14380e` · 2026-09-28 · feat(subtitles): soft CSS blur instead of 2x supersampling
- `src/App.css`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/assRenderer.ts`
- `src/settings.ts`

### `ef8151e` · 2026-09-28 · fix(subtitles): load a track once, then append new events
- `src/assRenderer.ts`

### `0317486` · 2026-09-28 · feat(subtitles): append-only event log with delta polling
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/subtitle_log.rs`

### `efca10f` · 2026-09-28 · fix(enginefs): finish an in-flight file read before seeking
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/VENDORED.md`

### `2e16012` · 2026-09-28 · fix(hls): serve init.mp4 only once complete, reopen direct reads on seek
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/direct_input.rs`

### `135e92a` · 2026-09-28 · fix(hls): keep absolute decode times in fMP4 fragments
- `crates/torrent-engine/src/media.rs`

### `d4dbef0` · 2026-09-28 · docs: transcode throughput and GPU-resident pipeline decision
- `PLAN.md`

### `01bd282` · 2026-09-28 · test(hls): transcode throughput mode for the smoke test
- `crates/torrent-engine/src/media.rs`

### `9ed5c55` · 2026-09-28 · docs: direct torrent reads for HLS runs
- `PLAN.md`
- `FILE_INDEX.md`

### `9a2c5ee` · 2026-09-28 · feat(hls): read torrents directly in HLS and subtitle runs
- `crates/torrent-engine/src/direct_input.rs`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`

### `1ab76ba` · 2026-09-28 · docs: fMP4 HLS segments and aborted-run tail cleanup
- `PLAN.md`
- `FILE_INDEX.md`

### `2f8d6a1` · 2026-09-28 · feat(hls): serve fMP4 segments instead of MPEG-TS
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`
- `src/PlayerView.tsx`

### `519db39` · 2026-09-28 · fix(hls): drop the truncated segment an aborted run leaves behind
- `crates/torrent-engine/src/lib.rs`

### `a7530bc` · 2026-09-28 · fix(subtitles): stream-copy ASS tracks instead of decoding them
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`

### `313ff0a` · 2026-09-28 · docs: in-process FFmpeg, build prerequisites, subtitle lift/AA
- `CLAUDE.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `bd893da` · 2026-09-28 · feat(subtitles): lift only dock-covered lines, 2x supersampled rendering
- `src/App.css`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/assRenderer.ts`
- `src/settings.ts`
- `src/subtitles.ts`

### `628fc53` · 2026-09-28 · feat(player): lift subtitles above the control dock while it's shown
- `src/App.css`
- `src/PlayerView.tsx`

### `93e952d` · 2026-09-28 · fix(engine): don't cache a copy fallback when the probe is slow
- `crates/torrent-engine/src/lib.rs`

### `41c4843` · 2026-09-28 · feat(engine): run FFmpeg in-process (ez-ffmpeg/ffmpeg-next) instead of the CLI
- `.cargo/config.toml`
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/build.rs`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`
- `vcpkg.json`

### `84ecda2` · 2026-09-28 · fix(release): spawn ffmpeg/ffprobe/mpv with CREATE_NO_WINDOW
- `crates/mpv-ipc/src/lib.rs`
- `crates/torrent-engine/src/lib.rs`
- `vendor/enginefs/src/engine.rs`
- `vendor/enginefs/src/hls.rs`
- `vendor/enginefs/src/lib.rs`

### `5ae5ea9` · 2026-09-28 · fix(player): retry the native clipboard for ~1s when another app holds it
- `src-tauri/src/lib.rs`

### `17b7275` · 2026-09-28 · fix(enginefs): detect unflushed zeros per 16 KiB block, serve valid prefix
- `vendor/enginefs/VENDORED.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `4ef5c5b` · 2026-09-28 · fix(parser): recognize ordinal seasons ('2nd Season')
- `src/episodeParser.ts`

### `e81dd4a` · 2026-09-28 · fix(player): copy frames through the native clipboard (arboard)
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src/PlayerView.tsx`

### `16dd815` · 2026-09-28 · docs: transcoding, vendored enginefs, player polish; note ffmpeg-next for later
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `888a1bb` · 2026-09-28 · chore(bundle): name the app nyaa-stream instead of the template's tauri-app
- `src-tauri/tauri.conf.json`

### `77e2bb3` · 2026-09-28 · feat(player): Ctrl+C copies the current frame (with subtitles) as PNG
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`

### `314de1d` · 2026-09-28 · fix(enginefs): wait up to 8s for real bytes instead of accepting zeros
- `vendor/enginefs/VENDORED.md`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `4afa9c8` · 2026-09-28 · feat(player): 60fps seek bar/clock, animated chrome, ambient glass dock
- `src/App.css`
- `src/PlayerPlaylist.tsx`
- `src/PlayerView.tsx`

### `6e7aa79` · 2026-09-28 · fix(enginefs): bound zero-read retries to ~200ms per piece, log once
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `a2dc946` · 2026-09-28 · chore(mpv-ipc): log expected 'property unavailable' replies at debug
- `crates/mpv-ipc/src/lib.rs`

### `ff42d6e` · 2026-09-28 · fix(thumbnails): defer torrent thumbnail capture while a stream plays
- `src-tauri/src/lib.rs`
- `src/torrentThumbnail.ts`

### `b39741b` · 2026-09-28 · feat(player): report decoder support, show when converting to H.264
- `src/App.css`
- `src/PlayerView.tsx`
- `src/playback.ts`
- `src/releases.ts`
- `src/types.ts`

### `67e9797` · 2026-09-28 · feat(engine): transcode formats the WebView can't decode to H.264
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`

### `9f111c3` · 2026-09-28 · fix(enginefs): re-read all-zero disk chunks through read_piece
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `20492dc` · 2026-09-28 · build: vendor enginefs (upstream f585ab6, MIT) via [patch]
- `Cargo.toml`
- `vendor/enginefs/Cargo.toml`
- `vendor/enginefs/LICENSE`
- `vendor/enginefs/VENDORED.md`
- `vendor/enginefs/build.rs`
- `vendor/enginefs/src/backend/librqbit.rs`
- `vendor/enginefs/src/backend/libtorrent/alerts.rs`
- `vendor/enginefs/src/backend/libtorrent/constants.rs`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/src/backend/libtorrent/handle.rs`
- `vendor/enginefs/src/backend/libtorrent/helpers.rs`
- `vendor/enginefs/src/backend/libtorrent/mod.rs`
- `vendor/enginefs/src/backend/libtorrent/playback.rs`
- `vendor/enginefs/src/backend/libtorrent/stream.rs`
- `vendor/enginefs/src/backend/metadata.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/priorities.rs`
- `vendor/enginefs/src/cache.rs`
- `vendor/enginefs/src/disk_cache.rs`
- `vendor/enginefs/src/engine.rs`
- `vendor/enginefs/src/files.rs`
- `vendor/enginefs/src/hls.rs`
- `vendor/enginefs/src/hwaccel.rs`
- `vendor/enginefs/src/lib.rs`
- `vendor/enginefs/src/metadata_cache.rs`
- `vendor/enginefs/src/metadata_pins.rs`
- `vendor/enginefs/src/piece_cache.rs`
- `vendor/enginefs/src/piece_waiter.rs`
- `vendor/enginefs/src/subtitles.rs`
- `vendor/enginefs/src/tracker_prober.rs`
- `vendor/enginefs/src/trackers.rs`

### `2a9c864` · 2026-09-28 · docs: sync planning docs with live-debug fixes and player playlist
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `362ea6a` · 2026-09-28 · feat(player): side playlist, control UX, direct episode play, timing fixes
- `src/App.css`
- `src/MediaPage.tsx`
- `src/PlayerPlaylist.tsx`
- `src/PlayerView.tsx`
- `src/icons.tsx`

### `2dfcb94` · 2026-09-28 · fix(releases): rank codecs this WebView can't decode last
- `src/releases.ts`

### `90de84d` · 2026-09-28 · perf(subtitles): warm libass renderer at track selection, cap render height
- `src/assRenderer.ts`

### `4262654` · 2026-09-28 · fix(engine): restart dead transcodes, reconnect on early EOF, background subs pass
- `crates/torrent-engine/src/lib.rs`

### `fbab937` · 2026-09-28 · docs: sync PLAN/PHASES/FILE_INDEX with subtitle, streaming and UI rework
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `c2ffe15` · 2026-09-28 · fix(ui): clearer fetch errors, preview scaling, continue-watching tie-break
- `src/App.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/watchProgress.ts`

### `41de78a` · 2026-09-28 · style: dusk-indigo redesign with bundled Zen Kaku Gothic New
- `index.html`
- `package-lock.json`
- `package.json`
- `src/App.css`
- `src/main.tsx`

### `536346c` · 2026-09-28 · feat(ui): resume, next episode, batch file picker, settings panel
- `src/App.tsx`
- `src/HomePage.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/SettingsPanel.tsx`
- `src/icons.tsx`
- `src/playback.ts`
- `src/types.ts`

### `9b164a0` · 2026-09-28 · feat(library): watch-progress store and preferred fansub group ranking
- `src/releases.ts`
- `src/watchProgress.ts`

### `d29b0bc` · 2026-09-28 · feat(engine): expose torrent file list and per-file stats/subtitles
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`

### `6046daf` · 2026-09-28 · perf(engine): rate-based seek restarts, smaller probe windows, ready ranges
- `crates/torrent-engine/src/lib.rs`

### `664586c` · 2026-09-28 · feat(player): render subtitles with libass (JASSUB), styled ASS + fonts
- `package-lock.json`
- `package.json`
- `src/PlayerView.tsx`
- `src/assRenderer.ts`
- `src/playback.ts`
- `src/settings.ts`
- `src/subtitles.ts`
- `src/types.ts`
- `vite.config.ts`

### `fdcdf07` · 2026-09-28 · feat(engine): extract subtitles as ASS inside the HLS job, copyts timeline
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`

### `0b6be20` · 2026-09-04 · fix(player): add crossOrigin so <track> subtitles actually render
- `src/PlayerView.tsx`

### `d48b97e` · 2026-09-05 · docs: sync PLAN.md/FILE_INDEX.md with per-anime results caching
- `PLAN.md`, `FILE_INDEX.md`

### `5456ad3` · 2026-09-05 · feat(search): cache nyaa.si results and torrent-detail scrapes per anime
- `src/App.tsx`

### `4678e02` · 2026-09-05 · fix(player): debounce keyboard seeking to stop stutter
- `src/PlayerView.tsx`

### `50a6264` · 2026-09-05 · docs: sync PLAN.md/FILE_INDEX.md with subtitle support
- `PLAN.md`, `FILE_INDEX.md`

### `d6e816b` · 2026-09-05 · feat(player): add subtitle track picker
- `src/types.ts`, `src/playback.ts`, `src/PlayerView.tsx`

### `68832f5` · 2026-09-05 · feat(tauri): add get_subtitle_tracks command
- `src-tauri/src/lib.rs`

### `bb29f17` · 2026-09-05 · feat(torrent-engine): extract embedded subtitle tracks to WebVTT
- `crates/torrent-engine/Cargo.toml`, `crates/torrent-engine/src/lib.rs`

### `0384c5f` · 2026-09-05 · docs: sync PLAN.md with the episode-mapping fixes
- `PLAN.md`

### `c7abb07` · 2026-09-05 · fix(search): also search nyaa.si without a "Season N" suffix
- `src-tauri/src/lib.rs`

### `6f4c99d` · 2026-09-05 · fix(search): recognize absolute-numbered episodes with no season marker
- `src/App.tsx`

### `c1bfe7b` · 2026-09-05 · fix(player): close statistics box on outside click
- `src/PlayerView.tsx`

### `62a1bec` · 2026-09-05 · fix(ui): remove stray body-margin scrollbar
- `src/App.css`

### `835bac3` · 2026-09-05 · docs: sync PLAN.md/FILE_INDEX.md with the libtorrent backend swap
- `PLAN.md`, `FILE_INDEX.md`

### `6a177fe` · 2026-09-05 · fix(playback): propagate TorrentId's numeric -> info-hash string change
- `src-tauri/src/lib.rs`, `src/PlayerView.tsx`, `src/playback.ts`, `src/types.ts`

### `a9af84a` · 2026-09-05 · feat(torrent-engine): replace librqbit session with enginefs's libtorrent backend
- `crates/torrent-engine/Cargo.toml`, `crates/torrent-engine/src/lib.rs`

### `aa934d3` · 2026-09-05 · build: add vcpkg manifest + triplet for libtorrent-rasterbar
- `.gitignore`, `.cargo/config.toml`, `triplets/x64-windows-v3-static-md-release.cmake`, `vcpkg.json`

### `ab03493` · 2026-09-05 · fix(media-page): scroll episode panel independently of the page
- `src/App.css`

### `67be02e` · 2026-08-30 · fix(player): stop hls.js killing playback on a slow-but-recoverable segment
- `PHASES.md`, `PLAN.md`, `src/PlayerView.tsx`

### `508a137` · 2026-08-30 · feat(sources): fix absolute-numbered episodes via AniList's relations graph
- `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`, `crates/anilist-client/src/lib.rs`, `crates/anilist-client/examples/offset_debug.rs`, `crates/anilist-client/examples/relations_debug.rs`, `src-tauri/src/lib.rs`, `src/App.tsx`

### `41c41f7` · 2026-08-30 · feat(thumbnails): capture torrent thumbnails near the episode midpoint
- `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`, `crates/mpv-ipc/src/lib.rs`, `src-tauri/src/lib.rs`, `src/App.tsx`, `src/torrentThumbnail.ts`

### `409b92a` · 2026-08-30 · style(home): size Latest Episodes cards 16:9 instead of a poster's 2:3
- `PHASES.md`, `src/App.css`

### `2979792` · 2026-08-30 · fix(player): seek-bar highlight reflects HLS readiness, not raw download
- `PHASES.md`, `crates/torrent-engine/src/lib.rs`, `src/PlayerView.tsx`, `src/types.ts`

### `d2c70e5` · 2026-08-30 · fix(sources): drop cross-season contamination from grouped results
- `FILE_INDEX.md`, `PLAN.md`, `src/App.tsx`, `src/episodeParser.ts`

### `d7d45da` · 2026-08-30 · feat(player): hover-only solid control bar + PotPlayer/YouTube keybinds
- `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`, `src/App.css`, `src/PlayerView.tsx`

### `7da48e9` · 2026-08-30 · fix(parser): stop "Final Season" colliding with real season 1
- `PHASES.md`, `PLAN.md`, `src/episodeParser.ts`

### `8724436` · 2026-08-30 · fix(search): always search both English and romaji titles, not fallback
- `PHASES.md`, `PLAN.md`, `src-tauri/src/lib.rs`, `src/browserFallback.ts`

### `86e46b5` · 2026-08-30 · fix(search): raise nyaa.si search page cap from 5 to 20
- `crates/nyaa-client/src/lib.rs`

### `268fc86` · 2026-08-30 · fix(parser): fix three real episode-parsing mismatches found live
- `FILE_INDEX.md`, `PHASES.md`, `crates/nyaa-client/examples/search_debug.rs`, `src/episodeParser.ts`

### `54e6320` · 2026-08-30 · fix(streaming): stop HLS transcode restart livelock on far seeks
- `crates/torrent-engine/src/lib.rs`

### `4038a5f` · 2026-08-30 · docs: sync PLAN/PHASES/FILE_INDEX/CLAUDE with HLS transcode rearchitecture
- `CLAUDE.md`, `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`

### `92ecc8e` · 2026-08-30 · feat(streaming): real HLS playback via a continuous per-file transcode
- `crates/torrent-engine/Cargo.toml`, `crates/torrent-engine/src/lib.rs`, `package-lock.json`, `package.json`, `src-tauri/src/lib.rs`, `src/PlayerView.tsx`, `src/types.ts`

### `1062ef4` · 2026-08-30 · feat(playback): autoplay latest-episode clicks straight into the player
- `src/App.tsx`, `src/HomePage.tsx`, `src/MediaPage.tsx`

### `da6821f` · 2026-08-30 · feat(metadata): add AniList per-episode duration for HLS estimates
- `crates/anilist-client/src/lib.rs`, `src/browserFallback.ts`

### `cfacbe6` · 2026-08-29 · feat(playback): remux through ffmpeg for reliable browser playback + seeking
- `CLAUDE.md`, `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`, `crates/torrent-engine/Cargo.toml`, `crates/torrent-engine/src/lib.rs`, `src-tauri/src/lib.rs`, `src/App.css`, `src/PlayerView.tsx`

### `c979d2d` · 2026-08-29 · feat(player): Stremio-style buffering/stats, source picker, safer play()
- `crates/torrent-engine/src/lib.rs`, `src/App.css`, `src/Buffering.tsx`, `src/MediaPage.tsx`, `src/PlayerView.tsx`, `src/StatisticsMenu.tsx`, `src/loadingProgress.ts`, `src/releases.ts`, `src/types.ts`

### `0c9b8eb` · 2026-08-29 · feat(window): launch maximized by default
- `src-tauri/tauri.conf.json`

### `9768c2c` · 2026-08-29 · fix(kitsu): add missing camelCase serde rename to KitsuMetadata
- `crates/kitsu-client/src/lib.rs`

### `e9a85d5` · 2026-08-29 · feat(devtools): forward frontend console/errors to the backend log
- `src/devLogger.ts`, `src/main.tsx`

### `cdd5136` · 2026-08-29 · feat(playback): replace mpv embedding with an HTML5 video player
- `CLAUDE.md`, `FILE_INDEX.md`, `PHASES.md`, `PLAN.md`, `crates/mpv-ipc/src/lib.rs`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src/App.css`, `src/MediaPage.tsx`, `src/PlayerView.tsx`, `src/playback.ts`, `src/types.ts`

### `4635087` · 2026-08-29 · feat(playback): detect mpv exit and clean up torrent/player state
- `crates/mpv-ipc/src/lib.rs`, `src-tauri/src/lib.rs`, `PLAN.md`, `PHASES.md`, `FILE_INDEX.md`

### `7234c79` · 2026-08-29 · feat(playback): wire play button, stream stats overlay, and stop/cleanup
- `crates/torrent-engine/src/lib.rs`, `src-tauri/src/lib.rs`, `src/App.css`, `src/MediaPage.tsx`, `src/types.ts`, `src/playback.ts`, `PLAN.md`, `PHASES.md`, `FILE_INDEX.md`

### `c4818ad` · 2026-08-25 · docs: sync PLAN/PHASES/FILE_INDEX with home page, Kitsu, and thumbnail-capture work
- `PLAN.md`, `PHASES.md`, `FILE_INDEX.md`

### `fb0f16d` · 2026-08-25 · feat(frontend): home page, media detail page, and episode parsing
- `src/App.tsx`, `src/App.css`, `src/HomePage.tsx`, `src/MediaPage.tsx`, `src/episodeParser.ts`, `src/library.ts`, `src/types.ts`, `src/browserFallback.ts`, `src/kitsu.ts`, `src/torrentThumbnail.ts`

### `9471481` · 2026-08-25 · feat(src-tauri): wire kitsu metadata, thumbnail capture, and airing-episodes commands
- `Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`

### `4e703f4` · 2026-08-25 · feat(kitsu-client): add crate for Kitsu backdrop/episode-thumbnail metadata
- `crates/kitsu-client/Cargo.toml`, `crates/kitsu-client/src/lib.rs`

### `59b11c9` · 2026-08-25 · feat(torrent-engine): add torrent removal and structured logging
- `crates/torrent-engine/src/lib.rs`

### `5de1a53` · 2026-08-25 · feat(mpv-ipc): add headless spawn and screenshot capture
- `crates/mpv-ipc/src/lib.rs`

### `69b7e24` · 2026-08-25 · feat(anilist-client): add format/season fields and batched airingSchedules query
- `crates/anilist-client/src/lib.rs`

### `313872c` · 2026-08-25 · feat(nyaa-client): switch from RSS to paginated HTML scrape
- `crates/nyaa-client/Cargo.toml`, `crates/nyaa-client/src/lib.rs`

### `b936e43` · 2026-08-22 · Scaffold nyaa-stream: Tauri+Preact shell, torrent/nyaa/anilist/mpv crates
- `Cargo.toml`, `src-tauri/*`, `crates/*`, `src/*`, `PLAN.md`, `PHASES.md`, `FILE_INDEX.md`, `CLAUDE.md`
