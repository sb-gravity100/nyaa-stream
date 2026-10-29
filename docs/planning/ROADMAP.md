# Future milestone specifications

Current version order and scope: [PLAN.md](../../PLAN.md). Detailed task status:
[PHASES.md](../../PHASES.md). Historical target-version notes remain for context.

## v0.7.0 player, memory and playback work

Native subtitle supersampling remains deferred to this milestone by SB.
Render subtitle masks at bounded 1.5x resolution and downsample before
composition; preserve authored layout/animation and video resolution. Verify
GPU compatibility, screenshots/export and rendering cost. Current mpv light
hinting is not supersampling.

**v0.7.0 custom Windows libmpv build (planned 2026-10-29).** Replace the
opaque bundled player build with a reproducible, project-maintained build of
upstream mpv/libmpv and its FFmpeg, libass, and libplacebo dependencies. Pin
each source revision and record build options. Build only the optional mpv
features and dependencies nyaa-stream uses, after auditing its libmpv API,
stream protocols, codecs, ASS/font handling, screenshots, and playback
commands. Disable unused outputs, protocols, encoders, and filters only when
the dependency audit confirms they are unnecessary. This should reduce
binary size and build surface; playback-speed gains must be demonstrated by
benchmarks rather than assumed. The linked `mpv-build` scripts
are Linux-focused; use them as a source-selection/options reference, and base
the Windows build on mpv's documented Windows build workflow or a maintained
Windows cross-toolchain after a small build feasibility spike. Keep the
existing vcpkg FFmpeg build for the HLS pipeline separate unless a later
measured design proves safe to consolidate.

Use PotPlayer only as a source of documented feature ideas, not as code or
binary input. Implement improvements through upstream mpv/FFmpeg/libass/
libplacebo features or independently authored patches: keep ASS antialiasing
and subtitle processing isolated from video scaling so detail is retained;
verify E-AC-3 decoding with known samples; optimize seek latency against the
current DLL using nyaa-stream's seek traces and repeatable playback cases;
optimize data delivery between the torrent streamer and mpv; and confirm
hardware-decoding paths retain a software fallback. Seek and stream work
may tune upstream mpv buffering/read-ahead, retargeting and cancellation
behavior, mpv's range/read pattern, or our stream-reader integration, but
each change must show a measured improvement without stale-seek or resume
regressions. Compare first-byte, first-frame, seek-complete, bytes-requested,
and buffering times on the same torrents. Do not copy
PotPlayer code or infer its private seek implementation. Add build identity
and selected decoder/hwdec context to logs without logging private paths or
credentials. Ship required license notices and corresponding source or
source-offer materials for the exact distributed build. Acceptance compares
old and new DLLs on the same ASS, E-AC-3, seek-latency, and hardware-decoding
cases, and confirms the existing
embedded-player ABI and HLS fallback still work.

**v0.7.0 batch transitions, next-episode preload, and seek dragging
(requested 2026-10-29).**
- Improve next-episode loading from batch sources: reuse the active torrent,
  resolved file list, and verified downloaded data rather than removing and
  re-adding the batch. Resolve the next episode by its season/episode mapping,
  including packs with multiple seasons or extra files. Preserve the selected
  batch where it contains the next episode; use the normal source-selection
  fallback when it does not. Measure next-episode click/autoplay to first frame
  on cached, partially downloaded, and poorly seeded batches.
- Add a next-episode preload timing option: retain the existing spare-bandwidth
  behavior as the default and offer "Only after the current episode is fully
  downloaded". Gate the latter on 100% verified bytes of the current episode's
  file, not whole-batch torrent progress or playback position. If already fully
  downloaded on open, the next file becomes eligible immediately. Keep preload
  lower priority than active playback, resolve only the next episode, and
  cancel or retarget it on episode/source changes, disabled preloading, or exit.
  Verify the next file receives no preload requests before the threshold and
  that existing downloaded data can be reused after switching episodes.
