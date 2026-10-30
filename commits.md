### `ee1fabf` · 2026-10-30 · feat(thumbnails): capture frames with FFmpeg
- `CLAUDE.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `crates/mpv-player/src/lib.rs`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`
- `docs/planning/V0_6_0.md`
- `src-tauri/src/lib.rs`

### `550d912` · 2026-10-30 · docs: generalize player references
- `PHASES.md`
- `commits.md`
- `docs/planning/ARCHITECTURE.md`
- `docs/planning/ROADMAP.md`

### `a37cb48` · 2026-10-30 · fix(dev): move Vite off Windows-reserved port
- `FILE_INDEX.md`
- `src-tauri/tauri.conf.json`
- `vite.config.ts`

### `8824c1b` · 2026-10-30 · perf(thumbnails): prefer smallest viable source and bound captures
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `docs/planning/V0_6_0.md`
- `src-tauri/src/lib.rs`
- `src/MediaPage.tsx`
- `src/changelog.ts`

### `1dfaf69` · 2026-10-30 · docs: record Win10Lite installer acceptance
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `docs/acceptance/v0.6.0/README.md`
- `docs/acceptance/v0.6.0/report-Msi.json`
- `docs/acceptance/v0.6.0/report-MsiToNsis.json`
- `docs/acceptance/v0.6.0/report-Nsis.json`
- `docs/planning/V0_6_0.md`
- `docs/prototypes/installer/FEASIBILITY.md`

### `6173aed` · 2026-10-30 · fix(installer): bundle MSVC runtime for clean Windows launch
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `README.md`
- `docs/planning/V0_6_0.md`
- `src-tauri/runtime/README.md`
- `src-tauri/runtime/vcruntime140.dll`
- `src-tauri/tauri.conf.json`
- `src/changelog.ts`

### `0876ad9` · 2026-10-30 · docs: clarify WebView2 runtime bootstrap
- `README.md`
### `c2a313c` · 2026-10-30 · docs: resolve updater lifecycle design audit
- `docs/planning/V0_6_0.md`
- `PHASES.md`
### `30e4aa6` · 2026-10-30 · style(installer): brand progress and setup action
- `src-tauri/installer/installer.nsi`
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `docs/prototypes/installer/FEASIBILITY.md`
- `src/changelog.ts`
### `4ca6869` · 2026-10-30 · docs: specify branded installer progress UI
- `docs/planning/V0_6_0.md`
### `0df8150` · 2026-10-30 · fix(installer): restore Start Menu choice on upgrades
- `src-tauri/installer/installer.nsi`
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `src/changelog.ts`
### `7a7c47d` · 2026-10-30 · test(magnet): preserve creation time on merge
- `crates/nyaa-client/src/store.rs`
### `cb329d5` · 2026-10-30 · docs: update nyaa test count
- `PHASES.md`
### `91e3acf` · 2026-10-30 · docs: record custom source persistence coverage
- `PHASES.md`
### `ef5d028` · 2026-10-30 · test(magnet): cover source migration and persistence
- `crates/nyaa-client/src/store.rs`
### `1ff9b07` · 2026-10-30 · docs: record signed v0.6.0 installer builds
- `PHASES.md`
- `docs/prototypes/installer/FEASIBILITY.md`
### `d38dc8b` · 2026-10-30 · fix(test): remove duplicate torrent detail fields
- `crates/nyaa-client/src/store.rs`
### `0a3b272` · 2026-10-30 · docs: record v0.6.0 updater defaults
- `PLAN.md`
- `PHASES.md`
- `docs/planning/V0_6_0.md`
### `0db99b8` · 2026-10-30 · feat(artwork): resolve missing TMDB IDs by IMDb
- `crates/artwork-client/src/lib.rs`
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `src/changelog.ts`
### `adb87f3` · 2026-10-30 · docs: plan IMDb artwork ID fallback
- `docs/planning/V0_6_0.md`
### `e63bb71` · 2026-10-30 · docs: align seekbar milestone scope
- `PHASES.md`
### `6aaee55` · 2026-10-30 · fix(updater): abort when playback teardown fails
- `src/updater.ts`
### `ed49be5` · 2026-10-30 · style: polish installer progress page
- `src-tauri/installer/installer.nsi`
### `77a7222` · 2026-10-30 · fix(installer): label setup action Install
- `src-tauri/installer/installer.nsi`
### `8c0d2d0` · 2026-10-30 · fix(hls): skip extras in pack playback
- `src/HlsPlayerView.tsx`
### `3eec94e` · 2026-10-30 · fix(installer): label setup action Install
- `src-tauri/installer/installer.nsi`
### `7dba978` · 2026-10-30 · fix(magnet): warn when pack numbering is unknown
- `src/AttachCustomSourceDialog.tsx`
### `0f205f3` · 2026-10-30 · fix(player): skip extras in default pack playback
- `src/PlayerView.tsx`
### `f2611f6` · 2026-10-30 · fix: merge duplicate custom source edits
- `crates/nyaa-client/src/store.rs`
### `ab0f0a4` · 2026-10-30 · docs: describe magnet attachment workflow
- `CHANGELOG.md`
- `src/changelog.ts`
### `c2ae1c2` · 2026-10-30 · fix(magnet): return parsed episode label
- `crates/nyaa-client/src/magnet.rs`
### `402f196` · 2026-10-30 · docs: mark magnet implementation complete
- `PHASES.md`
### `614384d` · 2026-10-30 · fix: infer custom packs from file metadata
- `src/AttachCustomSourceDialog.tsx`
### `0d0e649` · 2026-10-30 · docs: sync magnet attachment workflow
- `FILE_INDEX.md`
- `PHASES.md`
### `22aafdd` · 2026-10-30 · feat: attach magnets and map custom packs
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/AttachCustomSourceDialog.tsx`
- `src/HlsPlayerView.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/types.ts`
### `9b0d8ae` · 2026-10-30 · fix: exclude extras from pack inference
- `src/AttachCustomSourceDialog.tsx`

### `26fa230` · 2026-10-30 · feat: add custom source attach dialog
- `src/AttachCustomSourceDialog.tsx`
- `src/AttachCustomSourceDialog.css`

### `8aaa290` · 2026-10-30 · fix: preserve custom source identity on edits
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/magnet.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/magnet.rs`
### `8226ce7` · 2026-10-30 · fix(installer): use planned per-user install path
- `src-tauri/installer/installer.nsi`
### `b333ce0` · 2026-10-30 · docs: index player source editing
- `FILE_INDEX.md`
### `5fc05ca` · 2026-10-30 · feat: edit attached sources from player
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
### `5bb557d` · 2026-10-30 · docs: record signed installer build
- `PHASES.md`
- `PLAN.md`
- `docs/planning/V0_6_0.md`
- `docs/prototypes/installer/FEASIBILITY.md`
### `714c399` · 2026-10-30 · fix(installer): preserve shortcut choices across upgrades
- `docs/prototypes/installer/FEASIBILITY.md`
- `src-tauri/installer/installer.nsi`
### `1ebb7dc` · 2026-10-30 · docs: draft v0.6.0 release notes
- `CHANGELOG.md`
- `src/changelog.ts`
### `7eaa57a` · 2026-10-30 · docs: sync v0.6.0 implementation status
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `docs/planning/V0_6_0.md`
### `2771b18` · 2026-10-30 · feat(installer): add branded custom NSIS setup page
- `docs/prototypes/installer/FEASIBILITY.md`
- `scripts/make-nsis-art.ps1`
- `src-tauri/installer/installer.nsi`
- `src-tauri/installer/nsis-header.bmp`
- `src-tauri/installer/nsis-sidebar.bmp`
- `src-tauri/tauri.conf.json`
### `d695324` · 2026-10-30 · fix: preserve resume data before updater restart
- `src-tauri/src/lib.rs`
- `src/updater.ts`
### `8cf6f60` · 2026-10-30 · fix: handle custom sources in context menus
- `src/ContextMenu.css`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `docs/planning/V0_6_0.md`
### `da25111` · 2026-10-30 · feat: add arm-server artwork ID fallback
- `crates/artwork-client/src/lib.rs`
- `docs/planning/V0_6_0.md`
### `6f79f46` · 2026-10-30 · feat: connect artwork to media pages
- `src/App.css`
- `src/App.tsx`
- `src/MediaPage.tsx`
### `c91efcb` · 2026-10-30 · Merge branch `work/v0.6-thumbnails`
- `crates/torrent-engine/src/media.rs`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/thumbnail_build.rs`
- `src-tauri/src/lib.rs`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/torrentThumbnail.ts`
- `FILE_INDEX.md`
- `PHASES.md`
- `docs/planning/V0_6_0.md`

### `8ad8969` · 2026-10-30 · feat: add artwork provider settings and home fallbacks
- `crates/artwork-client/src/lib.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/src/artwork.rs`
- `src-tauri/src/lib.rs`
- `src/artwork.ts`
- `src/providerCredentials.ts`
- `src/SettingsPanel.tsx`
- `src/backup.ts`
- `src/App.tsx`
- `src/HomePage.tsx`
- `src/App.css`
- `FILE_INDEX.md`
- `PHASES.md`

