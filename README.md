# nyaa-stream

Anime-focused, Stremio-inspired desktop streaming client for Windows. Search
anime (AniList, with a Kitsu fallback), pick a release from nyaa.si, and
stream the torrent straight into the player while it downloads.

## Features

- Anime search, library, watch progress, "continue watching" and new-episode rows
- nyaa.si release matching per show, season and episode, with fansub-group and resolution preferences
- Torrent streaming via a local server (libtorrent) — no waiting for the full download
- Playback in embedded [mpv](https://mpv.io/) (subtitles, tracks, speed, A-B loop, clip export, frame capture), with an HLS/FFmpeg browser player as a fallback when mpv isn't available
- Picture-in-picture mini window (`P`), fullscreen (`F`), media keys
- Backup/restore of library, progress and settings; in-app updates from GitHub Releases

## Stack

Tauri 2 (Rust backend in `src-tauri/` and `crates/*`, Preact + TypeScript frontend in `src/`).
See [PLAN.md](PLAN.md) for architecture and [FILE_INDEX.md](FILE_INDEX.md) for a file map.

## Building

Prerequisites: Rust, Node, CMake, MSVC (VS 2022 Build Tools), [vcpkg](https://github.com/microsoft/vcpkg)
with `VCPKG_ROOT` set, and LLVM (libclang). The first build compiles libtorrent, OpenSSL and
FFmpeg through vcpkg (~15 minutes, cached afterwards). Details in PLAN.md's "Build prerequisites".

Put a GPL build of `libmpv-2.dll` in `src-tauri/lib/` (see its README); without it the app uses the HLS fallback player.

```bash
npm install
npm run tauri dev      # run
npm run tauri build    # installer (needs TAURI_SIGNING_PRIVATE_KEY_PATH for updater artifacts)
```

## Releasing

`npm run bump -- patch|minor|major`, commit `chore: release vX.Y.Z`, tag `vX.Y.Z`, build with the
signing key set, and attach the installer, its `.sig` and `latest.json` to the GitHub release.
The app checks `releases/latest/download/latest.json` for updates.

## Disclaimer

nyaa-stream is a client for public services (AniList, Kitsu, nyaa.si) and the BitTorrent network.
It hosts no content. You are responsible for what you download and stream.
