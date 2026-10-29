# Release policy and tooling

This document preserves release mechanics and dated historical decisions. The
current roadmap and latest SB instructions take precedence over old version
lists and upgrade sequences. v0.6.0 stays a local milestone until SB authorizes
publication. Update/installer feature requirements: [v0.6.0](V0_6_0.md).

## Release plan (decided 2026-09-29)

**v0.6.0 consolidation (SB, 2026-10-29).** All unpublished subtitle
styling/smoothing work formerly labeled v0.5.9 and v0.5.10 ships in v0.6.0.
Those patch versions are not published. Their local milestone tags are retired;
commit history remains intact. Version manifests and release notes now target
v0.6.0, but no v0.6.0 tag is created until its remaining features are complete.
See [the implementation specification](V0_6_0.md) for completed subtitle work
and [the future roadmap](ROADMAP.md) for deferred native supersampling.

**Version branches:** Give each new version its own branch named
`release/<version>` (for example, `release/0.5.7`). Start version-specific
work on that branch so its changes and release commit stay grouped by version.

**v0.5.8 parser replacement (published 2026-10-29; requested 2026-10-28).** Port the supplied
`animebatch.zip` into `crates/release-parse` and make its batch/season-range
rules the app's release parsing path. Expose its season-aware search as a
Rust API; keep the app's existing AniList-aware matcher. Retain the
existing Rust anitopy 2.1.1 port as the anitopy component. Port the guessit
title, season, resolution and source behavior used by animebatch to Rust,
without a Python runtime dependency. Preserve the archive's 42 labeled
regressions; compare app title-matching fixtures and the 10,151-title corpus
before switching behavior. Keep the supplied Python source in the repository
as a reference for parity. Shipped with v0.5.8. SB requested all implemented pre-v0.6.0 work be published as v0.5.8 on 2026-10-29; the signed Windows installers and updater manifest are published at https://github.com/sb-gravity100/nyaa-stream/releases/tag/v0.5.8. Unimplemented v0.6.0 features remain planned.
The 10,151-title comparison showed 1,812 title changes if guessit's shorter
title were made the app's canonical title (including loss of movie and season
qualifiers). Keep the anitopy title in `parse_release`; `parse_batch` exposes
the guessit title and both aliases for clients that need them.
The final corpus diff changes 65 of 10,151 typed releases: 58 kind labels,
20 episode ranges and 2 season lists (some records change multiple fields),
with no canonical-title changes. The app's 39 active title-matching tests
pass, including the fixture at its pre-port 118 wrongly kept Mushoku S1
releases; the first draft's spaced season-range overmatch was removed.

Three published releases: **v0.3.2** (the playback fixes) as soon as it
passes a live test, **v0.4.0** (splash/title bar, download cache, resume
buffer, contact and send logs, and the sbtl torrent engine), then **v1.0.0** (the stable Windows release
after v0.8.1). **v0.5.0 is also published** (decided 2026-10-13: it bundles the
unreleased v0.4.3 season-aware matching and the v0.4.4 AnimeTosho dataset /
Import Tosho Data / Settings modal (the v0.4.5 custom magnet paste moved to
v0.6.0 the same day), so the old 0.4.3-0.4.5 numbers are never released; the old v0.5.0 context-menu
milestone merged into v0.6.0). The milestones in between - v0.6.0 (context
menus + custom magnet paste + build thumbnails + HD banners + seek preview)
→ v0.6.1 (learned release-to-entry matcher) → v0.7.0 (custom Windows libmpv
build) → v0.8.0 (navigation/home rehaul) → v0.8.1 (Discover) - are each
bumped (`npm run bump`) and tagged **locally only**, never pushed. Commits
are pushed with plain `git push` (no tags); `npm run release` pushes only
the tag it publishes. Installs update 0.3.1 → 0.3.2 → 0.4.0 → 1.0.0 (v1.0.0 ships x86_64 and 32-bit
i686 builds, see "32-bit Windows support").
(Changed 2026-09-29 from "only v0.9.0 is published", so the playback fix
doesn't wait on the features; contact/logs moved from v0.9.0 to v0.4.0; a
multiplatform 1.0.0 was planned and dropped the same day - Windows only.)

**sbtl in v0.4.0** (decided 2026-10-04): v0.4.0 also switches the torrent
engine to sbtl and drops libtorrent (see "sbtl backend"). So a regression
can be told apart from the other v0.4.0 features, the live test runs
twice: once on main before the merge (libtorrent - the baseline numbers),
then again after merging `sbtl-backend`. With libtorrent removed there is
no engine to fall back to: an sbtl problem in v0.4.0 is fixed forward in
a published **v0.4.x** patch release (the one exception to the three
releases above).

**v0.4.2** (release parsing, nyaa.si database import, What's new fix) was
published 2026-10-08 on the user's go-ahead, although it isn't an sbtl fix
(an exception to the exception above).

**Local release script** (GitHub Actions is unavailable: the account is
billing-locked, 2026-09-29). `npm run release -- --notes "<text>"`
(`scripts/release.mjs`) does what `release.yml` did, on this PC:

1. Preflight: clean tree, `HEAD` tagged `v<package.json version>`, versions
   in lockstep (`package.json`, `tauri.conf.json`, `Cargo.toml`), `gh`
   logged in, `vcpkg_installed/` present (else `npm run setup`),
   `src-tauri/lib/libmpv-2.dll` present, release not already published.
2. Build: `npm run tauri build` with `TAURI_SIGNING_PRIVATE_KEY` set to the
   *contents* of `~/.tauri/nyaa-stream.key` (the `_PATH` variant isn't read)
   and an empty password; the key is never printed.
3. Collect `nyaa-stream_<v>_x64-setup.exe` + `.sig` (NSIS) and
   `nyaa-stream_<v>_x64_en-US.msi` + `.sig` from `target/release/bundle/`,
   write `latest.json` (`version`, `notes`, `pub_date`,
   `platforms.windows-x86_64.{signature, url}` from the NSIS `.sig` and the
   release download URL) - same shape as v0.3.0's.
4. Publish: push `main` and the one release tag, then
   `gh release create v<v> --verify-tag` with the five files; notes = the
   `--notes` text.

**Line endings** (found publishing v0.4.2, 2026-10-08; tooling, no
version bump). With `core.autocrlf=true`, the build rewrote
`src-tauri/Cargo.toml` with LF and `git status --porcelain` reported it
modified although the content (blob hash) was identical, so the preflight
refused a clean tree. Fixed twice over: `.gitattributes` pins
`* text=auto eol=lf` (`.bat`/`.cmd`/`.ps1` CRLF) and every working copy
was converted to LF, so a tool's LF rewrite is a no-op; and the preflight's
clean-tree check is content-based (`git diff --quiet`, `git diff --cached
--quiet`, untracked files) instead of `git status --porcelain`.

Flags: `--dry-run` (build + stage files, no push/publish; a missing release
tag is only a warning, so the build can be tested before tagging), `--skip-build`
(reuse existing bundle output). `release.yml` keeps only its manual
`workflow_dispatch` trigger, so a pushed tag can't also start a CI publish if
the account is unlocked later.

