# v0.7.0 custom libmpv native build recipe

Status: local tools and selected source closure prepared; independent ASS
overlay sampling patches authored and added to the source lock, 2026-10-31.
No custom DLL has been compiled, so configure, compiler compatibility and runtime
behavior remain unproven.
SB approved mpv v0.41.0 stable, native Windows LLVM, baseline x64 and the
frozen scope in [ROADMAP.md](ROADMAP.md). Further changes require SB review.
SB selected keeping separate FFmpeg builds and proceeding with local tool
setup/source locking. SB also approved the four core dependency versions below.

The v0.7.0 setup implementation `scripts/prepare-mpv.py` bootstraps
isolated tools from `native/mpv/tools.lock.json`, records local tool identity,
and leaves all downloads/build output under `target/mpv-build/`. The source
lock is recorded separately in `native/mpv/sources.lock.json`, before compilation.
The source lock covers 21 repository archives, 15 Meson source/build definitions
and three native patches. Their archives/patches are SHA-256 checked; the two
ASS source patches are applied to the extracted mpv and libplacebo trees. This
proves source preparation, not successful configuration, media compilation or
runtime compatibility.
`scripts/prepare-mpv.py --sources-only` fetches locked source archives without
compiling them. SB approved LuaJIT and x264; their exact source/build-definition
revisions are recorded in the source lock.
Pinned Meson WrapDB definitions under `native/mpv/wraps/` will retain source
and build-patch hashes for portable dependencies. Source-only preparation may
use Meson's download operation to verify those archives without configuring
or compiling media libraries. Vendored graphics/shader sources are pinned
separately from their parent project's release archive.
The source audit includes the x264 compiler-name adjustment and the independent
ASS overlay sampler patches. All are pinned by SHA-256. Source preparation
checks and applies the mpv/libplacebo patches; configuration/compilation still
needs to prove the complete path.

## Build layout and toolchain

Develop on `release/0.7.0`. Use a dedicated `target/mpv-build/` tree for tools,
source archives/checkouts, static dependency installation, build logs and the
candidate DLL. Track the recipe and eventual build scripts/source lock in the
repository. Keep the installed DLL and existing HLS FFmpeg tree separate.

The machine has LLVM 23.1.1, Visual Studio 2022 Community, CMake 4.3.2,
Ninja 1.13.2, Python 3.11.9, GNU Make and Git for Windows. Local Meson 1.12.1,
NASM 2.16.03 and pkgconf 2.5.1 are prepared. pkgconf was compiled with native
LLVM, static linkage and `/MD`. Executable paths, tool versions and Windows
SDK/Visual Studio identities are recorded in `target/mpv-build/tools-manifest.json`.
Rerun with `python scripts/prepare-mpv.py`; archives are SHA-256 checked against
`native/mpv/tools.lock.json`. Detailed commands/build output are retained in
`target/mpv-build/logs/tool-commands.log`.

Use the Visual Studio amd64 developer environment, LLVM `clang`/`clang++`,
`lld-link`, `llvm-lib` and `llvm-rc`. FFmpeg/x264 configure builds may use
`clang-cl` through their MSVC-compatible build paths and Git Bash as a shell.
This remains a native Windows build. Verify these paths with configuration
checks before compiling the full dependency graph.

Build dependencies statically into the shared `libmpv-2.dll`; choose the
dynamic MSVC runtime consistently. Record any remaining runtime DLL imports.
Use baseline x64 compiler settings throughout, preserving runtime-dispatched
SIMD assembly. Reject `-march=native`, unconditional AVX/AVX2 and inherited
app `x86-64-v3` flags. The existing app's CPU requirement is unchanged.

## Source selections

SB approved mpv and the four core media dependency versions. This is not yet
a complete transitive lock or a tested media-stack compatibility claim.

| Component | Selection | Purpose |
|---|---|---|
| mpv | `v0.41.0` | Approved client API, playback and export core |
| FFmpeg | `n8.0.3` | Maintenance release in the 8.0 family used by mpv's release build recipe |
| libass | `0.17.5` | ASS rendering; release includes MSVC-mode Clang fixes |
| libplacebo | `v7.387.0` | GPU rendering, HDR and tone mapping |
| dav1d | `1.5.3` | Software AV1 decoder, alongside hardware decode |
| Meson | `1.12.1` | Local Python virtual environment; observed PyPI version |
| NASM | `2.16.03` | Prepared portable assembler for optimized codec paths |
| pkgconf | `pkgconf-2.5.1` | Native static-library dependency discovery |

Resolve full commit IDs or verified archive hashes before building. Include
LuaJIT, x264, FreeType, HarfBuzz, FriBidi, character encoding detection/conversion,
zlib, image libraries, LCMS2, shader compiler/conversion dependencies and GPU
headers/loaders in the same lock. Pin every transitive checkout, wrap and patch;
do not retain floating `master`/`main` or an unrecorded WrapDB update.