- Make the seekbar follow dragging continuously: thumb, elapsed-time readout,
  and played portion follow the pointer immediately, without being snapped
  back by playback progress events. Retain pointer capture outside the bar,
  clamp to the playable duration, coalesce intermediate seek requests, and
  commit the final target on release. Handle cancel, source changes and unknown
  duration cleanly; preserve paused state and keyboard seeking. Verify both
  embedded mpv and HLS, including fullscreen, backward dragging, and targets
  not yet downloaded; stale seek completions must not override the latest drag.

The `msedgewebview2.DMP` snapshot reviewed 2026-10-29 contains about 2.56 GiB
of captured memory ranges and about 2.08 GiB of private committed virtual
memory. This is not a working-set measurement and does not attribute memory
to JavaScript objects or a particular subsystem; the dump has no V8 heap
snapshot stream. **Profile first:** add opt-in, bounded memory sampling for
the nyaa-stream host and WebView2 child processes, labeled by supported
process kind and PID. Record working set and private bytes with app state
(home, media page, player, minimized); avoid page contents and user paths.
Use matching WebView2 symbols plus DevTools heap snapshots and repeatable
idle/library/player runs to identify likely owners before treating this as a
leak. Do not change cache or suspension policies until profiles support it.
Then evaluate one shared WebView2 environment/control where the Tauri window
model allows it, bounded frontend image/thumbnail caches and released object
URLs, and WebView2's low-memory target or suspend/resume for inactive UI
states. Keep the renderer active while its controls are needed for playback.
Do not share the user-data folder with unrelated applications.

For process branding, label WebView2 processes by their supported process
kind in nyaa-stream diagnostics and apply the custom icon to the nyaa-stream
host/window. The WebView2 runtime owns its child executable identity; do not
rename or patch `msedgewebview2.exe` or its icon. Verify any Task Manager
grouping available through supported Windows app identity settings.

The PotPlayer installer review exposed no source-level evidence of its
memory-management strategy. Use its visible behavior only as a comparison;
memory changes must be independently implemented and justified by this
dump's follow-up profiling.

## 32-bit Windows support (planned, v1.0.0 - minor: new target)

Assumes "32-bit" = Windows x86 (`i686-pc-windows-msvc`); tell me if Linux
or ARM was meant. The stable v1.0.0 ships both `windows-x86_64` and
`windows-i686` builds. Work starts after v0.8.0 (features stay 64-bit-tested
until then, but must not add 64-bit-only assumptions).

- **Toolchain**: add the `i686-pc-windows-msvc` Rust target; vcpkg triplet
  `x86-windows-static-md` for the LGPL FFmpeg; sbtl's C sources built for
  i686 (`clang-cl -m32` / x86 `cl.exe`); LLVM libclang must be the
  x86-capable one for bindgen.
- **libmpv**: bundle a 32-bit `libmpv-2.dll` in `src-tauri/lib/x86/` (64-bit
  stays in `lib/x64/`); the loader picks by `cfg(target_pointer_width)`.
  `wid` embedding is unchanged.
- **Memory limits**: a 32-bit process has ~2 GB (up to 4 GB large-address
  aware) of address space, so the 64-bit tuning must be scaled: mpv
  `--demuxer-max-bytes`/`max-back-bytes` (150 MiB), sbtl's read-ahead
  window, the resume buffers and sbtl's cache use per-arch caps;
  mmap of multi-GB files is avoided (files are read in ranges, and file
  sizes/offsets are `u64` end to end - audit `usize` casts).
- **Packaging/updater**: separate NSIS/MSI per arch (`--target
  i686-pc-windows-msvc`); `latest.json` gets a `windows-i686` platform entry
  with its own signature; `scripts/release.mjs` builds and uploads both.
  The in-app updater picks its own arch automatically.
- **Verify**: 32-bit Windows VM or the WoW64 build of the 32-bit installer on
  this PC (installs and runs on 64-bit Windows, so live QA is possible
  here): playback, subtitles, resume, HLS fallback, updater. WebView2 is
  available for x86.
- Risks: 32-bit builds of vcpkg FFmpeg or sbtl may need patches; memory
  exhaustion on long HLS runs; small user base vs. extra build/QA cost.

## Navigation and home rehaul + Discover (planned, v0.8.0 navigation, Home and Discover)

