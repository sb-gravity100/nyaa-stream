# commits.md

Commit log, newest first. Prepend a new entry after every commit.

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
