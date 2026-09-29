<div align="center">

<img src="src-tauri/icons/icon.svg" alt="nyaa-stream logo" width="128" height="128">

<h1>nyaa-stream</h1>

<p><strong>Stream anime straight from nyaa.si - no waiting for the download.</strong></p>

<p>An anime-focused, Stremio-inspired desktop client for Windows.<br>
Pick a release and it starts playing while the torrent downloads.</p>

<p>
  <a href="https://github.com/sb-gravity100/nyaa-stream/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/sb-gravity100/nyaa-stream?style=for-the-badge&color=f07aa6&labelColor=171a2b&label=release"></a>
  <a href="https://github.com/sb-gravity100/nyaa-stream/releases"><img alt="Downloads" src="https://img.shields.io/github/downloads/sb-gravity100/nyaa-stream/total?style=for-the-badge&color=f07aa6&labelColor=171a2b"></a>
  <img alt="Windows x64" src="https://img.shields.io/badge/Windows-x64-f07aa6?style=for-the-badge&labelColor=171a2b&logo=windows&logoColor=white">
</p>

<p>
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-CE422B?style=flat-square&logo=rust&logoColor=white">
  <img alt="Preact" src="https://img.shields.io/badge/Preact-frontend-673AB8?style=flat-square&logo=preact&logoColor=white">
  <img alt="libtorrent" src="https://img.shields.io/badge/libtorrent-streaming-3b3f5c?style=flat-square">
  <img alt="mpv" src="https://img.shields.io/badge/mpv-player-691F69?style=flat-square&logo=mpv&logoColor=white">
</p>

<p>
  <a href="https://github.com/sb-gravity100/nyaa-stream/releases/latest"><strong>Download</strong></a>
  &nbsp;·&nbsp;
  <a href="#features">Features</a>
  &nbsp;·&nbsp;
  <a href="#keyboard-shortcuts">Shortcuts</a>
  &nbsp;·&nbsp;
  <a href="#building-from-source">Build from source</a>
  &nbsp;·&nbsp;
  <a href="#troubleshooting">Troubleshooting</a>
</p>

</div>