### `c7b129f` · 2026-10-29 · Merge branch `work/v0.6-installer`
- `src/updater.ts`
- `src/App.tsx`
- `src/SettingsPanel.tsx`
- `src/App.css`
- `docs/prototypes/installer/FEASIBILITY.md`
- `docs/prototypes/installer/index.html`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `8198f77` · 2026-10-29 · Merge branch `work/v0.6-artwork`
- `crates/artwork-client/Cargo.toml`
- `crates/artwork-client/src/lib.rs`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `docs/planning/V0_6_0.md`
### `dd747e0` · 2026-10-30 · feat: add frame generation workflows
- `FILE_INDEX.md`
- `PHASES.md`
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/media.rs`
- `docs/planning/V0_6_0.md`
- `src-tauri/src/cache.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/resume.rs`
- `src-tauri/src/thumbnail_build.rs`
- `src/HlsPlayerView.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`

### `36f6998` · 2026-10-29 · feat: build missing episode thumbnails
- `FILE_INDEX.md`
- `PHASES.md`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/lib.rs`
- `src/App.css`
- `src/HlsPlayerView.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/torrentThumbnail.ts`

### `0513336` · 2026-10-29 · Merge branch `work/v0.6-magnets`
- `crates/nyaa-client/src/magnet.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/magnet.rs`
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/MediaPage.tsx`
- `src/types.ts`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `5bb46a6` · 2026-10-29 · feat: validate release-page thumbnail frames
- `Cargo.toml`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/thumbnail_build.rs`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `FILE_INDEX.md`
- `PHASES.md`

### `2e09e10` · 2026-10-29 · feat: collect release description image URLs
- `crates/nyaa-client/src/lib.rs`

### `48aadc5` · 2026-10-29 · feat: add custom magnet sources
- `crates/nyaa-client/src/magnet.rs`
- `crates/nyaa-client/src/store.rs`
- `crates/nyaa-client/src/lib.rs`
- `src-tauri/src/magnet.rs`
- `src/App.tsx`
- `src/MediaPage.tsx`
- `src/types.ts`
- `src/App.css`

### `0f655dc` · 2026-10-29 · feat: add quiet in-app update flow
- `src/updater.ts`
- `src/App.tsx`
- `src/SettingsPanel.tsx`
- `src/App.css`
- `docs/prototypes/installer/FEASIBILITY.md`
- `docs/prototypes/installer/index.html`
- `docs/planning/V0_6_0.md`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `a0c2613` · 2026-10-29 · docs: record artwork provider decisions
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `docs/planning/V0_6_0.md`

### `daf4a6e` · 2026-10-29 · feat: add artwork provider client foundation
- `crates/artwork-client/Cargo.toml`
- `crates/artwork-client/src/lib.rs`

### `389ace0` · 2026-10-29 · feat: add player surface context actions
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `FILE_INDEX.md`
- `PHASES.md`

### `6272d7a` · 2026-10-29 · feat: add episode and source context actions
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `src/contextMenuStore.ts`
- `FILE_INDEX.md`
- `PHASES.md`

### `accb738` · 2026-10-29 · feat: add anime context menu actions
- `src/App.tsx`
- `src/HomePage.tsx`
- `src/contextMenuStore.ts`
- `PHASES.md`

### `a99675f` · 2026-10-29 · feat: add shared context menu foundation
- `src/contextMenuStore.ts`
- `src/ContextMenu.tsx`
- `src/ContextMenu.css`
- `src/main.tsx`
- `src/router.ts`
- `docs/planning/V0_6_0.md`
- `FILE_INDEX.md`
- `PHASES.md`

### `52013dc` · 2026-10-29 · feat: add native clipboard support
- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/capabilities/default.json`
- `PHASES.md`

### `94c33e8` · 2026-10-29 · style: enlarge installer primary button
- `docs/prototypes/installer/index.html`

### `4620482` · 2026-10-29 · feat: simplify installer prototype
- `docs/prototypes/installer/index.html`
- `PHASES.md`

### `3fcb2d4` · 2026-10-29 · feat: prototype compact installer screens
- `docs/prototypes/installer/index.html`
- `FILE_INDEX.md`
- `PHASES.md`

### `ef55db8` · 2026-10-29 · refactor: remove experimental ML pipeline
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `crates/nyaa-client/examples/build_tosho.rs`
- `crates/nyaa-client/examples/search_json.rs`
- `crates/nyaa-client/src/stem.rs`
- `crates/nyaa-client/src/tosho_build.rs`
- `crates/release-parse/examples/ml_parse.rs`
- `crates/release-parse/upstream/animebatch/PLAN.md`
- `docs/planning/MATCHING.md`
- `docs/planning/ROADMAP.md`
- `ml/export_extra.py`
- `ml/export_rows.py`
- `ml/fetch_anilist.py`
- `ml/score_search.py`
- `ml/train.py`
- `src-tauri/src/lib.rs`
- `src/DebugPage.css`
- `src/DebugPage.tsx`
- `src/changelog.ts`

### `7e31b59` · 2026-10-29 · docs: require v0.6.0 user data preservation
- `docs/planning/V0_6_0.md`
- `PHASES.md`

### `1482850` · 2026-10-29 · docs: revise milestone scope
- `PHASES.md`
- `PLAN.md`
- `docs/planning/MATCHING.md`
- `docs/planning/RELEASING.md`
- `docs/planning/ROADMAP.md`
- `docs/planning/V0_6_0.md`

### `69a355b` · 2026-10-29 · chore: consolidate subtitle work into v0.6.0
- `CHANGELOG.md`
- `PHASES.md`
- `PLAN.md`
- `docs/planning/RELEASING.md`
- `docs/planning/V0_6_0.md`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src/changelog.ts`

### `975469c` · 2026-10-29 · docs: organize project planning
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `README.md`
- `docs/planning/ARCHITECTURE.md`
- `docs/planning/MATCHING.md`
- `docs/planning/PLAYBACK.md`
- `docs/planning/RELEASING.md`
- `docs/planning/ROADMAP.md`
- `docs/planning/V0_6_0.md`

### `cb824ec` · 2026-10-29 · docs: plan v0.6.0 update experience
- `PLAN.md`
- `PHASES.md`

### `a3d8bd8` · 2026-10-29 · docs: plan v0.7.0 playback transitions
- `PLAN.md`
- `PHASES.md`

### `2df5a71` · 2026-10-29 · docs: defer native subtitle supersampling
- `PLAN.md`
- `PHASES.md`

### `ddbe3ab` · 2026-10-29 · docs: require supersampling in the native player
- `PLAN.md`
- `PHASES.md`

### `c167888` · 2026-10-29 · chore: release v0.5.10
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `PHASES.md`

### `c5f4661` · 2026-10-29 · fix: smooth subtitle text edges
- `src/subtitles.ts`
- `src/assRenderer.ts`
- `src/changelog.ts`
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`

### `9c81c74` · 2026-10-29 · docs: plan subtitle edge smoothing
- `PLAN.md`
- `PHASES.md`

### `715b4fa` · 2026-10-29 · chore: release v0.5.9
- `PHASES.md`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `d749cea` · 2026-10-29 · fix: update subtitle blending and video scaler
- `crates/mpv-player/src/embedded.rs`
- `src/changelog.ts`
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`

### `f24d149` · 2026-10-29 · docs: specify v0.5.9 subtitle blending
- `PLAN.md`
- `PHASES.md`

### `dd77fba` · 2026-10-29 · feat: add selective ASS dialogue styling
- `src/settings.ts`
- `src/subtitles.ts`
- `src/SettingsPanel.tsx`
- `src/changelog.ts`
- `CHANGELOG.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `4171719` · 2026-10-29 · docs: plan v0.5.9 selective subtitle styling
- `PLAN.md`
- `PHASES.md`

### `d73086f` · 2026-10-29 · docs: record v0.5.8 publication
- `PLAN.md`
- `PHASES.md`
- `CHANGELOG.md`

### `7f5b618` · 2026-10-29 · docs: prioritize opt-in memory profiling
- `PLAN.md`
- `PHASES.md`

### `25bf4ae` · 2026-10-29 · docs: plan v0.7.0 memory and branding work
- `PLAN.md`
- `PHASES.md`

### `108a51b` · 2026-10-29 · docs: consolidate v0.6.0 and resequence milestones
- `PLAN.md`
- `PHASES.md`

### `c24b250` · 2026-10-29 · docs: plan custom mpv build for v0.10.0
- `PLAN.md`
- `PHASES.md`

### `37be1f3` · 2026-10-29 · chore: finalize v0.5.8 subtitle blur
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `crates/mpv-player/src/embedded.rs`
- `src/changelog.ts`

### `70ba18e` · 2026-10-29 · chore: finalize v0.5.8 subtitle settings
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `crates/mpv-player/src/embedded.rs`
- `src/changelog.ts`

### `aa84766` · 2026-10-29 · fix: use native libass AA for subtitles
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `src/assRenderer.ts`
- `src/changelog.ts`

### `8980bbe` · 2026-10-29 · chore: finalize v0.5.8 subtitle AA
- `CHANGELOG.md`
- `PHASES.md`
- `PLAN.md`
- `src/changelog.ts`

### `2347cf5` · 2026-10-29 · fix: supersample HLS subtitle edges
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `src/App.css`
- `src/assRenderer.ts`
- `src/changelog.ts`

### `3689e99` · 2026-10-29 · docs: plan subtitle AA improvement
- `PLAN.md`
- `PHASES.md`

### `41a95d3` · 2026-10-29 · chore: release v0.5.8
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `src/PlayerView.tsx`
- `src/changelog.ts`

### `1798c5b` · 2026-10-29 · docs: record version branch convention
- `PLAN.md`

### `4dfdfcf` · 2026-10-29 · docs: record v0.5.8 release prep
- `PHASES.md`

### `272037f` · 2026-10-29 · chore: release v0.5.8
- `CHANGELOG.md`
- `PHASES.md`
- `PLAN.md`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src/changelog.ts`

