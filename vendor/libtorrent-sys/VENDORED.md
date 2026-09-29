# Vendored libtorrent-sys

Copied from https://github.com/stremio-native/stream-server at rev
`f585ab6eda9b1411034548c131bb0dc30c6f5f9e` (`bindings/libtorrent-sys/`),
MIT licensed (see `LICENSE`). Wired in through `[patch]` in the workspace
`Cargo.toml`, next to the vendored `enginefs`.

## Local changes

- `cpp/wrapper.cpp` session settings, for streaming (live logs showed single
  pieces stuck 10-15s at priority 7 while hundreds of others completed at
  10-14 MB/s):
  - `whole_pieces_threshold` 30 -> 2: at 30 any peer able to finish a whole
    piece within 30s (~20 KB/s for 512 KB pieces) was handed whole pieces,
    so one slow peer could own an urgent piece.
  - `strict_end_game_mode` true -> false: other peers may request the blocks
    of a stalled piece (some duplicate download).
  - `request_queue_time` 3 -> 1: less queued ahead of urgent requests.