Scope consolidated by SB on 2026-10-29: **navigation, Home rehaul and
Discover ship together in v0.8.0**. The media page, player and settings
keep their layouts while inheriting the navigation chrome and theme. The consolidated v0.8.0 rehaul/Discover milestone is a local-only tag, so v1.0.0 remains the
first published build with the new UI (see "Release plan").

**v0.8.0 - navigation and home rehaul**
- Persistent left rail (icons + labels, collapses on narrow windows) replacing
  the current top-of-page navigation: Home, Discover, Library, Search,
  Settings. Sits under the custom title bar from v0.4.0; hidden in the
  player. Routes stay hash-based (`router.ts`); add `#/discover`,
  `#/discover/browse`, `#/library`.
- Home keeps the hero, Continue watching and New episodes, restyled to the
  rail layout; the Library poster grid moves to its own Library page
  (Home shows a short "Your library" row linking to it).
- Search moves into the rail (also a global shortcut), results unchanged.
- No new backend; frontend-only, verified in the app and the browser preview.

**v0.8.0 - Discover** (`src/DiscoverPage.tsx`, new `anilist-client`
queries, cached per session + short disk TTL; Kitsu fallback where the
metadata fallback already exists)
- **Seasonal and trending**: rows for Trending now, This season, Next
  season (upcoming) and All-time popular; each row scrolls and opens the
  media page.
- **Browse with filters**: full page with genre, year, season, format,
  status filters and sort (popularity, score, trending, newest), paginated
  infinite scroll; filter state kept in the URL.
- **Recommendations**: "Because you watched X" rows from AniList
  recommendations for titles in the library/watch history; empty state
  when there is no history.
- **Airing schedule**: weekly calendar (local time) from AniList
  `airingSchedules`, library/watching shows highlighted, day and "now"
  jump; clicking opens the media page.
- Context menus (v0.6.0) apply to Discover cards; adult content follows the
  existing setting (default hidden).
- Risks: AniList rate limit (90/min) - batch the rows into few queries and
  cache; recommendations depend on the user's history size.

## Binge Mode (idea, target v1.1.0 - minor: new feature; after v1.0.0)

Idea recorded 2026-10-08; **rough sketch only, no phases yet** (user chose
PLAN.md-only for now). Whole-series queue: pick a show once and the app
builds an ordered episode queue and plays it through without the user
choosing a release per episode.

- **Entry point (decided 2026-10-08)**: a "Binge" button in the app header
  (title bar / top chrome, visible on every page except the player) that
  opens a **setup modal**. The media-page button and context-menu item
  (v0.6.0) are secondary shortcuts that open the same modal with the show
  preselected.
- **Setup modal** (`src/BingeModal.tsx`), a short wizard:
  1. *Pick series*: search box (AniList, same search as the app) with
     results as poster rows; one or more series can be added (a marathon of
     several shows is allowed, in the order chosen, reorderable).
  2. *Compile*: for each picked series the app works out what needs to be
     watched and shows it as a checklist: all seasons/cours of the franchise
     in watch order (AniList relations: prequel/sequel chain, TV entries
     first; movies, OVAs and specials listed but unchecked by default,
     toggleable), with episode counts, already-watched episodes marked and
     skipped by default, and per-episode source status (found / batch /
     missing) from the matcher.
  3. *Options*: the customizable settings below, preloaded from the global
     defaults or a preset, overridable here for this binge only.
  4. *Start*: creates the queue and starts playing from the first unwatched
     episode (or the chosen start point).
  The modal can be closed and reopened without losing the draft; a running
  binge shows in the header button (progress badge) and reopening shows the
  queue panel instead of the wizard.
- **Queue building**: reuse the season-aware matcher (`ShowMatcher`,
  `EpisodeLabel`, season sets) to list the entry's episodes from the release
  store/nyaa results. Per episode choose one release by preference: same
  group and quality as the previous episode, then seeders; a season batch
  torrent counts as one source for all its episodes (play files from it
  rather than switching torrents). Selection is stored per queue so it stays
  stable across sessions.
- **Playback**: when an episode ends (or at the outro), the next one starts
  automatically with a short cancellable "Next: Ep N" overlay; the queue
  continues across seasons only if the user opts in (sequel entries via
  AniList relations). Queue state (current index, chosen releases) persists
  in the app database so closing the app resumes mid-queue.