### `64b3aae` · 2026-10-29 · fix: add audio decoder playback context
- `crates/mpv-player/src/libmpv.rs`
- `src/mpvVideo.ts`

### `612ce7b` · 2026-10-29 · fix: measure seek traces at playback restart
- `src-tauri/src/load_trace.rs`
- `src/PlayerView.tsx`

### `b72547a` · 2026-10-29 · fix: split oversized nyaa search expressions
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`
- `crates/nyaa-client/src/lib.rs`

### `bed12a6` · 2026-10-29 · docs: plan v0.5.8 playback and search fixes
- `PLAN.md`
- `PHASES.md`

### `0f9ddc0` · 2026-10-28 · chore: keep debugger work unversioned
- `PLAN.md`
- `PHASES.md`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `0e590d0` · 2026-10-28 · chore: release v0.5.8 (reversed by `0f9ddc0`; tag removed)
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `aa830c7` · 2026-10-28 · feat: show experimental ML source matches in debugger
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `ml/score_search.py`
- `src-tauri/src/lib.rs`
- `src/DebugPage.tsx`
- `src/DebugPage.css`

### `5c6e6ff` · 2026-10-27 · feat: show live source mapping analytics
- `src/DebugPage.tsx`
- `src/DebugPage.css`
- `src/App.tsx`
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match.rs`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `338e495` · 2026-10-27 · chore: release v0.5.7
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `b66d8cb` · 2026-10-27 · feat: target nyaa searches to anime entry
- `CHANGELOG.md`
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match.rs`
- `src-tauri/src/title_match_fixture_tests.rs`
- `src/App.tsx`
- `src/browserFallback.ts`
- `src/changelog.ts`
- `src/types.ts`

### `6e32312` · 2026-10-26 · feat: add debugger scrolling and matching diagram
- `src/DebugPage.tsx`
- `src/DebugPage.css`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `0b7b92b` · 2026-10-26 · feat: include backend logs in debug trace
- `src-tauri/src/lib.rs`
- `src/DebugPage.tsx`
- `PLAN.md`
- `FILE_INDEX.md`

### `fc4fd03` · 2026-10-26 · feat: add search flow debug page
- `src/App.tsx`
- `src/App.css`
- `src/DebugPage.tsx`
- `src/DebugPage.css`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `dabc301` · 2026-10-26 · chore: release v0.5.6
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `d915507` · 2026-10-26 · fix: improve batch torrent episode mapping
- `crates/nyaa-client/src/batch_check.rs`
- `crates/nyaa-client/src/file_name.rs`
- `PLAN.md`
- `PHASES.md`
- `CHANGELOG.md`
- `src/changelog.ts`

### `d35ee39` · 2026-10-26 · docs: plan torrent file mapping follow-up
- `PLAN.md`
- `PHASES.md`

### `e99cc43` · 2026-10-26 · docs: consolidate recent fixes in v0.5.5
- `CHANGELOG.md`
- `PHASES.md`
- `src/changelog.ts`

### `7ab0740` · 2026-10-26 · fix: show player controls only on hover
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `CHANGELOG.md`
- `src/changelog.ts`
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `ddd3eb9` · 2026-10-26 · fix: stabilize player hover at top edge
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `CHANGELOG.md`
- `src/changelog.ts`
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `7d9d05a` · 2026-10-26 · fix: write dev logs to launch directory
- `src-tauri/src/lib.rs`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `CHANGELOG.md`
- `src/changelog.ts`
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `7979c30` · 2026-10-26 · fix: remove titlebar gap in fullscreen
- `src/App.css`
- `src/fullscreen.ts`
- `PLAN.md`
- `PHASES.md`
- `CHANGELOG.md`
- `src/changelog.ts`
- `FILE_INDEX.md`
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `56972e8` · 2026-10-26 · fix: slide home header out in fullscreen
- `src/fullscreen.ts`
- `src/App.css`
- `PLAN.md`
- `PHASES.md`
- `CHANGELOG.md`
- `src/changelog.ts`
- `FILE_INDEX.md`
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### `c829310` · 2026-10-17 · feat: sort nyaa searches by seeders (s=seeders&o=desc), key caches by sort, drop the newest-first early stop
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `PLAN.md`
- `PHASES.md`

### `796f1c2` · 2026-10-16 · feat: score a live nyaa search against a franchise with the L1 model (ml/score_search.py)
- `ml/score_search.py`
- `crates/nyaa-client/examples/search_json.rs`
- `FILE_INDEX.md`

### `af2f779` · 2026-10-16 · feat: harden the L1 matcher prototype (no-match cases, slices, JJK and Mushoku runs)
- `ml/train.py`
- `ml/export_extra.py`
- `ml/fetch_anilist.py`
- `PLAN.md`

### `d554297` · 2026-10-16 · docs: record L1 XGBoost prototype result
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `d1b8014` · 2026-10-16 · feat: L1 prototype of the XGBoost release-to-entry matcher (ml/, scratch)
- `ml/export_rows.py`
- `ml/fetch_anilist.py`
- `ml/train.py`
- `crates/release-parse/examples/ml_parse.rs`

### `798db24` · 2026-10-15 · docs: record that the AnimeTosho live feed cannot fill the post-dump gap
- `PLAN.md`

### `d4eb8cc` · 2026-10-15 · docs: record L1c peer consistency results (not enabled)
- `PLAN.md`
- `PHASES.md`

### `1f72c94` · 2026-10-15 · test: peer-season harnesses over tosho and the Mushoku crawl (ignored, env-driven)
- `src-tauri/src/tosho_verify_tests.rs`
- `src-tauri/src/mushoku_s3_tests.rs`

### `342e097` · 2026-10-15 · feat: peer-consistent season for unmarked releases (measured, not enabled)
- `crates/nyaa-client/src/peer_season.rs`
- `crates/nyaa-client/src/lib.rs`
- `FILE_INDEX.md`

### `8e2e186` · 2026-10-15 · docs: plan XGBoost for the learned release matcher (Python train, Rust score)
- `PLAN.md`
- `PHASES.md`

### `68cd908` · 2026-10-15 · docs: record L1e batch self-check results
- `PLAN.md`
- `PHASES.md`

### `14dd77e` · 2026-10-15 · test: batch self-check over tosho releases with file lists (ignored, env-driven)
- `src-tauri/src/tosho_verify_tests.rs`

### `110d0de` · 2026-10-15 · feat: batch self-check of a release's file list against its title range
- `crates/nyaa-client/src/batch_check.rs`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/files.rs`
- `FILE_INDEX.md`

### `195e512` · 2026-10-15 · docs: record L1f live RSS mapping check
- `PLAN.md`
- `PHASES.md`

### `3bd2153` · 2026-10-15 · docs: record L1d file-name coverage results
- `PLAN.md`
- `PHASES.md`

### `a823829` · 2026-10-15 · fix: read episode numbers and extras in torrent file names
- `crates/nyaa-client/src/file_name.rs`
- `crates/nyaa-client/src/label.rs`
- `crates/nyaa-client/src/files.rs`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/Cargo.toml`
- `FILE_INDEX.md`

### `c7964e5` · 2026-10-15 · test: file coverage harness also writes duplicate-numbered files (DUPS_OUT)
- `src-tauri/src/tosho_verify_tests.rs`

### `c47c3e6` · 2026-10-15 · docs: plan L1d-L1f file-name coverage, batch self-check and RSS mapping check
- `PLAN.md`
- `PHASES.md`

### `a81db51` · 2026-10-15 · test: file-level labeller coverage over tosho torrents with file lists (ignored, env-driven)
- `src-tauri/src/tosho_verify_tests.rs`

### `bd73ac7` · 2026-10-15 · docs: v0.6.1 baseline, gold set and peer-consistency feature from the Mushoku crawl
- `PLAN.md`
- `PHASES.md`

### `72e0177` · 2026-10-15 · test: labeller coverage over trusted tosho releases (ignored, env-driven)
- `src-tauri/src/tosho_verify_tests.rs`

### `fe5251a` · 2026-10-15 · test: title-only crawl pass for the Mushoku S3 entry (ignored, env-driven)
- `src-tauri/src/mushoku_s3_tests.rs`

### `6b8b34f` · 2026-10-15 · docs: plan v0.6.1 learned release-to-entry matcher (trusted rows only)
- `PLAN.md`
- `PHASES.md`

### `d15dba0` · 2026-10-13 · docs: tick v0.5.0 in PHASES.md, note the open live check of the full Tosho import
- `PHASES.md`

### `f3b5a19` · 2026-10-13 · chore: release v0.5.0
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `56bd953` · 2026-10-13 · docs: changelog for v0.5.0
- `CHANGELOG.md`
- `src/changelog.ts`

### `bbb7a07` · 2026-10-13 · docs: Mushoku S3 report, season_ambiguous rule (v0.5.0)
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `7e7bc72` · 2026-10-13 · test: ignored offline harness for the Mushoku S3 page (v0.5.0)
- `src-tauri/src/mushoku_s3_tests.rs`

### `b7c734e` · 2026-10-13 · fix: roman-numeral season labels and season_ambiguous drops stray prequel releases on later-season entries (v0.5.0)
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`
- `crates/nyaa-client/src/lib.rs`
- `src/App.tsx`
- `src/types.ts`

