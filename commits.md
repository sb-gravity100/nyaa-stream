# commits.md

Commit log, newest first. Prepend a new entry after every commit.

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
