# Playback and desktop feature reference

Detailed implementation notes and original feature plans, including historical
baselines. Old "planned" headings identify the original target, not a current
completion claim. Use [PHASES.md](../../PHASES.md) for task status.

## Fast playback start (v0.3.2 - patch: fix)

**Problem (measured 2026-09-29, SubsPlease 1080p MKV, 256 KB pieces):**
metadata took 1s and mpv issued `loadfile` ~15ms later, but piece 0 took
**20s** to verify while 342 other pieces (~90 MB) arrived first, then pieces
5-7 each stalled 5-10s - by the time the reader was 1.8 MB in, 96% of the
file was done. The speed in the statistics was real, just spent on the wrong
pieces. Causes, in the vendored `enginefs`:

1. `disk_backed_file_baseline_priority` puts the whole playing file at
   priority 1 from the first request, so libtorrent fills every peer's request
   queue with rarest-first bulk pieces (`max outstanding piece requests
   reached`). Priority 7 + deadlines only reorder *future* requests; the head
   piece queues behind seconds of in-flight bulk work on every peer.
2. The startup window is tiny: `INITIAL_FIRST_BYTE_WINDOW_PIECES = 3`, and
   after the first byte `prioritize_from` gives only the current piece
   priority 7 (read-ahead 4). mpv needs several MB before it plays, and every
   piece became urgent only when the reader hit it, paying the queue delay
   again each time.

**Fix (vendored `enginefs`, each change noted in `VENDORED.md`; C covers
first watch and continue watch, A/B cover everything else - seeks, and the
first seconds before C's mode is known):**

- **A - startup baseline 0:** foreground streaming intents start the playing
  file at baseline priority 0 (only the priority window is wanted) and raise
  it to 1 once the startup buffer (the first ~8 MB from the read position) has
  verified, or after a 15s fallback. Baseline 1 is still reached, so the
  torrent never reports `is_finished` early - the reason upstream chose 1
  (see the comment at `priorities.rs`). Seeks keep baseline 1 once raised.
- **B - wider startup window:** the pre-first-byte window becomes ~4 MB
  (16 pieces at 256 KB) at priority 7 with staggered deadlines, and the
  post-first-byte read-ahead keeps the next ~4 MB at 7 instead of just the
  current piece, so mpv's startup prefill downloads in parallel.
- **C - sequential download, two cases.** `play_magnet` takes a `watch`
  hint from the frontend (`first` when there's no saved progress, `resume`
  when Continue watching / Resume passes a start time) and hands it to the
  engine per torrent file - the engine can't tell on its own, since mpv
  always reads the header at offset 0 first and only then seeks to `start`.
  - **First watch:** sequential download (`set_sequential_download(true)`)
    from piece 0 with the file at baseline 1, so it fills in playback order.
  - **Continue watch:** the first seek after the header read marks the
    resume point. Pieces before it drop to priority 0 (the header pieces
    mpv already read stay), everything from it on is baseline 1, and
    sequential mode is on - so in-order download starts at the resume point,
    not piece 0. The skipped earlier pieces go back to 1 once the rest of the
    file has verified, or immediately when the player seeks back into them.
  - *Hint delivery (decided while implementing):* the frontend picks the
    file (a batch's episode) only after `play_magnet` returns, so the hint
    is stored per torrent (`TorrentEngine::set_watch_hint`) and taken by
    the next foreground stream of a file that isn't already streaming - a
    different file, or the same file reopened with no stream left. The
    previous episode still streaming never takes the next one's hint.
  - Both: libtorrent ranks piece priority above sequential order, so the
    priority-7 read window and pinned container metadata (MKV Cues / MP4
    moov) still jump the queue. Sequential mode never applies to
    background / probe / preload reads. A's baseline-0 phase is skipped
    while sequential mode is on. Upstream turns sequential off everywhere;
    this is our change.
  - **Seek into undownloaded data = continue watch.** A seek (forward or
    back) whose target piece isn't verified, and that the downloaded run
    around the old position can't cover, re-anchors continue-watch mode at
    the target: missing pieces before it drop to 0, sequential order
    restarts there. A seek landing inside already-downloaded data changes
    nothing. Debounced - the re-anchor applies once the reader has blocked
    on the target piece for ~100ms, so scrubbing through the seek bar (the
    log shows 4 seeks in 1s) doesn't thrash libtorrent's priorities; each
    re-anchor replaces the previous one, and dropped pieces come back to 1
    as in continue watch.
  - Trade-off: rarest-first protects swarm health; for one viewer streaming
    in order, order matters more.
- Log line fix: the "waiting for verified piece" diagnostic reports the window
  for the effective intent (it printed the original intent's window).
- *Implementation decisions (v0.3.2):*
  - A holds the rest of the file at 0 through **piece** priorities; the file
    priority stays 1, since a 0 file priority makes libtorrent write that
    file's pieces into its part file, away from the disk reader. Streams
    report their first read position so the 8 MB buffer follows mpv's seek.
  - Our raw-stream server opens every mpv request as a DirectInitial reader
    at offset 0 and then seeks, so the engine learns positions from the
    readers themselves (`ReadStarted`/`SeekBlocked`), not from the intent.
    Tail reads (the MKV Cues / MP4 moov region) are never positions.
  - Continue watch anchors at the first read starting past the 8 MB header
    region. A seek back into skipped pieces restores them only when the
    target piece is missing; a re-anchor needs an established mode (first
    watch, or continue watch after its anchor) and a target more than ~4 MB
    past the downloaded run from the current anchor.

**Warm mpv:** mpv already persists after first use (`mpv_stop` only sends
`stop`, dropping the file and its demuxer buffers). Changes:

- Spawn the embedded mpv early (idle, below the webview) instead of on the
  first play, so the first episode skips libmpv load + font install.
  It starts 1.5s after the page's first load finishes (`on_page_load`,
  once). (It was first moved there on the suspicion that spawning at launch
  delayed the page - wrong: the next launch was still 33s late. The real
  cause was dev-only: Vite listened on IPv6 `::1` only while the webview's
  IPv4 attempts to `localhost:1420` took ~2s each to fail; Vite and
  `devUrl` now both use `127.0.0.1`.)
  Skipped when libmpv isn't found at launch (HLS fallback, or `mpv_start`
  spawns lazily after a Settings path override).
- `mpv_stop` resets per-file state after `stop` - `ab-loop-a/b`, `speed`,
  `sub-delay`, `audio-delay`, `pause` - so nothing leaks into the next file.

**Stale "can't be played" error on source switch** (log 2026-09-29
09:01:06-09:01:31, Fate/strange Fake 02, sources switched Kaleido → SubsPlease
→ Erai-raws → SubsPlease): the error belongs to the source being left, not
the one that then plays. Switching calls `play_magnet`, which removes the old
torrent while mpv is still opening it; mpv's pending HTTP read gets
`stream request for unknown torrent`, the open fails (`loading failed` /
`unrecognized file format`) and its `end-file` arrives ~1s later - after the
switch began - and `MpvVideo` treats any `end-file` error as the current
file's (`src/mpvVideo.ts` `handleEvent`). Fix:

- `libmpv.rs`: `EventEndFile` gains mpv's `playlist_entry_id` (the real
  `mpv_event_end_file` layout: reason, error, playlist_entry_id, ...), and
  `start-file` events carry theirs, both forwarded in `mpv-event`.