### `40f732e` · 2026-10-13 · docs: custom magnet paste is planned for v0.6.0
- `PLAN.md`
- `PHASES.md`

### `6f56cef` · 2026-10-13 · docs: custom magnet paste on hold, not in v0.5.0
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`

### `63cbe42` · 2026-10-13 · docs: D2 spike result - sevenz-rust2 in-process 7z reader, no 7zr.exe bundle (v0.4.4)
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `3627847` · 2026-10-13 · feat(nyaa-client): build_tosho reads the 7z in-process, --external-7z keeps 7z e -so (v0.4.4 D2)
- `crates/nyaa-client/examples/build_tosho.rs`

### `fda3c65` · 2026-10-13 · feat(nyaa-client): in-process streaming 7z entry reader (sevenz-rust2) (v0.4.4 D2)
- `crates/nyaa-client/src/sevenz_stream.rs`
- `crates/nyaa-client/Cargo.toml`
- `crates/nyaa-client/src/lib.rs`

### `5558194` · 2026-10-13 · feat(tosho): import pipeline backend (download, build, import, cleanup) with progress event and cancel (D3)
- `src-tauri/src/tosho_import.rs`
- `crates/nyaa-client/src/tosho_pipeline.rs`
- `src-tauri/src/lib.rs`

### `a3aa4ee` · 2026-10-13 · feat(settings): Import Tosho Data button with confirm dialog and live progress (D4)
- `src/SettingsPanel.tsx`
- `src/App.css`

### `62c34b4` · 2026-10-13 · feat(sbtl-engine): fetch_one_file example, D1 spike of one-file download from the 1 TB at-data torrent
- `crates/sbtl-engine/examples/fetch_one_file.rs`

### `5ad75a3` · 2026-10-13 · feat(ui): Settings opens as a centered modal instead of a right drawer
- `src/App.css`

### `3f58c13` · 2026-10-13 · docs: this release is v0.5.0 (merges 0.4.3-0.4.5); old v0.5.0 context menus merge into v0.6.0
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`
- `CHANGELOG.md`
- `src/changelog.ts`

### `68f8cad` · 2026-10-13 · docs(plan): Import Tosho Data button - download, compile and import in one click (v0.4.4 T0)
- `PLAN.md`
- `PHASES.md`

### `907530d` · 2026-10-13 · docs: T8 verification results, aid_trusted rule, harness in FILE_INDEX (v0.4.4)
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `e5468f6` · 2026-10-13 · feat: trusted-aid releases bypass the frontend season filters (v0.4.4 T8)
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/types.ts`
- `crates/nyaa-client/src/lib.rs`

### `b6d3e01` · 2026-10-13 · test(tosho): ignored T8 verification harness over a built dataset (v0.4.4 T8)
- `src-tauri/src/tosho_verify_tests.rs`
- `src-tauri/Cargo.toml`

### `c48cf5c` · 2026-10-13 · fix(nyaa-client): search_local treats category 1_0 as all anime subcategories (v0.4.4 T8)
- `crates/nyaa-client/src/store.rs`

### `202dc96` · 2026-10-13 · docs: stem mapper holdout result, hook not enabled (v0.4.4 T9)
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `a74740a` · 2026-10-13 · test(title_match): nyaa-client's stem normalisation agrees with normalize (v0.4.4 T9)
- `src-tauri/src/title_match.rs`

### `a8941d8` · 2026-10-13 · feat(nyaa-client): import stem_aid with the dataset; aid_for_stem/aids_for_stems (v0.4.4 T9)
- `crates/nyaa-client/src/store.rs`
- `crates/nyaa-client/src/lib.rs`

### `ace0aab` · 2026-10-13 · feat(nyaa-client): holdout reports season carry-over among wrong stems (v0.4.4 T9)
- `crates/nyaa-client/src/stem.rs`
- `crates/nyaa-client/examples/build_tosho.rs`

### `9450c59` · 2026-10-13 · feat(nyaa-client): static stem->aid mapper trained on the dump, holdout eval (v0.4.4 T9)
- `crates/nyaa-client/src/stem.rs`
- `crates/nyaa-client/examples/build_tosho.rs`
- `crates/nyaa-client/src/tosho_build.rs`

### `6325af3` · 2026-10-13 · feat(matching): a known AniDB id decides which entry a release belongs to (v0.4.4 T5)
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/releases.ts`
- `src/types.ts`
- `PLAN.md`

### `790ace9` · 2026-10-13 · feat: entry season and episode offset from the AnimeTosho AniDB-TVDB map (v0.4.4 T7)
- `src-tauri/src/anime_lists.rs`
- `src-tauri/src/lib.rs`
- `src/App.tsx`

### `e097f2a` · 2026-10-13 · feat(nyaa-client): torrent details fall back to the dataset file list (v0.4.4 T6)
- `crates/nyaa-client/src/lib.rs`

### `8d51a89` · 2026-10-13 · feat(nyaa-client): import the AnimeTosho dataset into the store (v0.4.4 T3)
- `crates/nyaa-client/src/store.rs`
- `crates/nyaa-client/src/lib.rs`
- `src-tauri/src/lib.rs`
- `src/SettingsPanel.tsx`

### `92a74e0` · 2026-10-13 · feat(nyaa-client): AnimeTosho dataset builder (v0.4.4 T2)
- `crates/nyaa-client/src/tosho_build.rs`
- `crates/nyaa-client/examples/build_tosho.rs`
- `crates/nyaa-client/src/store.rs`

### `f2a3381` · 2026-10-13 · feat(anime-lists): AniList <-> AniDB id sets from Fribb, cache format v2 (v0.4.4 T4)
- `src-tauri/src/anime_lists.rs`

### `3b42841` · 2026-10-13 · docs(plan): static stem mapper trained on the AnimeTosho dump (v0.4.4 T9)
- `PLAN.md`
- `PHASES.md`

### `d902a01` · 2026-10-13 · docs(plan): custom magnet handles batches and packs (v0.4.5)
- `PLAN.md`
- `PHASES.md`

### `fd42596` · 2026-10-13 · docs(plan): custom magnet in the search bar (v0.4.5)
- `PLAN.md`
- `PHASES.md`

### `a56cb01` · 2026-10-13 · feat(nyaa-client): build_tosho --stats measures the AnimeTosho dump (v0.4.4 T1)
- `crates/nyaa-client/examples/build_tosho.rs`
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`

### `252d28f` · 2026-10-13 · feat(nyaa-client): streaming reader for the AnimeTosho mysqldump (INSERT parser)
- `crates/nyaa-client/src/tosho.rs`
- `crates/nyaa-client/src/lib.rs`

### `003ef7e` · 2026-10-13 · docs(plan): AnimeTosho T0 runs last, dev uses the local dump
- `PHASES.md`

### `3f6d07c` · 2026-10-13 · docs(plan): AnimeTosho T0 downloads only database-dump.7z from the 1 TB torrent
- `PLAN.md`
- `PHASES.md`

### `10688d2` · 2026-10-13 · docs(plan): in-app AnimeTosho dump download (v0.4.4 T0)
- `PLAN.md`
- `PHASES.md`

### `b056c24` · 2026-10-12 · fix: releases with no season marker are kept on later-season entries (only explicit other-season markers are dropped)
- `src/App.tsx`

### `88870e9` · 2026-10-08 · fix: an unmarked release is offered per episode only once its file list shows the episode's season
- `src/App.tsx`

### `be4c053` · 2026-10-08 · feat(title_match): primary season falls back to the prequel chain (split cours count once)
- `src-tauri/src/title_match.rs`

### `463dc98` · 2026-10-08 · fix(anime_lists): 10 min download limit (90 s timed out under a saturated connection), log the real error
- `src-tauri/src/anime_lists.rs`

### `a8f3a9d` · 2026-10-08 · fix: a pack's season-less files cover only its named season (else season 1), not every season
- `src/App.tsx`

### `89189d9` · 2026-10-08 · docs(phases): tick wrong-file fix
- `PHASES.md`

### `e16017f` · 2026-10-08 · fix(player): pick the episode's file from the entry's season and skip OVA/NCOP-style files
- `src/App.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`

### `5da2f1e` · 2026-10-08 · feat: get_entry_season - the matcher's primary season (TVDB, titles, first-season flag)
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`

### `219d5aa` · 2026-10-08 · feat: season and complete-series packs without an episode range serve each episode of their seasons
- `src/App.tsx`

### `69d2363` · 2026-10-08 · fix(title_match): complete-series packs and multi-season packs on entries of unknown season
- `src-tauri/src/title_match.rs`

### `89aa7ce` · 2026-10-08 · fix: multi-season and complete-series packs show on every season they cover
- `src/App.tsx`

### `33f0530` · 2026-10-08 · docs: v0.4.3 fixture check results, index title_match_fixture_tests
- `FILE_INDEX.md`
- `PHASES.md`
- `PLAN.md`

