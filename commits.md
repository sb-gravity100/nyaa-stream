# commits.md

Commit log, newest first. Prepend a new entry after every commit.

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
