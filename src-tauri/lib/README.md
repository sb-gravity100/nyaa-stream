# src-tauri/lib

Bundled native libraries (Tauri `bundle.resources`, installed next to the exe
under `lib/`).

The current local candidate is the tailored mpv v0.41.0 x64 build from
`scripts/build-mpv.py` (gitignored). Keep these app-local dependencies beside
it so Windows can load the DLL on machines without the matching runtimes:

- `MSVCP140.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` — x64 MSVC 14.44.35211.0
- `vulkan-1.dll` — built from the locked Vulkan-Loader source; license is in
  `vulkan-loader-LICENSE.txt`

The MSVC runtime files are licensed app-local redistributables; see
`src-tauri/runtime/README.md` for the source and Microsoft terms.

The upstream DLL is preserved at
`target/mpv-build/app-bundle-original/libmpv-2.dll`; earlier custom builds are
preserved under `target/mpv-build/app-bundle-pre-supersampling/` and
`target/mpv-build/app-bundle-pre-final-pipeline/` and
`target/mpv-build/app-bundle-pre-d3d11va/` and
`target/mpv-build/app-bundle-pre-composition/`. `mpv-player`
loads the Settings override first, then `<exe dir>`, `<exe dir>/lib`, and PATH;
on Windows it searches dependencies beside the selected DLL. Without a usable
libmpv or its dependencies the app falls back to the HLS player.