Search for a show, pick a release from [nyaa.si](https://nyaa.si), and it starts
playing while the torrent downloads. You don't have to wait for the whole file.
Metadata comes from AniList, with Kitsu as a fallback, and playback runs through
an embedded [mpv](https://mpv.io/) player.

> **Status:** early development (v0.3.x), Windows x64 only.

---

## Contents

- [Features](#features)
- [Installing](#installing)
- [Using the app](#using-the-app)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Settings](#settings)
- [Where your data lives](#where-your-data-lives)
- [Building from source](#building-from-source)
- [Architecture](#architecture)
- [Project layout](#project-layout)
- [Releasing](#releasing)
- [Troubleshooting](#troubleshooting)
- [Known limitations](#known-limitations)
- [Acknowledgements](#acknowledgements)
- [Disclaimer](#disclaimer)

---

## Features

### Discover
- Live anime search with recent-search history
- Show pages with key art, facts, an episode list, per-episode progress and watched toggles
- Home page with a featured hero, **Continue watching**, **New episodes** (airing shows in your library) and your **Library**
- Metadata from AniList GraphQL. When AniList is rate limited or down, the app falls back to Kitsu on its own
- Episode art falls back from Kitsu to AniList, then to a frame captured from the torrent itself

### Find a source
- nyaa.si releases matched to the show, season and episode, with franchise-wide ("absolute") episode numbers mapped back to the season
- Ranking by remembered fansub group, preferred fansubber, preferred resolution and seeders
- A local SQLite database of every release seen, so show pages fill in instantly and still work offline or when nyaa.si rate-limits
- Batch torrents play only the file for the matched episode, and the next episode's file preloads in the background

### Play
- Embedded **libmpv** player with custom controls: subtitle, audio and chapter tracks, subtitle delay, playback speed, frame stepping and skip-opening
- Subtitle styling you control (font, size, colors, outline or box, shadow, position), with an option to restyle ASS subtitles too
- **A-B loop and clip export** to MP4, with subtitles optionally burned in
- Frame capture: save a PNG (`X`) or copy to the clipboard (`Ctrl+C`)
- Picture-in-picture mini window, fullscreen, media keys, autoplay of the next episode, and resume
- Stremio-style buffering indicator and a statistics panel (peers, speeds, codecs, hardware decoding, dropped frames)
- **Fallback player:** when libmpv isn't available, an HLS player built on FFmpeg and hls.js takes over automatically, with JASSUB (libass) subtitles

### Manage
- Cache sizes and one-click clearing (nyaa responses, thumbnails, HLS segments, torrent data)
- Back up and restore your library, progress and settings
- In-app updates from GitHub Releases

---

## Installing

1. Download the latest installer from
   [Releases](https://github.com/sb-gravity100/nyaa-stream/releases/latest).
2. Run it. The app updates itself from then on: see **Settings → Updates**.

**Requirements**
- Windows 10 or 11, x64
- A CPU with AVX2 (x86-64-v3: Intel Haswell / AMD Excavator or newer, roughly 2013+). The build targets it, so older CPUs won't run it
- WebView2 (preinstalled on Windows 11 and current Windows 10)
- For the fallback player only: a GPU with NVENC, QSV or AMF helps a lot with transcoding. Without one it uses OpenH264 on the CPU

---

## Using the app

1. **Search** with the bar at the top and open a show.
2. **Pick an episode.** The best release is chosen for you using your preferences. Open the source menu in the player to switch to another release.
3. **Watch.** Progress saves as you go. Closing the player keeps its last frame as the episode's thumbnail on **Continue watching**.
4. **Save shows** to your library so their new episodes appear on the home page.

Tips:
- Set a **Preferred fansubber** (e.g. `ToonsHub CR`) in Settings. A release matches when its title contains every word you enter.
- Press `A` twice to mark a loop, then `E` to export it as an MP4 clip.
- Press `P` to shrink the window into an always-on-top corner player.

---

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `Space` / `K` | Play / pause |
| `←` `→` / `J` `L` | Seek 5s / 10s |
| `↑` `↓` | Volume |
| `M` | Mute |
| `C` | Subtitles on / off |
| Hold `C` + scroll | Step through subtitle tracks |
| `-` / `=` | Subtitle delay −/+ 0.1s |
| `,` / `.` | Step one frame back / forward |
| `[` / `]` / `\` | Playback speed slower / faster / reset |
| `Shift` | Skip the opening (90s if the file marks no chapter) |
| `N` | Next episode |
| `A` | A-B loop: set A, set B, clear |
| `E` | Export the looped section as MP4 |
| `X` | Save the current frame as PNG |
| `Ctrl` + `C` | Copy the current frame |
| `F` | Fullscreen (anywhere in the app) |
| `P` | Picture-in-picture |

Media keys (play/pause, next, previous) work globally while the player is open.

---

## Settings

Open them with the gear icon in the app bar.

| Section | What you can change |
| --- | --- |
| **Playback** | Resume where you left off, autoplay the next episode, stick with the same fansub group, hide unlisted sources, custom `libmpv-2.dll` path, screenshot folder, preferred fansubber, preferred quality |
| **Subtitles** | On by default, preferred language, full default style with a live preview, restyle ASS subtitles |
| **Storage** | Cache sizes per category and clearing them |
| **Updates** | Check for and install new versions |
| **Backup** | Export or import library, progress and settings as one file |

---

## Where your data lives

| What | Location |
| --- | --- |
| Library, watch progress, settings | WebView `localStorage` (use **Settings → Backup** to move it between machines) |
| Release database (`nyaa.db`) | `%APPDATA%\nyaa-stream\` (permanent, not touched by *Clear cache*) |
| Torrent data, thumbnails, nyaa response cache, HLS segments | `%LOCALAPPDATA%\nyaa-stream\` (safe to clear) |
| Logs (daily, last 7 kept) | `%LOCALAPPDATA%\nyaa-stream\logs\` |
| Screenshots | `Pictures` by default (configurable) |

---

## Building from source

### Prerequisites

| Tool | Notes |
| --- | --- |
| [Rust](https://rustup.rs/) (stable, MSVC toolchain) | |
| [Node.js](https://nodejs.org/) (LTS) + npm | |
| Visual Studio 2022 Build Tools | "Desktop development with C++" workload |
| [LLVM](https://github.com/llvm/llvm-project/releases) | For libclang (bindgen). Expected at `C:\Program Files\LLVM\bin`. Override with `LIBCLANG_PATH` in `.cargo/config.toml` |
| `libmpv-2.dll` | Optional. A GPL build (e.g. [zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild), `mpv-dev-x86_64-*.7z`) placed in `src-tauri/lib/`. Without it the app uses the HLS fallback player |

You don't need vcpkg or CMake. `npm run setup` downloads a prebuilt, checksum-verified archive of static
**libtorrent-rasterbar, OpenSSL and FFmpeg 7.1.2** (about 1.3 GB) into `vcpkg_installed/`. You only need vcpkg to
*rebuild* that archive after changing `vcpkg.json`. See [PLAN.md → Build prerequisites](PLAN.md#build-prerequisites-new-added-with-the-libtorrent-backend).

### Run in development

```bash
npm install
```

```bash
npm run setup
```

```bash
npm run tauri dev
```

`npm run setup` is only needed once, or again after the native-deps version changes.

### Build an installer

```bash
npm run tauri build
```

The installer ends up under `target/release/bundle/`.

### Useful checks

```bash
cargo check --workspace
```

```bash
cargo test --workspace
```

The crates also ship small debug tools that hit live services without launching the app, for example:

```bash
cargo run -p nyaa-client --example search_debug -- "frieren"
```

See [FILE_INDEX.md](FILE_INDEX.md) for the full list (`store_debug`, `offset_debug`, `fallback_debug`, mpv `smoke`, …).

### Optional: version-bump reminder hook

```bash
git config core.hooksPath .githooks
```

---

## Architecture

```
┌──────────────────────── Tauri window ─────────────────────────┐
│  Preact UI (transparent webview: pages, player controls)      │
│        │ invoke / events                                      │
│  ┌─────▼──────────── Rust backend (src-tauri) ─────────────┐  │
│  │  commands · title matching · Kitsu fallback · cache     │  │
│  │                                                         │  │
│  │  anilist-client   kitsu-client   nyaa-client (+SQLite)  │  │
│  │                                                         │  │
│  │  torrent-engine ──► local axum HTTP server              │  │
│  │   (libtorrent)       ├─ raw bytes (Range)  ──► libmpv   │  │
│  │                      └─ HLS (in-process FFmpeg) ► hls.js│  │
│  └─────────────────────────────────────────────────────────┘  │
│  libmpv renders into the window (wid) beneath the webview     │
└───────────────────────────────────────────────────────────────┘
```

- **Shell:** Tauri 2. The window stays opaque. Only the *webview* background is transparent, so mpv's video shows through under the HTML controls.
- **Torrents:** `enginefs`'s libtorrent backend (vendored in `vendor/enginefs`), which gives real per-file piece prioritization so the file you're watching gets the swarm's bandwidth.
- **Streaming server:** a local `axum` server with two layers: raw Range-capable bytes (mpv reads these directly) and an HLS layer (one continuous FFmpeg run per file, stream copy when possible, hardware H.264 transcode otherwise) for the browser fallback.
- **FFmpeg** is linked statically and runs in-process, using **LGPL features only** (no x264).
- **Search:** nyaa.si's HTML results are scraped, since its RSS feed caps at 75 results. All requests go through one shared throttle with 429/503 back-off, and every release lands in a permanent local database.

[PLAN.md](PLAN.md) covers the full design and its reasoning. [PHASES.md](PHASES.md) has the build order.

---

## Project layout

```
src/                  Preact + TypeScript frontend (pages, player, settings)
src-tauri/            Tauri app: commands, embedded mpv, cache/backup, media keys
  lib/                bundled native libs (libmpv-2.dll, gitignored)
crates/
  torrent-engine/     torrent client, streaming server, HLS/FFmpeg pipeline
  nyaa-client/        nyaa.si search, throttle, release database, cache
  anilist-client/     AniList GraphQL metadata
  kitsu-client/       Kitsu metadata + AniList fallback
  mpv-player/         runtime-loaded libmpv: playback, thumbnails, clip encode
vendor/enginefs/      vendored torrent backend (MIT, see VENDORED.md)
scripts/              setup, version bump, installer artwork
```

[FILE_INDEX.md](FILE_INDEX.md) describes every file.

---

## Releasing

Every user-facing change ships with a version bump, because the updater only offers versions newer than the installed one.

1. Bump the version. This keeps `package.json`, `tauri.conf.json` and `src-tauri/Cargo.toml` in sync:
   ```bash
   npm run bump -- patch
   ```
2. Commit `chore: release vX.Y.Z` and tag `vX.Y.Z`.
3. Build with the updater signing key. Never commit this key.
   ```bash
   TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/nyaa-stream.key npm run tauri build
   ```
4. Upload the installer, its `.sig`, and `latest.json` to the GitHub release. The updater reads
   `releases/latest/download/latest.json`.

---

## Troubleshooting

| Problem | Try |
| --- | --- |
| Player looks different or mpv features are missing | libmpv didn't load, so the HLS fallback player is running. Check `src-tauri/lib/libmpv-2.dll` (dev) or set **Settings → libmpv path** |
| Search results are missing or stale | nyaa.si may be rate-limiting. Cached and local-database results are shown in the meantime, and requests retry on their own |
| Stuck buffering | The torrent may have few seeders. Check peers in the statistics panel or pick another source |
| Fallback player stutters on 1080p HEVC/AV1 | No hardware encoder was found, so transcoding runs on the CPU. mpv playback doesn't need to transcode |
| Build fails in `bindgen` / `clang` | Install LLVM, or point `LIBCLANG_PATH` in `.cargo/config.toml` at its `bin` folder |
| Build can't find libtorrent / FFmpeg | Run `npm run setup` |
| Anything else | Check the logs in `%LOCALAPPDATA%\nyaa-stream\logs\` |

---

## Known limitations

- Windows only.
- Library, progress and settings don't sync between machines. Use backup/restore to move them.
- Batches without an episode range in their title can't be matched to individual episodes.
- Removing a torrent doesn't delete its downloaded data right away. Use **Settings → Storage** to clear it.

The full list is in [PLAN.md → Known gaps](PLAN.md#known-gaps--not-yet-implemented).

---

## Acknowledgements

- [Stremio](https://github.com/Stremio): the UX inspiration, plus reference for the buffering indicator and statistics panel
- [stremio-native/stream-server](https://github.com/stremio-native/stream-server): `enginefs`, the torrent backend
- [mpv](https://mpv.io/), [FFmpeg](https://ffmpeg.org/), [libtorrent](https://libtorrent.org/), [hls.js](https://github.com/video-dev/hls.js), [JASSUB](https://github.com/ThaUnknown/jassub)
- [AniList](https://anilist.co/) and [Kitsu](https://kitsu.app/) for metadata
- Bundled fonts: Gandhi Sans, Zen Kaku Gothic New, Outfit

### Licensing note

FFmpeg is built with LGPL features only. The bundled `libmpv-2.dll` is a GPL build, so a distributed build that
includes it is effectively GPL.

---

## Disclaimer

nyaa-stream is a client for public services (AniList, Kitsu, nyaa.si) and the BitTorrent network. It hosts no
content. You are responsible for what you download and stream, and for complying with the laws where you live.
