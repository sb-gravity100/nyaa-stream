# PLAN.md — nyaa-stream

Anime-focused Tauri 2 desktop client: Preact/TypeScript UI, Rust backend,
sbtl torrent streaming, embedded libmpv playback and an HLS fallback.

## Start here

- [v0.6.0 implementation lineup](docs/planning/V0_6_0.md): scope, order,
  behavior, proposed defaults, acceptance checks and remaining technical decisions.
- [PHASES.md](PHASES.md): task completion and detailed checklists.
- [FILE_INDEX.md](FILE_INDEX.md): exact files and responsibilities.
- [AGENTS.md](AGENTS.md): session, commit and release rules.

## Active roadmap

| Version | Scope | State |
|---|---|---|
| v0.5.8 | Implemented pre-v0.6.0 work | Published |
| v0.6.0 | Subtitle styling/smoothing plus context menus, custom magnets, artwork, thumbnails, seek previews, updates and bespoke installer | Published 2026-10-30; SB waived remaining Windows acceptance and deferred Fribb licensing; live-swarm and manual magnet acceptance remain open |
| v0.7.0 | Custom libmpv and direct sbtl streaming first, then subtitle rendering, memory profiling, batch transitions, preload timing, seek dragging and player/settings accessibility | P0 direct `sbtl://` stream callback is implemented and pinned to published sbtl 0.2.0; a cleared-cache A/B is promising but not conclusive; indexed-bitrate read-ahead, speed-scaled window, app-side seek prefetch and custom mpv sbtl buffer/cache-pause/idle hooks are implemented; playback/mpv logging captures debug messages and lifecycle events, redacts command data, and creates a timestamped file per run; broader runtime measurements remain pending; batch torrent/file reuse and season-aware handoff implemented |
| v0.8.0 | Navigation/Home rehaul and Discover | Planned |
| v1.0.0 | Stabilization and 32-bit Windows | Planned |
| v1.1.0 | Binge Mode | Idea; not implementation-ready |

## v0.6.0 implementation order

SB consolidated the unpublished v0.5.9/v0.5.10 subtitle work into v0.6.0.
There are no separate releases for those versions; existing implementation
and verification remain valid. SB authorized v0.6.0 publication with the
remaining Windows acceptance and Fribb license checks waived/deferred on
2026-10-30; those checks were not passed or resolved.

1. Prove installer/updater packaging, silent upgrade, migration and relaunch.
2. Build shared context menus and clipboard behavior.
3. Add direct magnet play and persistent episode/pack attachments.
4. Add artwork providers, ID mapping, credentials, attribution and cache.
5. Persist per-anime Nyaa release results with an explicit refresh action; build the missing-thumbnail torrent-capture queue and progress UI without fetching release-description images.
6. Add verified-data-only seekbar frame previews.
7. Build the bespoke installer on the proven backend.
8. Add quiet checks, header Update action and custom in-app update progress.
9. Run integrated Windows acceptance, sync docs and cut the local milestone.

Installer direction confirmed by SB: **compact dark layout matching the app**.
The custom NSIS template and branded install-choice page are integrated; NSIS
still owns progress and completion. The in-app quiet checker, staged download,
resume-buffer save and teardown wait are implemented. A local signed NSIS
signed NSIS/MSI artifacts were generated, and the Win10Lite installer matrix
passed NSIS→NSIS, MSI→MSI and MSI→NSIS upgrades (17/17 checks each). Both current
installers launch with the bundled app-local `VCRUNTIME140.dll`; upgrades retain
the release database and user-data marker, and uninstall retains user data.
The test records are in `docs/acceptance/v0.6.0/`. The v0.5.8-based updater
acceptance is not applicable because v0.5.8 has no in-app updater; the installer
upgrade matrix is the migration check. SB waived the remaining Windows lifecycle
acceptance for v0.6.0 on 2026-10-30; signed apply/relaunch, UAC, clean-machine
WebView2 bootstrap and repair/registration remain unverified. See
`docs/prototypes/installer/FEASIBILITY.md`. The selected update schedule is
30 seconds after readiness, then every six hours, with 15-minute to 6-hour
failure backoff. First install defaults to `%LOCALAPPDATA%/Programs/nyaa-stream`,
Start menu shortcut on and desktop shortcut off. Provider terms were
rechecked: Simkl is excluded until the app provides Simkl login and sync.
Fribb's mapping data has no stated license. SB deferred resolving it for the
v0.6.0 publication on 2026-10-30; this does not establish a license.
Continuous drag behavior, batch transition work and native supersampling remain
v0.7.0.

