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
| v0.6.0 | Subtitle styling/smoothing plus context menus, custom magnets, artwork, thumbnails, seek previews, updates and bespoke installer | Feature code is integrated; Windows lifecycle, live acceptance, Fribb data licensing and final release checks remain open |
| v0.7.0 | Custom libmpv, native subtitle supersampling, memory profiling, batch transitions, preload timing and continuous seek dragging | Planned |
| v0.8.0 | Navigation/Home rehaul and Discover | Planned |
| v1.0.0 | Stabilization and 32-bit Windows | Planned |
| v1.1.0 | Binge Mode | Idea; not implementation-ready |

## v0.6.0 implementation order

SB consolidated the unpublished v0.5.9/v0.5.10 subtitle work into v0.6.0.
There are no separate releases for those versions; existing implementation
and verification remain valid. Tag v0.6.0 only after the milestone is complete.

1. Prove installer/updater packaging, silent upgrade, migration and relaunch.
2. Build shared context menus and clipboard behavior.
3. Add direct magnet play and persistent episode/pack attachments.
4. Add artwork providers, ID mapping, credentials, attribution and cache.
5. Build missing-thumbnail lookup/capture queue and progress UI.
6. Add verified-data-only seekbar frame previews.
7. Build the bespoke installer on the proven backend.
8. Add quiet checks, header Update action and custom in-app update progress.
9. Run integrated Windows acceptance, sync docs and cut the local milestone.

Installer direction confirmed by SB: **compact dark layout matching the app**.
The custom NSIS template and branded install-choice page are integrated; NSIS
still owns progress and completion. The in-app quiet checker, staged download,
resume-buffer save and teardown wait are implemented. A local signed NSIS
artifact was generated, but its install/update lifecycle remains unverified;
keep passive updater mode until isolated Windows checks establish elevation,
existing MSI/NSIS migration, WebView2 bootstrap and relaunch behavior. See
`docs/prototypes/installer/FEASIBILITY.md`. The selected update schedule is
30 seconds after readiness, then every six hours, with 15-minute to 6-hour
failure backoff. First install defaults to `%LOCALAPPDATA%/Programs/nyaa-stream`,
Start menu shortcut on and desktop shortcut off. Provider terms were
rechecked: Simkl is excluded until the app provides Simkl login and sync.
Fribb's mapping data has no stated license; do not publish until resolved.
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
- Preserve authored ASS layout/animation; HLS supersampling is implemented,
  embedded mpv supersampling is deferred to v0.7.0.

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

## Seek-bar thumbnail preview (planned, v0.6.0 - minor: new feature)

See [detailed specification](docs/planning/V0_6_0.md).

## Subtitle preloading (planned, v0.4.0 - patch: fix)

See [detailed specification](docs/planning/PLAYBACK.md).

## Contact and send logs (planned, v0.4.0 - minor: new feature)

See [detailed specification](docs/planning/PLAYBACK.md).

## Load profiler (planned, v0.4.0 - patch: diagnostics)

See [detailed specification](docs/planning/PLAYBACK.md).

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
