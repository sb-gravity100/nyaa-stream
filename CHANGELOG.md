# Changelog

All notable changes to nyaa-stream. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

The in-app "What's new" window reads `src/changelog.ts`; keep both files in
step when cutting a release.

## [Unreleased]

## [0.6.0] - Unreleased

### Added
- Flat, keyboard-accessible context menus for inputs, anime, episode/source rows and the player, including custom-source actions.
- Paste a magnet or info hash to play it directly or search for an anime and attach it to an episode or pack; existing sources can be edited, moved or removed.
- Custom packs now appear on their numbered episode rows using inspected file and season metadata; extras are excluded and unknown packs stay on a Batch row.
- TMDB/fanart.tv artwork fallbacks (including IMDb-based TMDB lookup when an ID crosswalk is missing), provider-key settings, episode stills, saved Nyaa release results with an explicit refresh action, and a cancellable missing-thumbnail queue that picks the smallest adequately seeded source and captures up to ten frames at once.
- Seek-bar frame previews generated only from verified local torrent data.
- Quiet update checks, in-app download/verification progress, deferred restart and branded NSIS install-choice and progress pages.
- Interactive upgrades retain the user's existing Start Menu and desktop shortcut choices.
- The Windows installers include the MSVC runtime required by the native torrent bridge.

### Removed
- Experimental ML training/scoring tools, debugger predictions and unused dataset-mapper training. Rule-based matching and existing dataset compatibility are retained.

### Added
- Selective ASS dialogue style overrides for font/weight, size, text color, outline/background/shadow and bottom margin, shared by embedded mpv, HLS playback and clip export. Unchecked fields and authored inline effects are retained.

### Changed
- Embedded mpv uses `blend-subtitles=yes` and `scale=ewa_lanczos`, retaining scaler blur 1.5, Mitchell downscaling and subtitle blur 1.0.

### Fixed
- Plain-text subtitles use light font hinting in mpv playback and clip export; styled ASS tracks retain unhinted positioning and animation.
- HLS subtitle rendering uses 1.5x device-pixel supersampling up to a 2160px threshold, retaining native resolution on larger outputs.
- Updating now saves the playback resume buffer and waits for torrent teardown before starting the installer.
- Artwork provider credentials remain outside exported/imported backups.

## [0.5.8] - 2026-10-29

### Changed
- Release parsing now recognizes more batch episode ranges, box sets and season packs using the Rust port of animebatch's rules.
- A Rust animebatch API provides season-aware search and normalized title, source and resolution metadata without Python dependencies.

### Fixed
- Long exact-title season searches are split into shorter nyaa.si queries, avoiding HTTP 400 failures while retaining their title alternatives.
- Seek timing logs now stop at playback restart and include the requested and resumed positions; follow-up stats collection no longer inflates the reported delay.
- Audio decoder errors include playback and track context and no longer trigger video-buffer recovery.
- Search and request failure logs include concise query size and HTTP status details.
- The pointer remains visible across the player surface and hides only after inactivity, independently of control-bar hover state.
- HLS fallback subtitles use native-resolution libass coverage antialiasing with WebGL composition when available.
- Embedded mpv uses GPU video-stage subtitle blending with EWA Lanczos sharp, blur 1.5, and Mitchell downscaling; scaler blur applies to the image.
- Embedded mpv adds sub-blur=1.0 to soften subtitle borders.

## [0.5.7] - 2026-10-27

### Fixed
- Nyaa searches now target the selected anime entry with exact title and season phrases; ambiguous unmarked shared titles are excluded from season pages.
- Same-season AniList aliases improve split-cour and translated-title search coverage.
- Movie, OVA and special searches use the entry's full names rather than the franchise base.

## [0.5.6] - 2026-10-26

### Fixed
- Batch torrent mapping now checks explicit sparse episode lists without treating skipped numbers as missing.
- Files named with a numbered `Extra` marker no longer duplicate the matching episode in a pack.

## [0.5.5] - 2026-10-26

### Fixed
- Fullscreen removes the title-bar gap while keeping the home header visible.
- Development logs are written to the working directory used to launch the app.
- Player controls no longer flicker when the pointer is at the top edge.
- Player controls appear only over the top or bottom controls and fade out as soon as the pointer leaves.

## [0.5.0] - 2026-10-13

### Added
- AnimeTosho dataset: Settings → AnimeTosho dataset → **Import Tosho Data** downloads AnimeTosho's database dump from its torrent (only the 2.7 GB file, resumable, cancellable), builds a local dataset in-app and imports it. It adds ~1M releases offline, exact release-to-show matching through AniDB ids, batch file lists and AniDB→TVDB season maps. **Import from file** loads a prebuilt dataset.
- Settings opens as a centered modal.
- Search defaults to nyaa's "all anime" category (`c=1_0`) instead of English-translated only.
- Season-aware source matching: an entry's seasons come from its titles, TVDB (Fribb/anime-lists, cached weekly) and its AniList franchise (a TV entry with no TV prequel is season 1).
- Releases written with just the short name and a season ("Mushoku Tensei S03E12") now match that season's entry; later-season entries also search the short base name.
- Season packs without an episode number are batches of the matched season; a torrent's file list (scraped from its nyaa.si view page, kept permanently in the release database) decides the unclear ones.
- Files inside a pack take their season from a parent folder ("Season 2/"), and the player prefers the episode group's season when a pack holds the same episode number in several seasons.

### Fixed
- Season 3 of a long-running show no longer lists stray season 1 and 2 episode rows; releases that write the season as a numeral ("Mushoku Tensei III - 07") join the right season.
- Searching local releases in "all anime" now includes every anime subcategory.
- A season's page no longer lists other seasons' releases.
- Sequels with their own longer names ("Food Wars! The Second Plate") go to their own entry.
- Movies, OVAs and specials stay off a TV season's episode list.
- Titles with apostrophes ("Journey's End") match releases written without them.

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

[Unreleased]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.4.0...v0.4.2
[0.4.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/sb-gravity100/nyaa-stream/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sb-gravity100/nyaa-stream/releases/tag/v0.1.0
