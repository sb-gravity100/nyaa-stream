# Changelog

All notable changes to nyaa-stream. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

The in-app "What's new" window reads `src/changelog.ts`; keep both files in
step when cutting a release.

## [Unreleased]

### Added
- Search defaults to nyaa's "all anime" category (`c=1_0`) instead of English-translated only.

### Planned
- v0.4.3: season-aware source matching (see `PLAN.md`).

## [0.4.2] - 2026-10-08

### Added
- Much better at reading release names: seasons written as II/III/IV, season packs like "S01-S04" or "Season 1-3", "01 ~ 24" batches, Chinese and Japanese episode numbers, and 4-digit episodes. Backed by the new `release-parse` crate (an MPL-2.0 port of anitopy plus nyaa fixes).
- Movies and OVAs/specials get their own groups instead of "Unknown".
- Settings → Backup → nyaa.si release database: import a release database to search those releases instantly, even offline.

### Fixed
- Audio channels, frame rates and "H 264" in release names are no longer mistaken for episode numbers.
- An S01E06-style number now wins over a different season mentioned elsewhere in the name.
- The What's new window now appears after updating to 0.4.x, and after updates installed outside the app.
- Built release databases are a single file (no WAL sidecars).

## [0.4.1] - 2026-10-04

### Fixed
- Episodes start faster: connecting no longer waits on peers that don't answer.

## [0.4.0] - 2026-10-04

### Added
- New torrent engine ([sbtl](https://github.com/sb-gravity100/sbtl)) replacing libtorrent: faster startup and much faster seeking, and seeking into parts that aren't downloaded yet no longer stalls. The installer is smaller too.
- Continue watching resumes instantly: the first seconds of an unfinished episode are kept, and Resume reopens the exact release you were watching.
- Download cache: played episodes stay on disk (10 GB by default, adjustable in Settings), so rewatching or seeking back doesn't download again.
- A native loading screen while the app starts, and a custom title bar.
- Settings → Help: Contact and Send logs, for reporting problems.
- Load profiler: playback start and seek stages are traced in the logs.

### Fixed
- Subtitles show on the first play, and switching subtitles no longer re-buffers.

## [0.3.2] - 2026-09-29

### Added
- The seek bar shows which parts of the episode are already downloaded.
- Hardware video decoding setting (Settings → Playback). Turn it off if video turns blocky until you seek.
- A refreshed app icon and an animated loading screen.
- The What's new window, shown after each update.
- Frame stepping (`,` / `.`); subtitle delay moved to `-` / `=`.
- The next episode of a batch preloads at low priority.
- AniList banner art as a backdrop fallback when Kitsu has none; hero and backdrop art is upscaled and sharpened.

### Changed
- Playback starts once about 10 seconds are buffered, so it stutters less.
- Watching from the start downloads in order; resuming downloads from where you left off.
- The paused dimming and the centre play/pause/skip overlays are gone.

### Fixed
- Episodes start much faster: the beginning of the file and the part the player needs to open it download first.
- Seeking downloads the new spot first, right away.
- Blocky or smeared video on freshly downloaded parts; if it still happens, the player repairs it on its own.
- Switching sources continues at the same time, without a false "can't be played" error.
- Slow peers no longer hold up the next bit of video.
- Long titles fit in the home banner.

## [0.3.1] - 2026-09-29

### Added
- The home hero slides between featured shows.

### Fixed
- The home hero has a fixed, shorter height.

## [0.3.0] - 2026-09-29

### Added
- In-app updates from GitHub Releases (Settings → Updates).
- Picture-in-picture mini always-on-top window (`P`).

## [0.2.0] - 2026-09-29

### Added
- Rolling daily log files (last 7 kept) in `%LOCALAPPDATA%\nyaa-stream\logs\`.
- Branded installer banner and welcome-dialog art.

## [0.1.0] - 2026-09-29

### Added
- First release: AniList/Kitsu metadata, nyaa.si search and ranking, torrent streaming, and an embedded libmpv player with an HLS fallback.

[Unreleased]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.4.2...HEAD
[0.4.2]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.4.0...v0.4.2
[0.4.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sb-gravity100/nyaa-stream/releases/tag/v0.1.0