- `MpvVideo.load` records the entry id of the `start-file` that follows its
  `loadfile`; `end-file` for any other id is logged and ignored. (It takes
  the id `loadfile` itself returns when libmpv reports one; ids only grow,
  so an `end-file` arriving before the new id is known is stale when its
  id is at most the highest one seen before the load.)
- **Source switch = flush + seek back.** Picking another source (source
  menu, `setSelectedRelease`) first captures the playhead
  (`video.currentTime` once playback has started; otherwise the pending
  `resumeAtRef` stays), then sends mpv `stop` at once - dropping the old
  file and its demuxer/cache buffers so nothing of the old source keeps
  playing or rendering - and only then runs `play_magnet`. The new file loads
  with `start` = that captured time and the `resume` watch hint, so C's
  continue-watch mode downloads in order from there. Today `resumeAtRef` is
  captured once per episode, so a switch mid-episode restarts at the
  original resume point (or 0:00). The old torrent's open is cancelled by
  the `stop` instead of failing, which also removes the stale error at the
  source. Caveat: releases of one episode can differ by a few seconds
  (different cuts/intros); the seek lands on the same timestamp, not the
  same frame.

Also in that log: the Kaleido-subs source never started - `Libtorrent file
priorities did not match after acknowledgement timeout` at load, then
`stream request for unknown file index` 4s later, and mpv waited 10s with no
error until the switch. Investigate the ack-timeout path in the vendored
`apply_file_priorities`: a timeout should retry or fall through to streaming,
not leave the file unknown; if it still fails, surface the error at once.
*Done:* the update is resubmitted once, then streaming proceeds
(`file_priority_unconfirmed` warning with the mismatched file indices);
real errors (file error alert, torrent gone) still fail at once, and
`get_file_reader` failures are now logged with their cause instead of only
the stream server's "unknown file index".

**Seek-bar buffer indicators** (reported 2026-09-29: ranges vanish after a
seek). The mpv player's bar only shows mpv's demuxer cache
(`demuxer-cache-state.seekable-ranges`, `src/mpvVideo.ts` `buffered`); mpv
evicts old ranges once its back-buffer limit (default 50 MiB) is hit, so
after a jump the earlier buffered stretch disappears even though the torrent
still has those bytes on disk. `readyRanges` in `StreamStats` is HLS-only
(transcoded segments) and is empty for mpv. Fix - two layers:

- **Downloaded (dim):** `StreamStats` gains `downloaded_ranges` (seconds): the
  file's verified pieces merged into byte runs, mapped to time with the
  container's keyframe index (MKV Cues / MP4 index, read once per file via
  the in-process FFmpeg probe and cached next to `MediaProbes`), falling back
  to a linear bytes/duration estimate until the index is known. Runs shorter
  than ~1s of playback are merged into their neighbours so the bar isn't
  speckled. The player already polls `get_stream_stats`; the bar draws these
  under the buffer layer.
- **Buffer (bright):** mpv keeps more of what was played seekable:
  `--demuxer-max-back-bytes=150MiB` (from 50 MiB default; forward stays
  150 MiB) so a jump back into just-played video stays instant and its range
  stays on the bar. Costs up to ~100 MiB more RAM while playing.