### `0192da7` · 2026-10-08 · fix(title_match): fixture-found misses - same-season siblings, shared franchise short names, extras, roman numerals
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match.rs`
- `src-tauri/src/title_match_fixture_tests.rs`

### `983b821` · 2026-10-08 · test(title_match): fixture-driven recall/precision per franchise entry
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match_fixture_tests.rs`

### `2e7b1fd` · 2026-10-08 · docs: changelog entries for v0.4.3
- `CHANGELOG.md`
- `src/changelog.ts`

### `0a17f0b` · 2026-10-08 · docs: tick v0.4.3 steps 6-8, index files.rs and of_path
- `FILE_INDEX.md`
- `PHASES.md`

### `e531eec` · 2026-10-08 · feat: episode-less matched releases are season batches; file-list seasons resolve unsure ones
- `src/App.tsx`

### `82362c5` · 2026-10-08 · feat(player): among a pack's files with the same episode number, prefer the episode group's season
- `src/HlsPlayerView.tsx`
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`

### `5aa0789` · 2026-10-08 · feat: play files take their season from a parent folder (EpisodeLabel::of_path)
- `crates/nyaa-client/src/label.rs`
- `src-tauri/src/lib.rs`

### `3e97f4f` · 2026-10-08 · feat(title_match): strip synonyms and roman numerals in queries, base-name query for season 2+
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match.rs`

### `fadda7d` · 2026-10-08 · feat: TorrentDetails type carries files and seasons
- `src/types.ts`

### `2daf3b0` · 2026-10-08 · feat(nyaa-client): keep scraped view pages permanently in the release database
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`

### `4325881` · 2026-10-08 · feat(nyaa-client): TorrentDetails carries the scraped file list and seasons
- `crates/nyaa-client/src/files.rs`
- `crates/nyaa-client/src/lib.rs`

### `685896d` · 2026-10-08 · feat(nyaa-client): parse a torrent's file list into per-file season/episode
- `crates/nyaa-client/src/files.rs`
- `crates/nyaa-client/src/lib.rs`

### `b2c6421` · 2026-10-08 · docs: Binge Mode watch order decision in PLAN.md
- `PLAN.md`

### `319b37a` · 2026-10-08 · docs: Binge Mode header button and setup modal in PLAN.md
- `PLAN.md`

### `b6c32ab` · 2026-10-08 · docs: Binge Mode OP/ED rules and decisions in PLAN.md
- `PLAN.md`

### `4be1330` · 2026-10-08 · docs: make Binge Mode fully customizable in PLAN.md
- `PLAN.md`

### `01df104` · 2026-10-08 · docs: franchise-aware matching in PLAN, FILE_INDEX, PHASES
- `PLAN.md`
- `FILE_INDEX.md`
- `PHASES.md`

### `0094b79` · 2026-10-08 · feat: franchise-aware matching - first-season flag, sibling routing, movies/specials off TV entries
- `src-tauri/src/title_match.rs`
- `src-tauri/src/franchise_cache.rs`
- `src-tauri/src/lib.rs`

### `48a841e` · 2026-10-08 · feat(anilist-client): franchise() - prequel/sequel chains, side stories, first-season flag
- `crates/anilist-client/src/lib.rs`
- `crates/anilist-client/examples/franchise_debug.rs`

### `4caabf5` · 2026-10-08 · docs: add Binge Mode idea to PLAN.md (v1.1.0)
- `PLAN.md`

### `5e8190f` · 2026-10-08 · docs: sync FILE_INDEX and PHASES for anime_lists
- `FILE_INDEX.md`
- `PHASES.md`

### `5605d3d` · 2026-10-08 · feat: TVDB season numbers from Fribb/anime-lists join the entry season set
- `src-tauri/src/anime_lists.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/title_match.rs`
- `src/App.tsx`
- `src/torrentThumbnail.ts`

### `e74ed39` · 2026-10-08 · docs: sync FILE_INDEX and PHASES for ShowMatcher
- `FILE_INDEX.md`
- `PHASES.md`

### `6ec553d` · 2026-10-08 · feat(title_match): reject other-season releases, accept short name with season marker
- `src-tauri/src/title_match.rs`
- `src-tauri/src/lib.rs`

### `9f211f9` · 2026-10-08 · docs(phases): tick release/entry seasons
- `PHASES.md`

### `4beb539` · 2026-10-08 · feat(title_match): release seasons and entry seasons from release-parse
- `src-tauri/src/title_match.rs`
- `src-tauri/Cargo.toml`

### `e2e7332` · 2026-10-08 · docs(phases): tick apostrophe normalization
- `PHASES.md`

### `465a84c` · 2026-10-08 · fix(title_match): remove apostrophes instead of splitting on them
- `src-tauri/src/title_match.rs`

### `e1b8421` · 2026-10-08 · docs(phases): tick CHANGELOG/README tasks
- `PHASES.md`

### `9fed59a` · 2026-10-08 · docs(readme): refresh for v0.4.x and link CHANGELOG.md
- `README.md`

### `b96f966` · 2026-10-08 · docs: add CHANGELOG.md
- `CHANGELOG.md`

### `395780b` · 2026-10-08 · docs(plan): CHANGELOG.md and README refresh
- `PHASES.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `79b3419` · 2026-10-08 · docs: plan the line-ending fix (PLAN, PHASES)
- `PHASES.md`
- `PLAN.md`

### `0ab8b9b` · 2026-10-08 · docs(plan): record c=1_0 main nyaa filter
- `PLAN.md`

### `ed03291` · 2026-10-08 · feat(search): default nyaa filter to all anime (c=1_0)
- `src-tauri/src/lib.rs`
- `src/browserFallback.ts`

### `b3847e5` · 2026-10-08 · build(release): content-based clean-tree check
- `scripts/release.mjs`

### `308157f` · 2026-10-08 · build: .gitattributes pins LF line endings
- `.gitattributes`

### `44a1381` · 2026-10-08 · docs: v0.4.2 published
- `PLAN.md`

### `8b44754` · 2026-10-08 · docs: sync PLAN and CLAUDE.md with release-parse, db import and What's new fix
- `CLAUDE.md`
- `PLAN.md`

### `6a7210b` · 2026-10-08 · build(release): refuse a version without a CHANGELOG entry
- `scripts/release.mjs`

### `1a5eb81` · 2026-10-08 · fix: What's new shows after updates to 0.4.x and manual installs
- `PHASES.md`
- `src/WhatsNew.tsx`
- `src/changelog.ts`

### `4bada10` · 2026-10-08 · chore: release v0.4.2
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `fe95c93` · 2026-10-08 · docs: v0.4.2 checklist done (PHASES)
- `PHASES.md`

### `9b698a0` · 2026-10-08 · fix: built release databases are a single file (no WAL sidecars)
- `crates/nyaa-client/src/store.rs`

### `a85585a` · 2026-10-06 · feat: build_dataset example (crawl JSON -> release database)
- `FILE_INDEX.md`
- `crates/nyaa-client/examples/build_dataset.rs`

