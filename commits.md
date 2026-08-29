# commits.md

Commit log, newest first. Prepend a new entry after every commit.

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