- Tooltip unchanged; the HLS fallback keeps its `readyRanges` layer.
- **Implemented (2026-09-29) with the linear mapping only:** `StatsFile`
  (vendored enginefs) → `StreamStats.downloadedByteRanges` (file-relative
  bytes) → `downloadedTimeRanges` in `PlayerView.tsx` maps bytes/total ×
  duration, joins runs < 1s apart, drops < 0.5s specks, and draws them with
  the existing `.player-seek-ready` class. The keyframe-index mapping is not
  done yet: on a VBR encode the dim layer can be off by some seconds
  mid-file. Reading MKV Cues needs those bytes downloaded, which the tail of
  the file often isn't early on.

**Playback buffering, round 2** (live test 2026-09-29 12:26-12:28, resume of
Kaleido-subs Fate/strange Fake 11 at 1:43 - 2 GB HEVC, 512 KB pieces, 3-11
MB/s): click → first frame took **46s**, then stalls after the first frame and
5-13s per seek. From the log:

| Time | What happened |
|---|---|
| +0-7s | mpv reads the header (piece 0) while peers ramp 4 → 18 |
| +10-14s | header continues past 8 MB (font attachments); the startup buffer "verifies" on those header pieces, so baseline 1 is released |
| +14-35s | mpv reads the **Cues at 99.6%** (pieces 3813-3823): **21s** of waiting while 286 other pieces (~143 MB) arrive rarest-first |
| +35s | resume anchor fires on piece 16 (header continuation, not the resume point), then re-anchors on the real seek to piece 386 |
| +35-46s | resume region downloads; first frame at +46s |
| +49s | stall: reader at piece 454 with 26 of 32 read-ahead pieces missing |

Fixes (vendored enginefs unless noted):

1. **Index reads are metadata.** Our stream server opens every request at 0
   and seeks, so enginefs' `is_container_metadata_request` (request-offset
   based) never fires. A read/seek landing at or after
   `container_metadata_start` becomes `ContainerMetadata`: priority 7 +
   deadlines over the (16 MB-capped) window and pinned, so it outranks bulk
   and read-ahead.
2. **Prefetch the index at playback start.** When a file starts streaming,
   its tail (last 4 MB, MKV Cues / MP4 moov) is requested at priority 7 in
   parallel with the header, unless already downloaded - removing one
   sequential round trip before mpv can seek.
3. **Resume anchor on the real jump.** Anchor continue-watch on the first
   read that is a forward *jump* (not contiguous with the previous read) and
   not a metadata/tail read - never "first read past 8 MB", which header
   attachments trip.
4. **Resume keeps the baseline held until the resume region is ready.** With
   `watch: resume`, baseline stays 0 until the anchor's buffer (8 MB from
   the anchor) verifies, or 15s after the anchor - not when the header's
   first 8 MB does.
5. **Bitrate-sized read-ahead.** After the first byte, the priority window
   covers >= 30s of playback (file size / duration from the stream's
   bitrate estimate; 2 GB/24 min ≈ 1.4 MB/s → ~42 MB), capped at 64 MB,
   instead of a fixed ~4 MB.
   Within the window, priorities are graded by distance like Elementum's
   (7 for the ~4 MB urgent band, then 6, 5, 4, 3 and 2 further out), so
   nearer pieces win when bandwidth is short.
6. **Play once ~10s is buffered** (`embedded.rs`, user request 2026-09-29):
   `--cache-pause-initial=yes` + `--cache-pause-wait=10` - playback starts,
   and resumes after a seek or stall, only when 10s of video is in mpv's
   cache, instead of showing the first decodable frame and stalling again.
   A stall therefore pauses ~10s once instead of stuttering.
7. **Seeks re-prioritize at once:** a seek's first read already puts the
   target at priority 7 / deadline 0 and grows to the 30s graded window
   after the first byte (fix 5); in-order download re-anchors there after
   100ms blocked (`SEEK_REANCHOR_DEBOUNCE_MS`, was 300ms).

**Compared with Elementum** (`elgatito/elementum`, MIT, active; studied
2026-09-29 - `lt2http` from the same org is unmaintained since 2021 and
unlicensed): it pre-buffers head + tail (`end_buffer_size`, 1 MB default) at
priority 7 / deadline 0 before playing, reads ahead a flat 50 MB with graded
priorities (adopted above), keeps the rest of the chosen file at 1, and
forces tracker + DHT announces. It also has an optional libtorrent
pause/resume right after setting buffer priorities, which drops requests
already queued at peers - the backlog behind "max outstanding piece requests
reached". Not adopted (it disconnects every peer); the fallback if fixes 1-5
don't clear the queue delay.