### `1215b3b` · 2026-10-06 · feat: import a nyaa.si release database (Settings > Backup)
- `FILE_INDEX.md`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/lib.rs`
- `src/SettingsPanel.tsx`

### `c1ddcf9` · 2026-10-06 · docs: plan v0.4.2 scope (parsing + db import), matching moves to v0.4.3
- `PHASES.md`
- `PLAN.md`

### `a46910f` · 2026-10-06 · feat: frontend uses backend release labels (movie/special groups)
- `FILE_INDEX.md`
- `src/App.tsx`
- `src/HlsPlayerView.tsx`
- `src/PlayerView.tsx`
- `src/episodeParser.ts`
- `src/releases.ts`
- `src/torrentThumbnail.ts`
- `src/types.ts`

### `3289434` · 2026-10-06 · feat: EpisodeLabel from release-parse on search results and play files
- `FILE_INDEX.md`
- `crates/nyaa-client/Cargo.toml`
- `crates/nyaa-client/src/label.rs`
- `crates/nyaa-client/src/lib.rs`
- `crates/nyaa-client/src/store.rs`
- `src-tauri/src/lib.rs`

### `aeefab3` · 2026-10-06 · feat(release-parse): typed Release result + fixes from the episodeParser diff
- `FILE_INDEX.md`
- `crates/release-parse/README.md`
- `crates/release-parse/examples/labels.rs`
- `crates/release-parse/src/element.rs`
- `crates/release-parse/src/extra.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/number.rs`
- `crates/release-parse/src/parser.rs`
- `crates/release-parse/src/release.rs`

### `ef939b7` · 2026-10-06 · test: nyaa title corpus fixture (10,151 titles)
- `FILE_INDEX.md`
- `src-tauri/tests/fixtures/nyaa_titles/corpus.txt`

### `58f203e` · 2026-10-06 · feat(release-parse): season ranges, roman-numeral seasons, ~ episode ranges
- `crates/release-parse/src/extra.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/number.rs`
- `crates/release-parse/src/parser.rs`

### `dbfb06a` · 2026-10-06 · feat(release-parse): CJK episode (第N话/集) and season (第N期/季) counters
- `crates/release-parse/src/extra.rs`
- `crates/release-parse/src/keyword.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/number.rs`
- `crates/release-parse/src/parser.rs`

### `2b639e8` · 2026-10-06 · feat: release-parse crate, faithful anitopy port (MPL-2.0)
- `FILE_INDEX.md`
- `crates/release-parse/Cargo.toml`
- `crates/release-parse/LICENSE`
- `crates/release-parse/README.md`
- `crates/release-parse/examples/parse_lines.rs`
- `crates/release-parse/src/element.rs`
- `crates/release-parse/src/helper.rs`
- `crates/release-parse/src/keyword.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/number.rs`
- `crates/release-parse/src/parser.rs`
- `crates/release-parse/src/token.rs`
- `crates/release-parse/src/tokenizer.rs`

### `2d1eabd` · 2026-10-06 · docs: plan release-parse crate (MPL-2.0 anitopy port)
- `PHASES.md`
- `PLAN.md`

### `5bdd011` · 2026-10-06 · docs: plan anitopy-style release parser in Rust
- `PHASES.md`
- `PLAN.md`

### `b7dc2a0` · 2026-10-04 · docs: fold title analysis into the season-aware matching plan
- `PHASES.md`
- `PLAN.md`

### `3817e9d` · 2026-10-04 · docs: nyaa title format analysis
- `FILE_INDEX.md`
- `TITLE_ANALYSIS.md`

### `b818365` · 2026-10-04 · test: nyaa title fixtures for season-aware matching
- `FILE_INDEX.md`
- `src-tauri/tests/fixtures/nyaa_titles/franchises.json`

### `8a8ae2b` · 2026-10-04 · docs: plan view-page fallback for unsure releases
- `PLAN.md`
- `PHASES.md`

### `885e304` · 2026-10-04 · docs: plan season-aware source matching (v0.4.2)
- `PLAN.md`
- `PHASES.md`

### `e9cefff` · 2026-10-04 · chore: release v0.4.1
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `662f1ad` · 2026-10-04 · build: link sbtl as a prebuilt DLL (native/sbtl)
- `native/sbtl/sbtl.dll`
- `native/sbtl/sbtl_import.lib`
- `native/sbtl/VERSION`
- `native/sbtl/README.md`
- `.cargo/config.toml`
- `src-tauri/tauri.conf.json`
- `scripts/release.mjs`
- `package.json`
- `PLAN.md`
- `FILE_INDEX.md`

### `4c1e4bf` · 2026-10-04 · chore: sbtl v0.1.1
- `crates/sbtl-engine/Cargo.toml`
- `crates/sbtl-engine/src/torrent.rs`
- `PLAN.md`

### `c43ec45` · 2026-10-04 · fix(release): --notes-file for multi-line release notes
- `scripts/release.mjs`
- `CLAUDE.md`

### `15f492d` · 2026-10-04 · chore: release v0.4.0
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### `2b7791a` · 2026-10-04 · docs: resume buffers verified live; dropped-buffer delete issue (PHASES)
- `PHASES.md`

### `b970409` · 2026-10-04 · fix(player): one progress key per session; resume plays its saved source
- `src/MediaPage.tsx`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`

### `2dfc515` · 2026-10-04 · docs: fixed progress key and resume source fix for v0.4.0 (PHASES)
- `PHASES.md`

### `fda1e2c` · 2026-10-04 · docs: v0.4.0 live test results, libtorrent vs sbtl_engine (PLAN, PHASES)
- `PLAN.md`
- `PHASES.md`

### `8532f1d` · 2026-10-04 · fix(resume): a save no longer deletes its own buffer
- `src-tauri/src/resume.rs`

### `ae4b79f` · 2026-10-04 · docs: sbtl_engine engine_smoke A/B results (PLAN, PHASES)
- `PLAN.md`
- `PHASES.md`

### `0cf3c1c` · 2026-10-04 · docs: sbtl_engine as built, enginefs gone (PLAN, PHASES, FILE_INDEX, CLAUDE, README)
- `PLAN.md`
- `PHASES.md`
- `FILE_INDEX.md`
- `CLAUDE.md`
- `README.md`

### `88c33b5` · 2026-10-04 · refactor: remove the vendored enginefs
- `Cargo.toml`
- `vendor/enginefs/` (removed)

### `e0353a3` · 2026-10-04 · refactor: torrent-engine runs on sbtl_engine; watch hint removed
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/src/direct_input.rs`
- `crates/torrent-engine/src/resume_buffer.rs`
- `src-tauri/src/lib.rs`
- `src/playback.ts`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`

### `f39e19b` · 2026-10-04 · feat(sbtl-engine): torrent engine crate over sbtl
- `crates/sbtl-engine/Cargo.toml`
- `crates/sbtl-engine/LICENSE-enginefs`
- `crates/sbtl-engine/src/lib.rs`
- `crates/sbtl-engine/src/torrent.rs`
- `crates/sbtl-engine/src/magnet.rs`
- `crates/sbtl-engine/src/trackers.rs`
- `crates/sbtl-engine/src/tracker_prober.rs`

### `d4e9410` · 2026-10-04 · docs: plan sbtl_engine, replacing the vendored enginefs (PLAN, PHASES)
- `PLAN.md`
- `PHASES.md`

### `b0a49ed` · 2026-10-04 · docs: sbtl-backend merged (PHASES)
- `PHASES.md`

### `1f95811` · 2026-10-04 · Merge branch 'sbtl-backend': sbtl becomes the only torrent engine
- (merge: see the sbtl-backend entries below)

### `c934dc9` · 2026-10-04 · docs: sbtl engine checklist for v0.4.0 (PHASES)
- `PHASES.md`

### `f155f39` · 2026-10-04 · docs: sbtl is the only engine (VENDORED, FILE_INDEX, CLAUDE, README)
- `vendor/enginefs/VENDORED.md`
- `FILE_INDEX.md`
- `CLAUDE.md`
- `README.md`

### `4875f10` · 2026-10-04 · docs: sbtl is the only engine, sbtl update workflow, v0.4.0 release plan (PLAN)
- `PLAN.md`

### `491c50c` · 2026-10-04 · feat(release): refuse to build without a tagged sbtl; dev:sbtl-local
- `scripts/release.mjs`
- `package.json`

### `96374a9` · 2026-10-04 · build: vcpkg builds only FFmpeg
- `vcpkg.json`
- `.cargo/config.toml`
- `scripts/fetch-native-deps.mjs`

### `7d6d02a` · 2026-10-04 · fix(cache): keep sbtl's state directory, evict a torrent's sbtl files
- `src-tauri/src/download_cache.rs`

### `5b22bbe` · 2026-10-04 · refactor: sbtl is the only torrent engine
- `Cargo.toml`
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/examples/engine_smoke.rs`
- `crates/torrent-engine/src/lib.rs`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`
- `src/types.ts`
- `vendor/enginefs/Cargo.toml`
- `vendor/enginefs/src/lib.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/librqbit.rs` (removed)
- `vendor/enginefs/src/backend/libtorrent/` (removed)
- `vendor/libtorrent-sys/` (removed)

### `41e5bbc` · 2026-10-04 · fix(release): only npm runs through a shell on Windows
- `scripts/release.mjs`

### `ec71470` · 2026-10-04 · docs: sbtl naming and git dependency (PLAN, FILE_INDEX, VENDORED)
- `PLAN.md`
- `FILE_INDEX.md`
- `vendor/enginefs/VENDORED.md`

### `250d871` · 2026-10-04 · refactor: tl engine renamed sbtl, built from its public repo
- `vendor/enginefs/Cargo.toml`
- `vendor/enginefs/src/lib.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/sbtl_backend.rs`
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/src/lib.rs`
- `crates/torrent-engine/examples/engine_smoke.rs`
- `src-tauri/Cargo.toml`
- `package.json`
- `src/types.ts`
- `src/loadingProgress.ts`

### `2253456` · 2026-10-04 · fix(ui): pin home app bar flush under title bar
- `src/App.css`

### `5aa5736` · 2026-10-03 · docs: tl backend review fixes (PLAN, FILE_INDEX)
- `PLAN.md`
- `FILE_INDEX.md`

### `dff9fa3` · 2026-10-03 · fix(enginefs): tl backend review fixes
- `vendor/enginefs/src/backend/tl_backend.rs`

### `b37c74f` · 2026-10-03 · docs: tl backend gains DHT, uTP, seeding, HTTPS trackers
- `PLAN.md`
- `FILE_INDEX.md`

### `e42f4d2` · 2026-10-03 · feat(enginefs): tl backend uses tl's DHT state, seeding and upload throttle
- `vendor/enginefs/src/backend/tl_backend.rs`

### `5672b89` · 2026-10-03 · docs: tl buffer signals (PLAN, FILE_INDEX, VENDORED)
- `PLAN.md`
- `FILE_INDEX.md`
- `vendor/enginefs/VENDORED.md`

### `d729ed2` · 2026-10-03 · test: engine_smoke playback phase prints buffer levels
- `crates/torrent-engine/examples/engine_smoke.rs`

### `59f0f2b` · 2026-10-03 · feat: buffering progress from the backend's real buffer when available
- `src/types.ts`
- `src/loadingProgress.ts`

### `61970c0` · 2026-10-03 · feat: StreamStats.buffer and per-backend mpv cache-pause-wait
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/player.rs`