## Architecture constraints

- Keep embedded libmpv under a transparent webview; never make the native
  window transparent. HLS remains the missing-libmpv fallback.
- sbtl must resolve from a published git tag; native DLL identity must match.
  No committed local patch overrides.
- Preserve permanent release data, source attachments and playback/resume state.
- Keep HLS FFmpeg separate from future custom-libmpv dependencies unless a
  measured design supports consolidation.
- Preserve authored ASS layout/animation; HLS supersampling and the v0.7.0
  embedded mpv 2× rendering path are implemented.

## Release and documentation rules

- Implement each version on `release/<version>`; use the lockstep bump script.
- v0.6.0 and intermediate milestones remain locally tagged unless SB explicitly
  authorizes publication. Never push all tags.
- See [release policy/tooling](docs/planning/RELEASING.md) for signing, build,
  dry-run, installer assets and publishing. Dated historical release lists do
  not override current decisions.
- Keep this file concise. Put detailed technical specifications in the matching
  document below; update PHASES.md and FILE_INDEX.md when scope or files change.
- Existing references to a PLAN.md section resolve through the specification
  index below. Original section headings are retained for compatibility.

## Planning documents

| Document | Contents |
|---|---|
| [v0.6.0 specification](docs/planning/V0_6_0.md) | Complete feature lineup and acceptance criteria |
| [Architecture](docs/planning/ARCHITECTURE.md) | Stack, build prerequisites, data flow, torrent engines and development notes |
| [Playback](docs/planning/PLAYBACK.md) | Playback, cache/resume, subtitles, desktop UI and historical measurements |
| [Matching](docs/planning/MATCHING.md) | Matching, AnimeTosho and rule-based search debugger |
| [Future milestones](docs/planning/ROADMAP.md) | v0.7.0 player/memory/playback, navigation, Discover, x86 and Binge Mode |
| [Custom libmpv build](docs/planning/MPV_BUILD.md) | Native LLVM recipe, proposed dependency pins, setup stages and acceptance gates |
| [Direct sbtl/mpv streaming](docs/planning/MPV_SBTL_FIX_0.7.0.md) | v0.7.0 design and phased work to replace loopback HTTP Range delivery with direct sbtl streams in libmpv |
| [Releasing](docs/planning/RELEASING.md) | Release tooling/policy and dated release history |

<details>
<summary>Specification index — original PLAN.md section names</summary>

## Reference

See [detailed specification](docs/planning/ARCHITECTURE.md).

## Stack

See [detailed specification](docs/planning/ARCHITECTURE.md).

## Workspace layout

See [detailed specification](docs/planning/ARCHITECTURE.md).

## Data flow (search → browse → play)

See [detailed specification](docs/planning/ARCHITECTURE.md).

## Context menus (planned, v0.6.0 - minor: new feature; was v0.5.0, moved 2026-10-13)

See [detailed specification](docs/planning/V0_6_0.md).

## v0.6.0 implementation lineup (2026-10-29)

See [detailed specification](docs/planning/V0_6_0.md).

## Release plan (decided 2026-09-29)

See [detailed specification](docs/planning/RELEASING.md).

## Fast playback start (v0.3.2 - patch: fix)

See [detailed specification](docs/planning/PLAYBACK.md).

## Native splash window and custom title bar (planned, v0.4.0 - minor: new feature)

See [detailed specification](docs/planning/PLAYBACK.md).

## Download cache (planned, v0.4.0 - minor: new feature)

See [detailed specification](docs/planning/PLAYBACK.md).

## Continue-watching resume buffer (planned, v0.4.0 - minor: new feature)

See [detailed specification](docs/planning/PLAYBACK.md).

## Build thumbnails button (planned, v0.6.0 - minor: new feature)

See [detailed specification](docs/planning/V0_6_0.md).

## HD banner sources (planned, v0.6.0 - minor: new feature)

See [detailed specification](docs/planning/V0_6_0.md).
Prefer season-tagged Fanart.tv logos; otherwise use an eligible logo uploaded within 180 days of the anime season start, then the default logo.

## Seek-bar thumbnail preview (planned, v0.6.0 - minor: new feature)