**Round 3 (live test 2026-09-29 12:50, first watch of a 2 GB Kaleido-subs
HEVC file at 10-14 MB/s: ~31s to moving video).** Piece 0 now arrives in
2.8s, but two stalls remain, both *single pieces stuck 10-15s at priority 7
while hundreds of others complete*: the index read (piece 3803, 13s) and head
pieces 18-20 (15s). libtorrent-sys' hard-coded session settings
(`wrapper.cpp`) let one slow peer own an urgent piece: `whole_pieces_threshold
= 30` hands whole pieces to any peer that could finish one within 30s,
`strict_end_game_mode = true` stops other peers requesting its blocks, and
`request_queue_time = 3` queues ~3s of requests ahead of urgent ones. The
index also started 5.6 MB from the end (Kaleido's fonts), outside the 4 MB
tail prefetch.

1. **Vendor `libtorrent-sys`** (`vendor/libtorrent-sys`, same `[patch]` as
   enginefs, rev f585ab6): `whole_pieces_threshold` 30 → 2,
   `strict_end_game_mode` → false (stalled blocks can be requested from
   other peers too, at the cost of a little duplicate download),
   `request_queue_time` 3 → 1. Also the base for fast resume later.
2. **Tail prefetch = 1% of the file, clamped 4-16 MB.**

**Decoder corruption on downloaded data** (ToonsHub movie, ~40s after a seek
to 34:21): smeared blocky frames for about a minute; seeking back to the
same spot played cleanly, so the bytes on disk are right. A one-off bad input
leaving the hardware decoder (`--hwdec=auto-safe`, D3D11) in a bad state
until a seek flushed it fits; software decoding recovers at the next
keyframe. mpv's own messages aren't logged, so the cause can't be confirmed
yet:

3. **Capture mpv warnings/errors:** `mpv_request_log_messages("warn")`,
   log-message events written to our log (`mpv` target, level mapped), rate
   limited (max 20 lines/s, suppressed count summarized) so a decoder error
   storm can't flood it.
4. **Hardware decoding setting** (Settings → Playback: Auto / Off, default
   Auto): applied as mpv's `hwdec` property (`auto-safe` / `no`) on attach and
   when changed, so the corruption can be tested with software decoding.

**Corrupt bytes on fresh pieces** (Slime S4E24, 2026-09-29 13:12-13:13; seen
thanks to the mpv log capture): mpv's MKV demuxer logged `Corrupt file
detected. Trying to resync` 7 times at 1.46-1.55 GB, all within 1-4s after a
seek and all inside pieces verified in the previous 1-2s (2 MiB pieces
697-698, etc.). No zero reads were logged, so the zero guard never fired -
the reader handed out stale non-zero bytes. libtorrent hashes a piece from
its own buffers and may queue its disk writes (`max_queued_disk_bytes` 128
MB - up to ~12s at 10 MB/s), so the reader's separate OS file handle can
see a verified piece's *old* disk content: zeros (caught) or data from an
earlier failed/overwritten write (not caught). Software decoding hid the
damage better (recovers at the next keyframe); the hardware decoder stayed
corrupted until a seek.

- **Fix:** pieces verified within the last 30s are served only from
  libtorrent's own copy (`read_piece`, which includes unflushed writes) -
  never the OS file handle. `PieceWaiterRegistry` records when each piece
  finished (`finished_within`). Older pieces keep plain disk reads. If
  libtorrent's copy fails (timeout/error), the disk path is used as before
  (the zero guard still applies).

**Hero clips long titles** (Fate/kaleid liner Licht Nameless Girl,
2026-09-29): `.home-hero` has a fixed `height: 340px` and bottom-aligns its
body, so a title that wraps to 3 lines at the 4.4rem cap pushes the reason
chip and the title's first line up under the app bar (`overflow: hidden`).

- **Fix:** `min-height: 340px` instead of `height`, so the hero grows to fit;
  the title is clamped to 2 lines (`-webkit-line-clamp`, full name in
  `title=`) and titles over ~40 chars drop to a smaller clamp
  (`.home-hero-title.long`) so the carousel's height barely changes between
  slides.

**Rebuild the buffer on corruption** (user request 2026-09-29): a safety net
behind the fresh-piece fix - the bytes on disk are right later, so
re-reading heals whatever got through.

- **Detect** in the mpv log capture (`libmpv.rs`): the MKV/lavf demuxer's
  `Corrupt file detected` / resync, or the video decoder's `error while
  decoding` / `concealing ... errors`. Not `mmco` errors - A-B loop jumps
  produce those harmlessly. Matching lines are also sent to the frontend as
  an IPC-shaped `stream-corrupt` event (prefix, text).
- **Rebuild** (`MpvVideo`): mpv `drop-buffers` (discards demuxed and decoded
  data), then an exact seek to the current time through the normal seek
  path, so mpv re-reads from the playhead.
- **Guard rails:** at most one rebuild per 5s; after 3 rebuilds within 60s
  it stops and logs an error rather than looping on a spot that stays bad.

**What's new dialog** (user request 2026-09-29): after an in-app update
restarts the app, a dialog lists the new version's features and fixes -
never on a fresh install or a normal launch.

- `src/changelog.ts`: bundled notes, `{ version, date, features[], fixes[] }`
  per release, newest first, in plain user-facing words.
- `installUpdate` (`src/updater.ts`) stores `nyaa-stream:updatedFrom` =
  `{ from, to }` in localStorage *before* `downloadAndInstall` - on Windows
  the passive NSIS installer takes over there and the app may never reach
  `relaunch()`.
- At startup (`src/WhatsNew.tsx`, mounted in `App.tsx`): if the marker's `to`
  equals the running version (`getVersion()`), show every changelog entry
  newer than `from` up to it (a skipped release, e.g. 0.3.1 → 0.4.0, shows
  all of them), then clear the marker. A marker whose `to` doesn't match
  (failed install) is cleared silently. Closes with a button or Escape.
- **Bridge for 0.3.1 and older** (their updater writes no marker): every
  launch records `nyaa-stream:lastVersion`; with no record yet but existing
  `nyaa-stream:*` user data, the launch counts as an update from 0.3.1. A
  fresh install has no data and shows nothing.
- **Updates outside the in-app updater** (v0.4.2): no marker, but a
  recorded `lastVersion` older than the running one counts as an update
  from it (the setup .exe run by hand).
- **Every release needs a `CHANGELOG` entry** (found 2026-10-08: none were
  written for 0.4.0/0.4.1, so updating to 0.4.x showed nothing - the
  dialog has nothing to list). `release.mjs`'s preflight now refuses a
  version without one.
- **Fixture check of v0.4.3 matching** (now part of v0.5.0) (2026-10-08,
  `title_match_fixture_tests.rs`, `cargo test -p nyaa-stream
  title_match_fixture -- --nocapture`). Recall per entry with the TVDB
  season: Mushoku 100% on S3 (375/375), 100% on Cour 2, 86-100% elsewhere;
  Slime 100% except S4 Part 3 (588/655: AniList gives that entry no English
  title); Food Wars 100% on S1/S2/S4/S5. Known gaps: without a TVDB season a
  later entry with a TV prequel and no season in its titles rules nothing
  out (Mushoku Cour 2 keeps 118 S3 releases) and cannot accept the bare
  "Food Wars! S03" form (Food Wars S2-S5 recall 35-74%); ~80% of entries
  lack a Fribb TVDB season, so the prequel chain stays a first-season flag.
  Rules the check added: a sibling only takes a release that carries none of
  this entry's seasons; shared leading words with a relative are the entry's
  short name (also a search query); "movie/OVA/OAD/special" words keep a
  release off a TV entry unless it is a pack that bundles extras; a roman
  numeral after a short name is a season.
- **`CHANGELOG.md`** (2026-10-08) is the developer-facing mirror of
  `src/changelog.ts` (Keep a Changelog format, plus an `Unreleased`
  section). Add the entry to both when a release is cut.

**Splash screen** (user request 2026-09-29, "startup was pretty slow"). The
log of that launch: the Rust side was ready 0.5s in, but the page's first
script ran 32s later - Vite's dev server serving unbundled modules after a
fresh start (a release build loads one bundle from disk). A splash covers
that gap, and the first moments of a release launch.

- Inline in `index.html` (markup + CSS, no JS), so it paints before any
  script: the BrandMark cat head on the app background, a looping animation
  (head bob + ear twitch, a light sweep across the logo, the play triangle
  pulsing, a sakura arc orbiting it, a few drifting petals, a breathing
  glow) and the nyaastream wordmark. Always animates (no reduced-motion
  switch, per the project's convention).
- `main.tsx` fades it out (0.45s) two frames after the first render, then
  removes it.

**Logo polish** (user request 2026-09-29, same idea - a sakura cat head
with a play button - better drawn): friendlier rounded ear tips with a
darker inner ear, softer head corners and chubbier cheeks, the play
triangle rounded and optically centered in the face (a triangle's visual
center sits right of its box center), a soft top highlight. Used
everywhere: `BrandMark` (`src/icons.tsx`), the splash (`index.html`, ears
split out to twitch), `public/favicon.svg`, `src-tauri/icons/icon.svg`, and
the app/taskbar/installer icons regenerated from it
(`npm run tauri icon src-tauri/icons/icon.svg`). Checked side by side with
the old mark at 16, 32 and 128 px before switching.

**Later (not in v0.3.2):** prefetch the head (~8 MB, then dropped if unused)
of the most likely source when an episode page opens - depends on A/B.

## Native splash window and custom title bar (planned, v0.4.0 - minor: new feature)

**Native splash window.** Startup gets its own small window instead of
relying on the main page: a borderless, non-resizable, centered splash
window (`label: "splash"`, ~360x400, no taskbar entry) showing the logo
animation from v0.3.2's in-page splash (moved to `public/splash.html`).
The main window starts hidden (`visible: false`); after the app's first
paint, `main.tsx` calls an `app_ready` command that shows the main window
(maximized) and closes the splash. If `app_ready` never arrives within 20s,
Rust shows the main window anyway (logged as a warning) so a broken page
can't leave the app invisible. The inline `index.html` splash goes (the main
window is hidden until it has painted). Never `transparent: true` on either
window - it broke click input (CLAUDE.md).

**Custom title bar.** The main window becomes frameless
(`decorations: false`); a slim strip (~32px) across the top of every page
replaces the system title bar and *blends with the app bar* below it (same
translucent glass, no visible seam): drag region (`data-tauri-drag-region`,
double-click toggles maximize), a small logo + "nyaa-stream" on the left,
minimize / maximize-restore / close on the right (Windows-style hit targets,
close turns red on hover). Resize edges stay (undecorated resizing, window
shadow on). In the player the strip shows and fades with the player's top
controls, and is hidden in fullscreen and PiP. Needs the window
permissions (`core:window:allow-start-dragging`, `-minimize`,
`-toggle-maximize`, `-close`, `-show`) in the main capability.
Player pointer tracking spans the document so the title-bar overlay remains
part of the top hover area instead of repeatedly leaving and re-entering the
player surface. Player controls are shown only over the top/bottom hover
areas (or an open player menu); moving out hides them immediately.

## Download cache (planned, v0.4.0 - minor: new feature)

Downloaded pieces are kept on purpose and reused, instead of piling up by
accident. Re-opening a torrent whose files are still in `downloads/` already
reuses them: sbtl reads its resume file (`downloads/.sbtl/<hash>.resume`)
or re-checks the existing data on add (`state=1`), and every verified piece
is instantly available - seeking back or re-watching never downloads them again.

- **Index:** `downloads/.cache-index.json` - per info hash: its files on disk,
  bytes used, `last_used`, and the (anime, episode) it was played for.
  `last_used` is updated whenever a file of it starts streaming.
- **Cap:** Settings → "Download cache" size, default **10 GB** (0 = delete a
  torrent's files when playback stops). After `stop_playback`'s removal and
  at app start, least-recently-used torrents are deleted (files + index
  entry) until the total fits. Never the torrent currently playing.
- **Continue watching:** an episode in the Continue watching row keeps its
  torrent's files regardless of the cap (the cap may be exceeded by those);
  once it leaves the row (dismissed, watched, replaced by a newer episode),
  its files become ordinary cache entries and are evicted first.
- **Clear cache** also empties `downloads/` (except the playing torrent).
- **With the resume buffer (below):** both are kept. The resume buffer serves
  the first seconds of a resume instantly even while sbtl re-checks a
  large cached file, and survives the cache evicting it.
- **Fast resume:** built into sbtl (its resume file), so a cached torrent
  skips the re-check unless the file changed.
- Commands: `download_cache_status` (size/entries, for Settings),
  `set_download_cache_limit`; eviction logged per torrent (name, bytes,
  reason).
- **As built** (`src-tauri/src/download_cache.rs`, `src/downloadCache.ts`):
  entries hold file paths relative to `downloads/` (eviction also deletes
  the torrent's `.sbtl/<hash>.resume`/`.torrent` and any `.<hash>.parts`
  libtorrent left; `downloads/.sbtl/` itself is never an orphan nor
  pruned), and the `<animeId>:<episodeKey>` episodes played
  from them (`play_magnet`'s new `episode` arg). Sizes are *allocated*
  bytes (`GetCompressedFileSizeW`) - the files are sparse. Eviction runs 2s
  after `stop_playback`'s removal (the engine releasing the files), at
  startup (also sweeping unindexed leftovers, by mtime - only then, so a
  just-added torrent is never mistaken for one), and on a limit or keep
  change. The frontend sends the Continue watching row's episodes via
  `set_download_cache_keep` whenever progress/dismissals change; an entry
  leaving the row gets `last_used = 0`. Kept entries survive even a 0 cap.
  Clear cache deletes everything but the playing torrent (kept ones too).
  Thumbnail captures' scratch files are deleted unless that release is
  cached.

## Continue-watching resume buffer (planned, v0.4.0 - minor: new feature)

Each episode in the home page's Continue watching row keeps a small on-disk
buffer of *its* source, so Resume starts playing at once while the torrent
reconnects, instead of waiting on the swarm (see "Fast playback start").
Depends on v0.3.2's `watch: resume` continue-watch mode.

- **What is kept, per entry (~16 MB + open data):** whole verified pieces
  (already hash-checked, so they are trustworthy as-is) covering
  1. the ranges mpv read to *open* the file - header, tracks, attachments,
     MKV Cues / MP4 moov - recorded by the stream handler from the start of
     the load until mpv's `file-loaded`;
  2. ~16 MB from the keyframe at or before the saved position. The byte
     offset comes from the container index via in-process FFmpeg
     (`media.rs`, same ffmpeg-next probe as `MediaProbes`, index lookup
     time → byte), not from a bitrate estimate.
- **When:** on player close / episode change (`stop_playback`), before the
  torrent and its download folder are removed, when the entry is still in
  progress (not completed, past the resume threshold). Pieces missing
  locally at that moment are skipped - the buffer is best-effort.
- **Where:** `<cache_dir>/nyaa-stream/resume/<info_hash>-<file_idx>/` - a
  `pieces.bin` + `index.json` (piece length, piece → offset in the bin, the
  file's size/name, the position it was cut for, the magnet). One buffer per
  (anime, episode); saving again replaces it.
- **Which source resumes:** `ProgressEntry` gains `source` (magnet, file
  index, file name). Resume prefers that release when it is still listed, so
  the buffer matches the bytes; otherwise the normal pick applies and the
  buffer is ignored (then deleted).
- **Serving it:** the stream handler answers a range from the buffer's
  pieces when they cover it, and opens the engine reader only where they
  stop; the engine's continue-watch anchor is set to the buffer's end, so
  in-order download starts right after what's already local. Nothing is
  written back into libtorrent's storage (libtorrent-sys has no `add_piece`
  binding; the torrent re-downloads those pieces normally).
- **Removal:** the buffer is deleted when the episode leaves Continue
  watching - dismissed from the row (`dismissContinueWatching`), marked
  watched / completed, or replaced by a newer episode of the same anime
  (the row keeps one entry per anime) - and by Settings → Clear cache.
  A startup sweep deletes buffers with no matching progress entry. Hard cap
  of 25 buffers / ~600 MB, oldest `updatedAt` first.
- **As built** (`src-tauri/src/resume.rs`,
  `crates/torrent-engine/src/resume_buffer.rs`): no separate commands.
  `stop_playback(resume)` takes `{animeId, episodeKey, fileIdx, position,
  magnet}` from the player's unmount (only when position >= 20s and < 90%)
  and saves first, while the torrent is still in the engine. Open reads are
  recorded by `stream_handler` until the player calls `stream_file_loaded`
  (mpv's `file-loaded`; capped at 64 MB). The bytes are the intersection of
  the wanted ranges with the file's verified runs (`downloaded_ranges`),
  read through the engine's own reader (`read_ranges`, a background read).
  Not from the file on disk: a just-verified piece may not be written there
  yet (sbtl's disk worker can hold verified pieces in memory). They
  are stored in `data.bin` + `ranges.json`, with the
  app's `entry.json` beside them. The keyframe offset comes from
  `media::keyframe_byte_offset` on the *local file* (no HTTP, 10s timeout).
  A wrong offset only makes the buffer less useful; it never serves wrong
  bytes. `play_magnet` attaches any buffer of that info hash whose file
  size still matches. The stream handler then serves through a
  `BufferedReader` that opens the engine's file handle lazily, only when a
  read leaves the buffer, and stops torrent reads at the next buffered run.
  Removal rides the download cache's keep sync: `set_download_cache_keep`
  also sweeps buffers whose episode isn't in the list, which covers
  dismissed, watched, replaced and the startup sweep. Clear cache (the
  torrent data row) deletes `resume/` too, and its size counts there.

## Subtitle preloading (planned, v0.4.0 - patch: fix)

Symptom: on first play the default subtitle track is selected (the pick
effect in `PlayerView.tsx` runs once per file when mpv's `track-list`
first arrives and sends `sid`) but nothing renders until the user toggles
subtitles off and on. Re-sending `sid` works, so the selection itself is
right; mpv just hasn't started reading the track's packets from the torrent
stream at that point (the pick fires while the file is still opening,
before the demuxer has the track's early packets).

- **Log finding (2026-09-29, `nyaa-stream.2026-09-29.log`)**: every switch
  to a track (`sid` 1) is followed ~100ms later by `[mpv] buffering`, and
  the engine logs "waiting for verified piece". mpv discards subtitle packets
  it read while the track was off, so selecting an embedded track mid-play
  makes the demuxer re-read the subtitle packets from earlier in the file
  (ASS needs every event since the start) - byte ranges of the torrent that
  may not be downloaded yet. `sid=no` therefore also drops the track's
  packets, so cycling EN → off → EN re-triggers the reload each time. This
  is the same mechanism as the "subs don't show until toggled": the
  on-open pick arrives after mpv already started demuxing without the track.
- **Fix**: (1) Off = `sub-visibility=no`, keeping `sid` on the selected
  track so its packets keep being demuxed (no buffering when cycling
  EN ↔ off); `sid` only changes when the user picks a different track.
  (2) Preload: pass the default track to mpv at load time (`sid=<default>`
  / `--slang` from the setting, set before `loadfile`) so packets are read
  from the first byte instead of after the on-open pick, and re-assert `sid`
  once after the first `playing` if `sub-text` is still empty. (3) If a
  track switch to a *different* track still stalls, treat the subtitle byte
  ranges as metadata reads in enginefs (priority 7) - decide after (1)-(2).
- Never changes the user's own on/off/track choice; no-op with subtitles
  off or no tracks.
- **As built** (`src/mpvVideo.ts`): `load()` sets `slang` (the setting's
  code plus its ISO 639-2 form, e.g. `en,eng`), `sid=auto` and
  `sub-visibility` (= subtitles enabled) before `loadfile`.
  `setSubtitle(null)` only hides. A track pick reads `sid` and sets it only
  when it differs. The re-assert polls `sub-text` once a second for 10s
  after the first frame. If no text showed and a track is selected and
  visible, it sends `sid=no` then the track. With subtitles disabled, mpv
  still auto-selects and demuxes a track, so turning them on is instant.

## Contact and send logs (planned, v0.4.0 - minor: new feature)

Settings gets a **Help** section with two buttons. Nothing is uploaded
automatically - no webhook or other secret ships in the public app.

- **Contact** opens a modal with two links: the repo
  (`https://github.com/sb-gravity100/nyaa-stream`) and the author's Discord
  username (`__sb______`) with a Copy button - a profile link would need
  the numeric user id. Links open in the system browser (opener plugin).
- **Send logs** (`export_logs` command) zips the log folder
  (`%LOCALAPPDATA%
yaa-stream\logs`, the last 7 daily files) plus a
  `system.txt` (app version, Windows version, mpv available, libmpv
  version) into `nyaa-stream-logs-<date>.zip` in Downloads, reveals it in
  Explorer, then offers two ways to send it:
  - **Discord** - copy the author's username, then drag the zip into a DM.
  - **GitHub** - opens a new issue on the repo prefilled (title, app version,
    OS, "describe what happened"); the user attaches the zip. The modal notes
    that GitHub issues are public.
- **Development log location:** dev builds write daily logs directly into
  the working directory from which the app was launched; release builds keep
  using `%LOCALAPPDATA%\nyaa-stream\logs`.
- **Windows username hidden:** while zipping, every occurrence of the
  user's profile path (`C:\Users\<name>`, also with forward slashes and in
  JSON-escaped `\` form) is replaced with `%USERPROFILE%`, and the bare
  account name (`%USERNAME%`) with `<user>`; the zip gets the scrubbed
  copies, the files on disk stay untouched. Everything else is kept as-is
  (author's call, 2026-09-29): torrent/episode titles and fansub groups
  stay, since they're needed to debug matching and playback. The modal says
  in one line that watched titles are included and GitHub issues are
  public. Tokens are never logged in the first place (logging rules + TMDB
  token plan), so none can leak.
- Logged: button presses, zip path and size, failures.
- **As built** (`src-tauri/src/logs_export.rs`, `src/HelpSection.tsx`):
  the zip is `nyaa-stream-logs-<UTC date>.zip`. `system.txt` holds the app
  version, `cmd /C ver` (no console window), arch, mpv available, and the
  libmpv version (`mpv-version` when the player has started this run,
  otherwise "unknown"). The scrub is ASCII case-insensitive and also
  applies to file names. Discord is the username with a Copy
  button in both modals (`DISCORD_USERNAME` in `HelpSection.tsx`). The GitHub issue body
  carries only the version and "Windows".

## Load profiler (planned, v0.4.0 - patch: diagnostics)

Playback start and seeks/resumes sometimes take long, and the logs don't
say why: the evidence is scattered across `play_magnet`, the metadata wait,
enginefs's "waiting for verified piece" and the mpv events, with no
timeline tying them together. A per-load trace answers "where did the time
go" in one log line. It also gives the numbers for the fast-resume
(re-hash) decision in "Download cache".

- **Traces:** one per load, of two kinds. `start` runs from the player
  opening to mpv's first frame. `seek` runs from a seek or Resume to mpv's
  `playback-restart`. The frontend opens one with `trace_begin(kind)` and
  gets an id back. Stages are marked from the frontend (`trace_mark`) and
  from the backend, as monotonic ms since begin.
- **Start stages:**
  - `player_open` (mount), then `play_magnet`, split in the backend into
    `torrent_add`, `metadata` (the `files()` wait) and `checking` (time the
    torrent spent in libtorrent's `checking_files` state, i.e. the
    re-hash, polled every 250ms while `play_magnet` runs).
  - `resume_buffer` (attached, bytes), `loadfile`, `first_request` (the
    first foreground `stream_handler` request), `file_loaded` (mpv), then
    `first_frame`.
- **Seek stages:** `seek_issued`, `first_request` at the new offset, and
  `first_frame`.
- **Torrent wait accounting:** while a trace is open, the stream readers
  (`RecordingReader` / `BufferedReader`) time each read that went Pending
  on the engine (Pending -> Ready). The trace sums them: count, total ms,
  longest, plus bytes served from the resume buffer vs. the torrent. Peers
  and download rate are snapshotted at the end.
- **Output (logs only):**
  - A debug line per stage (`[trace] start#3 metadata +2310ms`).
  - One info summary when the trace completes, e.g.
    `load trace kind=start total_ms=8210 torrent_add=40 metadata=2310
    checking=3100 open=900 first_frame=... torrent_wait_ms=4200 waits=12
    longest_wait_ms=2100 buffer_bytes=... peers=... rate=...`.
  - A trace whose player closes first is logged with `outcome=abandoned`.
  - The summaries land in Send logs zips automatically.
- **Cost:** no new threads. The checking poll runs only during
  `play_magnet`, and the wait timing is an `Instant` per Pending read.
- **Then:** live summaries for a cold start, a cached re-open, a Continue
  watching resume and a seek into undownloaded data. They decide fast
  resume (libtorrent resume data) and point at the next fix.
- **As built** (`src-tauri/src/load_trace.rs`,
  `crates/torrent-engine/src/read_stats.rs`, `src/loadTrace.ts`):
  - Commands are `trace_begin(kind, torrentId?)`, `trace_mark` and the
    async `trace_end`; `play_magnet` takes `trace`.
  - In the summary, `name=ms` is a span and `name@ms` is an offset from
    the start. The fields are `first_request_ms`, `requests`,
    `torrent_wait_ms`, `waits`, `longest_wait_ms`, `torrent_bytes`,
    `buffer_bytes`, `peers` and `rate_mbps`.
  - The `checking` state comes from a new vendored
    `EngineStats::state` (libtorrent's state code). The probe gives up
    after 10s if no checking was seen.
  - Wait and request events sit in a per-torrent ring of 1024; byte
    counters are diffed against a baseline taken when the trace learns its
    torrent.
  - The start trace ends on mpv's `canplay` (playback-restart, the first
    frame) and is abandoned on a source switch, a failure or a close.
  - A seek trace opens on a new `seeking` event from
    `MpvVideo.flushSeek` (only once the start trace has ended), marks
    `retarget` for coalesced seeks, and ends on `seeked`.

## Settings as a centered modal (v0.5.0, was v0.4.4 - UI change, ships with the dataset work)

The Settings panel (`src/SettingsPanel.tsx`, styles in `src/App.css`
"Settings") was a right-edge drawer (full height, slides in from the right).
It becomes a centered modal dialog: dimmed + blurred backdrop, the panel
centered with rounded corners, width `min(640px, 92vw)` (the AnimeTosho
import row and its progress need more room than the 460px drawer), height
`min(82vh, 780px)`, header/footer fixed and the body scrolling. Opens with a
short scale+fade pop instead of the slide. Behaviour unchanged: same
`role="dialog"`/`aria-modal`, Esc and backdrop click close it, focus is
trapped as before. Phone-width windows (<600px) fall back to full-screen.
CSS only; no component logic change.