### `612dbd7` · 2026-10-03 · feat(enginefs): buffer_status in TorrentHandle, implemented by the tl backend
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/tl_backend.rs`

### `7f82268` · 2026-10-03 · docs: engine_smoke and tl vs libtorrent measurements
- `FILE_INDEX.md`
- `PLAN.md`

### `fa6cf98` · 2026-10-03 · test: engine_smoke example for the Range-GET streaming path
- `crates/torrent-engine/examples/engine_smoke.rs`

### `c6112de` · 2026-10-03 · fix(enginefs): tl backend file names use native separators like libtorrent
- `vendor/enginefs/src/backend/tl_backend.rs`

### `38143f7` · 2026-10-02 · docs: tl backend (PLAN, FILE_INDEX, VENDORED)
- `PLAN.md`
- `FILE_INDEX.md`
- `vendor/enginefs/VENDORED.md`

### `df84a29` · 2026-10-02 · feat: build-time engine switch (libtorrent default, tl optional)
- `crates/torrent-engine/Cargo.toml`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/Cargo.toml`
- `package.json`

### `f19325d` · 2026-10-02 · feat(enginefs): tl backend behind the tl feature
- `vendor/enginefs/src/backend/tl_backend.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/lib.rs`
- `vendor/enginefs/Cargo.toml`

### `997b931` · 2026-09-30 · docs: load profiler as built, tasks done
- `PHASES.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `d2e1f62` · 2026-09-30 · feat: frontend load traces for playback start and seeks
- `src/loadTrace.ts`
- `src/playback.ts`
- `src/mpvVideo.ts`
- `src/PlayerView.tsx`

### `85c835e` · 2026-09-30 · feat: stream read accounting in load trace summaries
- `crates/torrent-engine/src/read_stats.rs`
- `crates/torrent-engine/src/resume_buffer.rs`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/load_trace.rs`
- `src-tauri/src/lib.rs`

### `2186326` · 2026-09-30 · feat: trace play_magnet stages (add, metadata, checking, resume buffer)
- `src-tauri/src/lib.rs`
- `src-tauri/src/load_trace.rs`
- `src-tauri/src/resume.rs`
- `crates/torrent-engine/src/lib.rs`

### `4a1afa7` · 2026-09-30 · feat(enginefs): expose libtorrent torrent state in EngineStats
- `vendor/enginefs/VENDORED.md`
- `vendor/enginefs/src/backend/librqbit.rs`
- `vendor/enginefs/src/backend/libtorrent/helpers.rs`
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/lib.rs`

### `2efa6bf` · 2026-09-30 · fix: read resume buffer bytes through the engine, not the disk file
- `crates/torrent-engine/src/resume_buffer.rs`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/resume.rs`
- `PLAN.md`

### `c12464f` · 2026-09-30 · feat: load trace registry with stage logging and summaries
- `src-tauri/src/load_trace.rs`
- `src-tauri/src/lib.rs`

### `6a8a811` · 2026-09-30 · docs: plan the load profiler for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `08e93b1` · 2026-09-30 · docs: Discord contact by username
- `PLAN.md`
- `PHASES.md`

### `a7de70b` · 2026-09-30 · feat: show the Discord username with a copy button
- `src/HelpSection.tsx`
- `src/App.css`

### `90f80d3` · 2026-09-30 · docs: subtitle preloading and send logs as built
- `PHASES.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `0936f77` · 2026-09-30 · feat: Settings Help section with Contact and Send logs
- `src/HelpSection.tsx`
- `src/SettingsPanel.tsx`
- `src/App.css`

### `8a6e35a` · 2026-09-30 · feat: export_logs zips scrubbed logs and system info
- `src-tauri/src/logs_export.rs`
- `src-tauri/src/player.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`

### `f51a487` · 2026-09-30 · fix: subtitle preload and visibility-based off (completes 541e0bb)
- `src/mpvVideo.ts`
- `src/PlayerView.tsx`

### `541e0bb` · 2026-09-30 · fix: preload subtitles and hide them without dropping the track
- `src/mpvVideo.ts`
- `src/PlayerView.tsx`

### `56223f5` · 2026-09-30 · docs: resume buffer as built, tasks done
- `PHASES.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `36f269e` · 2026-09-30 · feat: sweep resume buffers with Continue watching and Clear cache
- `src-tauri/src/resume.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/cache.rs`

### `edef49a` · 2026-09-30 · feat: serve resumed streams from their resume buffer
- `crates/torrent-engine/src/resume_buffer.rs`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/resume.rs`
- `src-tauri/src/lib.rs`

### `f4873a8` · 2026-09-30 · feat: progress entries remember their source; Resume prefers it
- `src/watchProgress.ts`
- `src/PlayerView.tsx`

### `11f90ab` · 2026-09-30 · feat: save a resume buffer when an in-progress episode stops
- `crates/torrent-engine/src/resume_buffer.rs`
- `src-tauri/src/resume.rs`
- `src-tauri/src/lib.rs`
- `src/PlayerView.tsx`
- `src/playback.ts`
- `src/watchProgress.ts`

### `287cfb0` · 2026-09-30 · feat: keyframe byte offset lookup and verified file ranges
- `crates/torrent-engine/src/media.rs`
- `crates/torrent-engine/src/lib.rs`

### `8b0532b` · 2026-09-30 · feat: record the byte ranges mpv reads to open a file
- `crates/torrent-engine/src/resume_buffer.rs`
- `crates/torrent-engine/src/lib.rs`
- `src-tauri/src/lib.rs`
- `src/playback.ts`
- `src/PlayerView.tsx`

### `30756b8` · 2026-09-30 · docs: download cache as built, tasks done
- `PHASES.md`
- `PLAN.md`
- `FILE_INDEX.md`

### `904814a` · 2026-09-30 · test: download cache eviction, keep and clear
- `src-tauri/src/download_cache.rs`

### `ebf8cd9` · 2026-09-30 · feat: download cache size setting and clear
- `src-tauri/src/download_cache.rs`
- `src-tauri/src/cache.rs`
- `src-tauri/src/lib.rs`
- `src/SettingsPanel.tsx`

### `f5ea62a` · 2026-09-30 · feat: keep Continue watching torrents past the download cache cap
- `src-tauri/src/download_cache.rs`
- `src-tauri/src/lib.rs`
- `src/downloadCache.ts`
- `src/main.tsx`

### `87da9dc` · 2026-09-30 · feat: LRU-evict the download cache after stop and at startup
- `src-tauri/src/download_cache.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`

### `da0e6ca` · 2026-09-30 · feat: download cache index of played torrents
- `src-tauri/src/download_cache.rs`
- `src-tauri/src/lib.rs`
- `src/playback.ts`
- `src/PlayerView.tsx`
- `src/HlsPlayerView.tsx`

### `d340a55` · 2026-09-30 · docs: mark splash/title bar tasks done, index new files
- `PHASES.md`
- `FILE_INDEX.md`

### `5f52a7a` · 2026-09-30 · feat: frameless window with custom title bar
- `src/TitleBar.tsx`
- `src/main.tsx`
- `src/App.css`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`

### `64d7618` · 2026-09-30 · feat: native splash window shown until app_ready
- `public/splash.html`
- `index.html`
- `src/main.tsx`
- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`

### `e022727` · 2026-09-29 · docs: plan navigation/home rehaul and Discover for v0.9.x
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`

### `bfe02c5` · 2026-09-29 · docs: plan 32-bit Windows support for v1.0.0
- `PLAN.md`
- `PHASES.md`

### `7e90fe0` · 2026-09-29 · docs: drop Linux/multiplatform, v1.0.0 is the stable Windows release
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`

### `fe2a6d6` · 2026-09-29 · docs: run Linux build testing in cloud sessions
- `PLAN.md`
- `PHASES.md`

### `fda4a99` · 2026-09-29 · docs: plan Windows + Linux multiplatform debut as v1.0.0
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`

### `8fddaf3` · 2026-09-29 · docs: plan subtitle off as sub-visibility, from log findings
- `PLAN.md`
- `PHASES.md`

### `da8c1c6` · 2026-09-29 · docs: plan subtitle preloading for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `1849140` · 2026-09-29 · docs: move contact and send logs to v0.4.0, publish v0.4.0 and v0.8.0
- `PLAN.md`
- `PHASES.md`
- `CLAUDE.md`

# commits.md

Commit log, newest first. Prepend a new entry after every commit.

### `934596f` · 2026-09-29 · docs: give the README a centered header with logo and badges
- `README.md`

### `0f2ad46` · 2026-09-29 · docs: plan the native splash window and custom title bar for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `f1d87db` · 2026-09-29 · docs: correct the startup delay cause in PLAN.md
- `PLAN.md`

### `a1cd03d` · 2026-09-29 · fix: serve the dev frontend on 127.0.0.1
- `vite.config.ts`
- `src-tauri/tauri.conf.json`

### `bbfa40b` · 2026-09-29 · docs: record the mpv pre-spawn regression and fix
- `FILE_INDEX.md`
- `PLAN.md`

### `25d8d82` · 2026-09-29 · fix: pre-spawn mpv after the page loads, not at launch
- `src-tauri/src/lib.rs`
- `src-tauri/src/player.rs`

### `774419e` · 2026-09-29 · docs: mention the new icon and splash in the 0.3.2 notes
- `src/changelog.ts`

### `29e0ab1` · 2026-09-29 · docs: tick splash and logo in PHASES.md
- `PHASES.md`