See [detailed specification](docs/planning/V0_6_0.md).

## Subtitle preloading (planned, v0.4.0 - patch: fix)

See [detailed specification](docs/planning/PLAYBACK.md).

## Contact and send logs (planned, v0.4.0 - minor: new feature)

See [detailed specification](docs/planning/PLAYBACK.md).

## Load profiler (planned, v0.4.0 - patch: diagnostics)

See [detailed specification](docs/planning/PLAYBACK.md).

## Playback and mpv logging coverage (implemented, v0.7.0 patch: diagnostics)

Problem: playback traces span the frontend, Tauri commands, torrent readers and
libmpv, while mpv previously forwarded only warning-level messages. This made
start, seek, cancellation, fallback and decode failures hard to connect.

Implemented: log player command categories and outcomes, stream callback
lifecycle/read/seek totals, libmpv debug messages and event summaries, and
frontend mpv event/error branches. Redact sensitive command values and media
URLs; suppress only identical repeated mpv log lines while retaining a count.
Each app run starts a new UTC timestamped log file, with the seven most recent
run files retained. This is a diagnostics patch within v0.7.0, with no feature
version change. `cargo check --workspace` and `npm run build` pass.

## Direct sbtl buffer control in mpv (implemented, v0.7.0 patch: playback)

Problem: mpv previously saw direct sbtl reads only as a blocking custom stream,
so its cache-pause decisions and the player UI cannot use sbtl's verified
playback-time buffer as their source of truth. Pausing also leaves sbtl's
read-ahead policy unchanged.

Implemented: add a locked custom-libmpv stream interface that exposes sbtl buffer
snapshots without linking mpv to sbtl or consuming sbtl's event queue. Drive
`sbtl-buffer` and cache pause from the same readiness thresholds; forward
playback pause changes to the active sbtl stream while keeping the app as the
single event consumer. Retain the public callback/HTTP paths as fallbacks. The
patched candidate was rebuilt and staged locally; live app verification remains
open.

## 32-bit Windows support (planned, v1.0.0 - minor: new target)

See [detailed specification](docs/planning/ROADMAP.md).

## Navigation and home rehaul + Discover (planned, v0.8.0 navigation, Home and Discover)

See [detailed specification](docs/planning/ROADMAP.md).

## Binge Mode (idea, target v1.1.0 - minor: new feature; after v1.0.0)

See [detailed specification](docs/planning/ROADMAP.md).

## Dev build speed (tooling, no version bump)

See [detailed specification](docs/planning/ARCHITECTURE.md).

## sbtl backend (the torrent engine; default from v0.4.0)

See [detailed specification](docs/planning/ARCHITECTURE.md).

## sbtl_engine (v0.4.0 - replaced the vendored enginefs)

See [detailed specification](docs/planning/ARCHITECTURE.md).

## Season-aware source matching (v0.4.2 parsing + db import; matching ships in v0.5.0, was v0.4.3 - patch: fix)

See [detailed specification](docs/planning/MATCHING.md).

Remote Nyaa queries stay within 600 characters; long first-season sibling-exclusion lists are trimmed because local season matching remains authoritative.

## Batch file selection from torrent manifests (planned, v0.7.0 patch: playback)

Use the same AniList franchise episode offset for file labels inside a torrent
that source grouping already uses for release titles. Do not let fractional
labels such as `24.5` silently match integer Episode 24; keep NC-version
alternates out of automatic episode selection. If a manifest still cannot be
matched safely, preserve ranked-source fallback. Validate against the real
file lists in `nyaa-crawl-30pages_batches.json` and the Asakura Slime season
packs.

## AnimeTosho dataset (v0.5.0, was v0.4.4 - minor feature)

See [detailed specification](docs/planning/MATCHING.md).

## Search flow debugger (developer tool, local only)

See [detailed specification](docs/planning/MATCHING.md).

## Custom magnet in the search bar (planned, v0.6.0 - minor feature; was v0.4.5, held out of v0.5.0 on 2026-10-13)

See [detailed specification](docs/planning/V0_6_0.md).

## Settings as a centered modal (v0.5.0, was v0.4.4 - UI change, ships with the dataset work)

See [detailed specification](docs/planning/PLAYBACK.md).

## Known gaps / not yet implemented

See [detailed specification](docs/planning/ARCHITECTURE.md).

</details>