- **Look-ahead**: while episode N plays, warm episode N+1 using the existing
  prefetch/resume-buffer machinery (download cache, "Fast playback start");
  look-ahead depth is a setting and respects the cache size cap.
- **Queue UI**: a panel in the player and on the media page showing upcoming
  episodes with the chosen release; reorder/skip/swap release per episode;
  episodes with no source are shown greyed and skipped.
- **Customizable (decided 2026-10-08)**: nothing about Binge Mode is
  hardcoded; every behaviour is a setting, with a global default in
  Settings → Binge Mode and a per-show override saved with the queue
  (set from the Binge button's options popover):
  - Auto-advance: on / ask each time / off; overlay countdown length.
  - Skip intro/outro: off / auto-skip / show a skip button. OP and ED are
    separate switches, plus these opt-in exceptions (all optional, all off
    by default):
    - *Keep first OP*: never skip the OP of the series' first episode.
    - *Keep last ED*: never skip the ED of the series' last episode.
    - *Season-aware*: on seasoned series, play the OP the first time it is
      seen in a season (episode 1 of that season) and play the ED only at
      the very end of the season; skip them everywhere else.
    - *OP/ED change tracking* (WIP, best-effort): long-running series swap
      their OP/ED every cour. Each distinct OP/ED is treated as its own
      "theme" so "first time" / "last time" apply per theme, not per series:
      the first episode using a new OP plays it, the rest skip it, and the
      last episode of a theme's run can keep its ED. A theme is identified
      by chapter titles (`OP`, `ED`, `Opening 2`...) when the file has them,
      else by a repeated-segment fingerprint across the queue's episodes
      (audio/video match of the same span in consecutive episodes). It is
      not deterministic yet: when detection is unsure it falls back to
      "don't skip" and never cuts story content. No external skip-time
      service.
  - Release choice: prefer same group / same quality / most seeders / smallest
    size; preferred groups, resolution, audio and sub language lists.
  - Season batches: allow or avoid them as a source.
  - Cross-season continuation: off / ask at season end / automatic.
  - Missing episode or dead torrent: skip silently / pause and ask / try the
    next-best release after a timeout (timeout configurable).
  - Look-ahead depth (0 = off), capped by the cache size setting.
  - Start point: from episode 1 / resume point / chosen episode.
  - Per-episode overrides in the queue panel beat all of the above.
  Defaults are conservative (auto-advance on, same group/quality, no
  cross-season, ask on missing episode, look-ahead 1).

Decided 2026-10-08: Binge Mode is an **explicit mode** (the existing "next
episode" behaviour stays as is, untouched); settings are saved as **named
presets** (e.g. "Dub marathon"), possibly - a later stage, not blocking the
first release of Binge Mode.

**OP/ED detection is deferred (2026-10-08)**: sources label chapters
inconsistently (`OP`, `Opening`, `Intro`, `Chapter 2`, language variants,
none at all), so the whole skip/keep/theme-tracking block above is parked
until a later stage; label normalisation and fingerprinting get designed
then. The first Binge Mode release ships the header button, setup modal,
queue, auto-advance, release choice and look-ahead without skipping.

**Watch order (decided 2026-10-08)**: the compile step uses AniList
prequel/sequel relations only: walk back to the root prequel, then forward,
TV entries in that order and checked. Movies, OVAs, specials and recaps are
listed at the end, unchecked, and the user can drag to reorder anything. No
auto-placement of specials in broadcast order (needs a curated source; a
wrong guess is worse than a visible "unchecked at the end"). To verify before
writing phases: whether the Fribb `anime_lists` TVDB season/episode mapping
orders split-cour shows better than AniList alone.

Open questions (to decide before this becomes phases):
1. Multi-series marathons: keep, or one series per binge for the first
   release?
2. Does a preset also hold per-show overrides, or only the global defaults?

Risks: inconsistent release groups mid-series (quality/audio jumps), torrents
with no seeders stalling the queue (needs a timeout then fall back to the
next-best release), look-ahead bandwidth/disk use.