### `62d7829` · 2026-09-29 · chore: regenerate installer art from the new icon
- `src-tauri/installer/banner.bmp`
- `src-tauri/installer/dialog.bmp`

### `2a01e3c` · 2026-09-29 · chore: regenerate app icons from the polished logo
- `src-tauri/icons/`

### `82d462a` · 2026-09-29 · feat: polish the logo
- `src/icons.tsx`
- `public/favicon.svg`
- `src-tauri/icons/icon.svg`
- `index.html`

### `565e859` · 2026-09-29 · docs: plan the logo polish for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `5ce72b2` · 2026-09-29 · feat: add an animated splash screen
- `index.html`
- `src/main.tsx`

### `4dad80a` · 2026-09-29 · docs: plan the splash screen for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `89a1dd5` · 2026-09-29 · docs: index the what's new files in FILE_INDEX.md
- `FILE_INDEX.md`

### `8f99515` · 2026-09-29 · feat: show what's new after an update
- `src/changelog.ts`
- `src/WhatsNew.tsx`
- `src/updater.ts`
- `src/main.tsx`
- `src/App.css`
- `PHASES.md`
- `PLAN.md`

### `448c117` · 2026-09-29 · docs: plan the what's new dialog for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `60b9215` · 2026-09-29 · chore: release v0.3.2
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `PHASES.md`

### `fe76205` · 2026-09-29 · docs: sync FILE_INDEX.md and PLAN.md for v0.3.2
- `FILE_INDEX.md`
- `PLAN.md`

### `a921784` · 2026-09-29 · fix: grow the home hero to fit long titles
- `src/App.css`
- `src/HomePage.tsx`
- `PHASES.md`

### `87cea5c` · 2026-09-29 · feat: rebuild mpv's buffer when corruption is detected
- `src/mpvVideo.ts`
- `PHASES.md`

### `8277417` · 2026-09-29 · feat: turn mpv corruption log lines into stream-corrupt events
- `crates/mpv-player/src/libmpv.rs`
- `PHASES.md`

### `3f832b8` · 2026-09-29 · docs: plan rebuilding the buffer on corruption
- `PLAN.md`
- `PHASES.md`

### `4a61101` · 2026-09-29 · docs: plan the hero long-title fix
- `PLAN.md`
- `PHASES.md`

### `7b36720` · 2026-09-29 · fix: serve freshly verified pieces from libtorrent's own copy
- `vendor/enginefs/src/piece_waiter.rs`
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`
- `vendor/enginefs/VENDORED.md`
- `PHASES.md`

### `885cdc6` · 2026-09-29 · docs: plan the fresh-piece corruption fix
- `PLAN.md`
- `PHASES.md`

### `b152132` · 2026-09-29 · docs: index vendored libtorrent-sys in FILE_INDEX.md
- `FILE_INDEX.md`

### `52f028d` · 2026-09-29 · fix: stop slow peers from holding urgent pieces
- `vendor/libtorrent-sys/cpp/wrapper.cpp`
- `vendor/libtorrent-sys/VENDORED.md`
- `PHASES.md`

### `d3a633c` · 2026-09-29 · build: use the vendored libtorrent-sys
- `Cargo.toml`

### `e7c5459` · 2026-09-29 · chore: vendor libtorrent-sys unchanged from stream-server f585ab6
- `vendor/libtorrent-sys/`

### `375a539` · 2026-09-29 · fix: prefetch 1% of the file tail, 4-16 MB
- `vendor/enginefs/src/backend/priorities.rs`
- `vendor/enginefs/VENDORED.md`
- `PHASES.md`

### `0cdf5bf` · 2026-09-29 · feat: add a hardware video decoding setting
- `src/settings.ts`
- `src/SettingsPanel.tsx`
- `src/PlayerView.tsx`
- `PHASES.md`

### `7378931` · 2026-09-29 · feat: log mpv's own warnings and errors
- `crates/mpv-player/src/libmpv.rs`
- `crates/mpv-player/src/embedded.rs`
- `PHASES.md`

### `f86c1de` · 2026-09-29 · docs: plan buffering round 3 and decoder diagnostics
- `PLAN.md`
- `PHASES.md`

### `251e2fe` · 2026-09-29 · docs: plan the download cache for v0.4.0
- `PLAN.md`
- `PHASES.md`

### `82baaf1` · 2026-09-29 · docs: note buffering round 2 in VENDORED.md
- `vendor/enginefs/VENDORED.md`

### `8f3ca90` · 2026-09-29 · docs: record buffering round 2 in PLAN.md and PHASES.md
- `PLAN.md`
- `PHASES.md`

### `037ec06` · 2026-09-29 · fix: re-anchor in-order download 100ms after a blocked seek
- `vendor/enginefs/src/backend/priorities.rs`

### `bdeff55` · 2026-09-29 · fix: start and resume playback once 10s is buffered
- `crates/mpv-player/src/embedded.rs`

### `0f05a78` · 2026-09-29 · fix: prioritize index reads and grade a bitrate-sized read-ahead
- `vendor/enginefs/src/backend/libtorrent/disk_stream.rs`

### `e6cc73a` · 2026-09-29 · fix: prefetch the file tail and hold resumes until the real resume point
- `vendor/enginefs/src/backend/libtorrent/playback.rs`

### `50dea5a` · 2026-09-29 · docs: compare buffering plan with Elementum
- `PLAN.md`

### `5e244d9` · 2026-09-29 · feat: add tail prefetch, read-ahead and resume-point helpers to enginefs
- `vendor/enginefs/src/backend/priorities.rs`

### `2c72189` · 2026-09-29 · fix: let mpv refill 3s of cache before resuming after a stall
- `crates/mpv-player/src/embedded.rs`
- `PHASES.md`

### `1f58003` · 2026-09-29 · docs: plan playback buffering fixes from the live test
- `PLAN.md`
- `PHASES.md`

### `b711df6` · 2026-09-29 · docs: record the seek-bar indicator implementation
- `PLAN.md`
- `PHASES.md`

### `dd89523` · 2026-09-29 · feat: show downloaded ranges on the mpv seek bar
- `src/PlayerView.tsx`
- `src/App.css`

### `c36a697` · 2026-09-29 · feat: expose downloaded byte ranges in StreamStats
- `crates/torrent-engine/src/lib.rs`
- `src/types.ts`

### `757a36c` · 2026-09-29 · feat: report per-file downloaded byte ranges in enginefs stats
- `vendor/enginefs/src/backend/mod.rs`
- `vendor/enginefs/src/backend/libtorrent/handle.rs`
- `vendor/enginefs/src/backend/librqbit.rs`
- `vendor/enginefs/src/lib.rs`
- `vendor/enginefs/VENDORED.md`

### `8f64aa3` · 2026-09-29 · fix: keep 150 MiB of played video in mpv's back buffer
- `crates/mpv-player/src/embedded.rs`
- `PHASES.md`

### `e1f28d0` · 2026-09-29 · docs: plan seek-bar downloaded and buffer indicators for v0.3.2
- `PLAN.md`
- `PHASES.md`

### `133bc36` · 2026-09-29 · docs: index .vscode/settings.json in FILE_INDEX.md
- `FILE_INDEX.md`

### `8edec7d` · 2026-09-29 · chore: give rust-analyzer its own target dir
- `.vscode/settings.json`
- `.gitignore`
- `PHASES.md`

### `0c067c1` · 2026-09-29 · chore: link dev builds with rust-lld
- `.cargo/config.toml`
- `scripts/release.mjs`
- `PLAN.md`
- `PHASES.md`

### `1530ecc` · 2026-09-29 · chore: trim dev-profile debug info
- `Cargo.toml`

### `937fe35` · 2026-09-29 · docs: plan dev build speed changes
- `PLAN.md`
- `PHASES.md`

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

### `d7d45da` · 2026-08-30 · feat(player): hover-only solid control bar + player keyboard shortcuts
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
### `b4061f6` · 2026-10-28 · docs: plan animebatch parser replacement
- `PLAN.md`
- `PHASES.md`
### `37b7f14` · 2026-10-28 · docs: retain animebatch source for port
- `crates/release-parse/upstream/animebatch/`
- `FILE_INDEX.md`
- `PHASES.md`
### `a930d8e` · 2026-10-28 · feat: port animebatch release rules
- `crates/release-parse/src/batch.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/release.rs`
- `crates/release-parse/tests/animebatch.rs`
- `crates/release-parse/tests/animebatch_cases.json`
- `FILE_INDEX.md`
- `PHASES.md`
### `8e6bd92` · 2026-10-28 · feat: port animebatch search and guessit fields
- `crates/release-parse/src/animebatch.rs`
- `crates/release-parse/src/batch.rs`
- `crates/release-parse/src/lib.rs`
- `crates/release-parse/src/release.rs`
- `crates/release-parse/Cargo.toml`
- `crates/release-parse/tests/animebatch.rs`
- `crates/release-parse/tests/animebatch_oracle.json`
- `FILE_INDEX.md`
- `PHASES.md`
### `cf31982` · 2026-10-28 · fix: constrain animebatch range parsing
- `crates/release-parse/src/batch.rs`
- `crates/release-parse/src/release.rs`
- `crates/release-parse/tests/animebatch.rs`
- `PLAN.md`
- `PHASES.md`
- `CHANGELOG.md`