The resolved selection is in `native/mpv/sources.lock.json` and
`native/mpv/wraps/`. It includes Graphite shaping, compressed-font support,
DirectWrite font discovery, Windows character conversion, image libraries,
shader compilation/conversion and the three hardware-encoder interfaces.
Shaderc's own DEPS commits pin glslang and SPIR-V dependencies; libplacebo's
vendored sources use its exact gitlink revisions. Test/demo-only dependencies
are excluded under explicit options in the lock. mpv uses the classic libjpeg
API, so the separate TurboJPEG API and its PNG dependency are not required.

The NVIDIA header selection is `n11.1.5.3`, a version explicitly accepted by
FFmpeg 8.0.3 and containing AV1 NVDEC declarations, instead of raising the
NVENC API requirement to SDK 13. Driver/runtime behavior still needs testing.
libplacebo retains native Dolby Vision metadata reshaping; its optional raw
RPU parser is separate from the metadata supplied through mpv/FFmpeg.

The v0.41.0 upstream native script is a reference, not the final recipe: it
enables integrations and dependencies outside SB's approved scope and builds
standalone executables/static libmpv. Its default FFmpeg Meson port returned a
browser challenge during this audit. Proposed alternative: build upstream
FFmpeg's pinned source with its native MSVC-compatible toolchain and LLVM,
then expose installed static libraries to Meson through pinned pkgconf metadata.
That alternative still needs configuration proof; do not silently switch to
Docker, MinGW binaries or a prebuilt full-featured DLL if it fails.

## Configuration policy

- mpv: `libmpv=true`, `cplayer=false`, `buildtype=release`, `build-date=false`,
  `gpl=true`; shared mpv with static dependencies. Retain Lua through approved LuaJIT.
- Disable `javascript`, `cplugins`, `cdda`, `dvdnav`, `libbluray`, `dvbin`,
  `libavdevice`, `vapoursynth`, `sdl2-gamepad` and `libarchive`.
- Preserve WASAPI, D3D11, applicable Windows hardware decoding and GPU/OpenGL/
  Vulkan fallbacks; retain libass/font support, HDR/color handling and screenshots.
- Preserve subtitle rendering policy: render subtitles separately from scaled
  video, retain ASS/font/style overrides and outline blur, use light hinting for
  plain text and no hinting for authored ASS. The first candidate should match
  current rendering behavior against the recorded baseline.
- Independent ASS overlay sampling is implemented in the locked native patches
  `mpv-ass-overlay-sampling.patch` and `libplacebo-ass-overlay-sampling.patch`.
  mpv registers `ass-scale`, `ass-dscale`, `ass-aa`, `ass-blur`, `wns-ass-aa`,
  and `wns-ass-blur`; the built-in `sub-ass-scale-with-window` selects the
  window-resolution path. gpu-next passes ASS masks through libplacebo's filter
  sampler, with the video scaler untouched. HLS retains its existing 1.5x
  JASSUB path; clip export remains on its ASS style/encode path. These patches
  have not been compiled or visually verified yet.
- FFmpeg: explicitly select approved H.264, HEVC and AV1 software/hardware
  decoders, and AAC, AC-3, E-AC-3, FLAC and Opus audio decoders. Retain subtitle
  and image support needed by the app; do not confuse these with extra requested
  media codecs. Prefer dav1d rather than building redundant AV1 software libraries.
- Export: retain native AAC, software H.264 through approved x264 and H.264 NVENC,
  QSV and AMF, with required driver APIs/headers. Keep MP4 muxing and PNG/JPEG
  screenshot output. Validate required bitstream filters, parsers and conversion
  paths, including the app's `vf=format=yuv420p` export option.
- Network: retain HTTP/HTTPS, local files and their transport/TLS dependencies;
  remove approved unused inputs. Prefer Windows TLS facilities where supported.
- Remove approved unused effect categories. Build an explicit filter list from
  actual dependencies; retain format/scale, resampling, subtitle burn-in and
  timing functions plus mpv's pitch-preserving speed control.
- New subtitle-rendering feature approved by SB on 2026-10-31: add embedded-mpv
  Settings controls for ASS-only upscaling/downscaling filters, edge antialiasing
  and blur, and a window-resolution subtitle path with its own AA/blur values.
  These are user-adjustable controls in the existing Subtitles Settings section;
  they apply to embedded playback/screenshots. Clip export retains its selected
  ASS style through libmpv's encode path; the new overlay filters apply only to
  video outputs. The UI selects ASS rasterization at video or
  window resolution, separate up/down filters, and independent AA/blur values
  for both resolutions. Keep video `scale`/`dscale` behavior independent.
  These behaviors will use original mpv/libplacebo renderer patches; the
  `ass-*`/`wns-*` examples in mpv issue 2820 were not options in mpv v0.41.0;
  the locked native patches register and implement the selected controls.
  HLS keeps its existing 1.5x JASSUB path and does not consume these controls.
