# sbtl.dll

The prebuilt sbtl torrent engine that nyaa-stream links, instead of compiling
sbtl's C sources on every clean build. Committed on purpose (about 170 KB).

| file | what |
|---|---|
| `sbtl.dll` | the engine; shipped next to the app exe (`src-tauri/tauri.conf.json`, `resources`) |
| `sbtl_import.lib` | its import library, for the linker |
| `VERSION` | the sbtl tag these were built from; must equal the tag pinned in `crates/sbtl-engine/Cargo.toml` (`scripts/release.mjs` checks it) |

`.cargo/config.toml` sets `SBTL_PREBUILT_DIR` to this folder, which makes
`sbtl-sys` link the DLL and copy it next to the executables it builds. The
Rust side compares `sbtl_abi_version()` with the value it was built against
when a session starts, so a DLL from a different sbtl version fails loudly
instead of misreading structs.

## Updating it (with every sbtl tag bump)

In the sbtl checkout, at the new tag:

```
cmake --preset nyaa
cmake --build --preset nyaa
```

then copy `build/nyaa/bin/sbtl.dll` and `build/nyaa/bin/sbtl_import.lib` here,
write the tag into `VERSION`, and commit all three with the Cargo tag change
(`chore: sbtl vX.Y.Z`). The `nyaa` preset leaves out sbtl's MP4/MKV indexer,
which nyaa-stream does not use (its `sbtl_media_*` calls report "unsupported").

## Working on sbtl and nyaa together

`npm run dev:sbtl-local` points cargo at `../sb_torrent` and at the DLL built
there (`build/nyaa/bin`), so rebuild it with the commands above after
changing sbtl's C code.

## Building without the DLL

Unset `SBTL_PREBUILT_DIR` (remove the line from `.cargo/config.toml`):
`sbtl-sys` then compiles the C sources itself with clang-cl, about 3 s on a
clean build, and nothing needs to be shipped beside the exe.
