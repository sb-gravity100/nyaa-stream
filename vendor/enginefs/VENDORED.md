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
  invalid as first byte of an EBML number`), corrupting demux.
