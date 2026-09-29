# nyaa-stream

An anime-focused, Stremio-inspired desktop streaming client for Windows. Search
for a show, pick a release from [nyaa.si](https://nyaa.si), and stream the
torrent straight into the player while it downloads — no waiting for the full
file.

## Features

**Discover**
- Anime search and metadata from AniList, with a Kitsu fallback
- Library, watch progress, "continue watching" and new-episode rows

**Stream**
- nyaa.si release matching per show, season and episode, with fansub-group and resolution preferences
- Torrent streaming through a local HTTP server (libtorrent) with Range support

**Play**
- Embedded [mpv](https://mpv.io/) player: subtitles, audio/subtitle tracks, speed, A-B loop, clip export and frame capture
- HLS/FFmpeg browser player as an automatic fallback when mpv isn't available
- Picture-in-picture mini window (`P`), fullscreen (`F`), media keys

**Manage**
- Backup and restore of library, progress and settings
- In-app updates from GitHub Releases

## Tech stack

| Layer | Technology |
| --- | --- |
| App shell | [Tauri 2](https://tauri.app/) |
| Frontend | Preact + TypeScript + Vite (`src/`) |
| Backend | Rust (`src-tauri/`, `crates/*`) |
| Torrents | libtorrent, local streaming server |
| Playback | libmpv (embedded), FFmpeg + hls.js (fallback) |
| Metadata | AniList GraphQL, Kitsu, nyaa.si RSS |

## Project layout

```
src/                 Preact/TS frontend (views, player controls)
src-tauri/           Tauri app: commands, embedded mpv window, config
crates/
  torrent-engine/    torrent client, streaming server, HLS/FFmpeg pipeline
  nyaa-client/       nyaa.si search
  anilist-client/    AniList metadata
  kitsu-client/      Kitsu metadata (fallback)
  mpv-player/        libmpv bindings (playback + thumbnails)
```

See [PLAN.md](PLAN.md) for architecture and [FILE_INDEX.md](FILE_INDEX.md) for a full file map.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) and Node.js
- CMake and MSVC (Visual Studio 2022 Build Tools)
- [vcpkg](https://github.com/microsoft/vcpkg) with `VCPKG_ROOT` set
- LLVM (libclang)
- A GPL build of `libmpv-2.dll` in `src-tauri/lib/` (see its README). Without it, the app uses the HLS fallback player.

The first build compiles libtorrent, OpenSSL and FFmpeg through vcpkg (about 15 minutes, cached afterwards).
More detail is in PLAN.md's "Build prerequisites".

### Run

```bash
npm install
npm run tauri dev
```

### Build an installer

```bash
npm run tauri build
```

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `P` | Picture-in-picture mini window |
| `F` | Fullscreen |

## Disclaimer

nyaa-stream is a client for public services (AniList, Kitsu, nyaa.si) and the BitTorrent network.
It hosts no content. You are responsible for what you download and stream.
