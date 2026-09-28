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
