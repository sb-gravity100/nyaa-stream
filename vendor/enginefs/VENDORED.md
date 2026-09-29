# Vendored enginefs

Copied from https://github.com/stremio-native/stream-server at rev
`f585ab6eda9b1411034548c131bb0dc30c6f5f9e` (`enginefs/`), MIT licensed
(see `LICENSE`). Wired in through `[patch]` in the workspace `Cargo.toml`;
`libtorrent-sys` still comes from the same upstream git revision.

## Local changes

- `src/backend/libtorrent/disk_stream.rs`: a disk read that returns an
  all-zero chunk is re-served from libtorrent's own `read_piece` (the
  "broker") instead of being passed on. Upstream only guarded the very
  first read at offset 0; in practice libtorrent reports a piece as
  verified before its bytes are visible through a separate OS file
  handle, and ffmpeg received runs of `0x00` mid-file (`0x00 at pos N
  invalid as first byte of an EBML number`), corrupting demux. The reader
  waits up to 8s per piece for real bytes before accepting zeros as
  genuine padding; a shorter 200-500ms bound was verified to accept real
  unflushed bytes as zeros and produce pixelated frames.
  Detection works per 16 KiB block (libtorrent's write granularity), not
  per read: a read that is valid bytes followed by an unflushed block is
  served only up to that block (`first_zero_block`), since whole-chunk
  checks let exactly those mixed reads through.
- `src/backend/libtorrent/disk_stream.rs`: a file read that returned
  Pending is finished (and its bytes discarded) before the reader seeks
  its file handle. The broker path can advance the position while a
  tokio `File` read is still in flight, and tokio then rejects the seek
  with "other file operation is pending" - surfaced to readers as I/O
  errors mid-stream, several times per episode (verified live).
- `src/backend/libtorrent/playback.rs`, `src/backend/libtorrent/handle.rs`,
  `src/backend/mod.rs`: `TorrentHandle::set_preload_file` - the coordinator
  keeps every file but the playing one at priority 0, so nothing else in a
  batch downloads. A registered preload file (the next episode) now gets
  libtorrent's lowest priority (1) alongside it, applied on the next
  activation and immediately if a file is already playing. The playing file's
  own piece windows keep their higher priorities, so the preload only uses
  spare bandwidth.
- `src/backend/priorities.rs`, `src/backend/libtorrent/playback.rs`,
  `src/backend/libtorrent/disk_stream.rs`: startup baseline 0
  (`StartupHold`). Upstream kept the whole playing file at priority 1 from
  the first request, so libtorrent filled every peer's request queue with
  rarest-first bulk pieces and the head piece queued behind them (measured:
  piece 0 took 20s while 342 other pieces arrived first). A foreground
  stream's new generation now holds every piece of the file outside the
  priority windows at 0 - piece priorities, not the file priority, which
  would route pieces into libtorrent's part file away from the disk reader -
  and raises them to 1 once `STARTUP_BUFFER_BYTES` (8 MB) from the read
  position verified, or after 15s. Retired window pieces reset to 0 while
  held. Streams report their first read position (`ReadStarted`) so the
  buffer follows mpv's seek; a new generation drops an unraised hold and
  forgets the acknowledged file priorities, whose rewrite resets every piece.
- `src/backend/priorities.rs`, `src/backend/libtorrent/disk_stream.rs`:
  ~4 MB startup window. `MAX_STARTUP_PIECES` 4 -> 16 (byte-capped at 4 MB)
  and `INITIAL_FIRST_BYTE_WINDOW_PIECES` 3 -> the same, all at priority 7
  with staggered deadlines; after the first byte the next ~4 MB
  (`disk_backed_urgent_pieces`) stay at 7 instead of only the current
  piece (still yielding to missing Cues/moov pins). Upstream made each
  piece urgent only when the reader reached it, paying the swarm's queue
  delay per piece.
- `src/backend/priorities.rs`, `src/backend/mod.rs`,
  `src/backend/libtorrent/handle.rs`, `src/backend/libtorrent/playback.rs`,
  `src/backend/libtorrent/disk_stream.rs`: sequential download for the
  watched file (`WatchHint`, `TorrentHandle::set_watch_hint`). The hint is
  stored per torrent and taken by the next foreground stream of a file that
  isn't already streaming. First watch: `set_sequential_download(true)`
  from piece 0, no startup hold. Continue watch: the first reported read
  past the header marks the resume point - pieces before it drop to 0,
  the rest are 1, sequential from there; the skipped pieces return to 1
  once the rest verified or on a seek back into missing ones. A foreground
  read blocked 300ms on its start piece that in-order download won't reach
  soon re-anchors at that piece (`SeekBlocked`). Upstream forced sequential
  mode off on every window move (`prioritize_from`, `apply_hot_window`);
  the coordinator owns it now. Background/probe reads never report
  positions.
- `src/backend/libtorrent/disk_stream.rs`: the "waiting for verified piece"
  diagnostic reports the window of the effective intent (sequential after
  the first byte), not the stream's original intent.
- `src/backend/libtorrent/playback.rs`, `src/engine.rs`: a file-priority
  update that doesn't read back within the 2s acknowledgement timeout is
  resubmitted once and then streamed anyway (warned as
  `file_priority_unconfirmed`, with the mismatched file indices). Upstream
  failed the activation, so the stream server answered "unknown file
  index" and mpv waited with no error (a Kaleido-subs torrent, verified
  live). `get_file_with_intent` now logs why `get_file_reader` failed
  instead of discarding the error.
- `src/backend/mod.rs`, `src/backend/libtorrent/handle.rs`: `StatsFile`
  gains `downloaded_ranges` - the file's verified pieces merged into
  file-relative `[start, end)` byte runs (`file_downloaded_ranges`, from the
  piece presence `stats()` already reads). The player's seek bar draws them
  as a "downloaded" layer; mpv's own demuxer cache forgets ranges after a
  seek, so it can't show what's on disk. Other backends leave it empty.
- `src/backend/libtorrent/disk_stream.rs`, `playback.rs`, `src/backend/priorities.rs`
  (buffering round 2, from a live resume that took 46s to the first frame):
  foreground reads in the file tail are `ContainerMetadata` (our stream
  server opens at 0 and seeks, so upstream's request-offset check never
  fired) with the whole window at 7; the file tail (1% of the file, 4-16 MB) is
  requested at 7 with the header (`TAIL_PREFETCH_STREAM_ID`); after the first byte the read-ahead
  covers 30s of playback (cap 64 MB) with priorities graded 7→2 by distance,
  like Elementum; streams report where playback reads ended so a resume
  anchors only past that frontier (`is_resume_point`), and a resume keeps
  the startup hold until it anchors (`RESUME_ANCHOR_FALLBACK_EXTRA_MS`);
  `SEEK_REANCHOR_DEBOUNCE_MS` is 100ms.
- `src/piece_waiter.rs`, `src/backend/libtorrent/disk_stream.rs`: a piece
  verified less than 30s ago (`FRESH_PIECE_BROKER_WINDOW`, finish times kept
  by `PieceWaiterRegistry::finished_within`) is served only from
  libtorrent's own copy (`read_piece`), never this stream's OS file handle.
  libtorrent hashes from its buffers and may queue the disk write, so the
  handle could read the piece's old disk content - stale *non-zero* bytes
  the zero guard can't catch (mpv's MKV demuxer: "Corrupt file detected",
  within seconds of a seek, only on just-verified pieces). The disk path
  (and zero guard) remains the fallback when libtorrent's copy fails.