- Keep saved-frame screenshots aligned with the embedded playback renderer.
  Subtitle-burned clip export retains the selected ASS track/style, without
  claiming the playback-only overlay filters apply in encode mode.
- Input containers have not been approved for trimming. Preserve existing
  container coverage during this build; decoder selection still limits media.
- Exclude standalone ffmpeg/ffplay/ffprobe programs, demos and development
  binaries from the shipped artifact. Preserve build diagnostics and separate
  debugging material. Any test-only build is a separate, explicit verification step.

## Setup stages for SB review

1. Prepare local tools under `target/mpv-build/`: create a Python virtual
   environment, install pinned Meson there, fetch portable NASM, and prepare
   pinned pkgconf. Limit environment changes to the build process. Verify hashes
   and package contents; no global PATH, Python or default Rust changes.
2. Fetch source dependencies and resolve the complete source/tool lock. Record
   URLs, commit IDs, archive SHA-256, licenses and patches. Generate the exact
   enabled component lists and build commands from the audited dependency graph.
3. Review the lock and configuration results with SB before the full build.
   Configure/build static dependencies, then shared libmpv. Preserve logs and
   stop on missing required capabilities rather than silently disabling them.
4. Review the candidate's capabilities, imports, size and license/source bundle.
   Exercise it through the Settings DLL override before replacing the bundled DLL.

Stages 1 and source downloads/locking in stage 2 are authorized by SB's
selection to keep separate builds and prepare local tools/source lock.
Core dependency versions are approved. Full-build
execution, installed-player replacement and publication remain later steps.

Stage 1 passed: tool bootstrap completed and pkgconf compiled with LLVM.
Stage 2 previously passed for the original source closure. The updated
source-only command must verify archive and patch hashes and apply the ASS
patches before that stage can be marked complete again. Full media-stack
configuration/compilation remains the next approval gate. Required build
definitions may need adjustment when configuration is exercised; record and
review any changes to the source lock.

## Acceptance and evidence

- Check exported client symbols and event/property behavior required by
  `crates/mpv-player/src/libmpv.rs`; verify Lua loading explicitly.
- Confirm the build reports all retained decoders/encoders, outputs and renderers,
  and that approved exclusions are absent. Retain compiled configuration evidence.
- Exercise H.264 (including anime 10-bit sources), HEVC 10-bit, AV1, all retained
  audio codecs, ASS/font attachments, screenshots, HTTP Range reads, HDR/tone
  mapping, software/hardware decode and subtitle-burned MP4 export.
- Verify ASS filter choices independently change subtitle pixels while the
  source video remains unchanged; check video- and window-resolution subtitle
  modes, AA/blur bounds, scaling ratios, borders, anime typesetting/animation,
  Settings persistence and updates, high-DPI changes, fullscreen, embedded mpv,
  clip export and screenshots. Confirm HLS retains its current subtitle output.
- Compare playback/start/seek results against the original DLL on identical
  sources. The 2026-10-30 log contains 1,295 successful seek summaries; the
  slowest is 11,520 ms with 11,503 ms of recorded torrent waits. That is a
  stream-delivery case, not proof of a slow decoder.
- Reproduce E-AC-3 errors after seeks, Matroska parsing/queue warnings and four
  audio underruns from that log. Establish downloaded-input versus live-torrent
  behavior before attributing failures or improvements to the new DLL.
- Add build/decoder/GPU identity evidence for comparisons; the inspected log
  lacks those details. Measure CPU, GPU, memory and binary size rather than
  claiming gains from trimming alone. Verify HLS fallback remains functional.
- Retain complete licenses and corresponding source/build material for the exact
  distributed DLL. Native subtitle supersampling requires its own implementation
  and verification; a custom build alone does not establish that feature.

## Upstream references

- [mpv v0.41.0 Windows build](https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/compile-windows.md)
- [mpv v0.41.0 options](https://github.com/mpv-player/mpv/blob/v0.41.0/meson.options)
- [mpv native build reference](https://github.com/mpv-player/mpv/blob/v0.41.0/ci/build-win32.ps1)
- [FFmpeg releases](https://ffmpeg.org/releases/) and [native configuration](https://github.com/FFmpeg/FFmpeg/blob/n8.0.3/configure)
- [libass 0.17.5](https://github.com/libass/libass/releases/tag/0.17.5)
- [libplacebo v7.387.0](https://github.com/haasn/libplacebo/releases/tag/v7.387.0)
- [dav1d 1.5.3](https://github.com/videolan/dav1d/tree/1.5.3)
