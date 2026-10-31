# v0.7.0 custom libmpv native build recipe

Status: the locked full build pipeline completed successfully, and the x64
candidate is staged in `src-tauri/lib/` with its MSVC/Vulkan runtime DLLs. The
upstream DLL remains backed up at `target/mpv-build/app-bundle-original/`; the
previous custom candidate is at `target/mpv-build/app-bundle-pre-supersampling/`.
Runtime checks passed for API/options, H.264/AAC and AV1 playback, and the
subtitle-burned x264/AAC export. A controlled 1080p `gpu-next` window check
confirmed visible subtitles and a visible ASS blur change. Through the Rust
`EmbeddedMpv` wrapper, 720p playback at 1 second passed with video-stage
blending and 2× window supersampling enabled. The 720p-in-1080p and native 1080p
image comparisons both kept subtitles visible; the supersampling mode changed
24,130 and 18,382 pixels in the subtitle regions, respectively. The Rust
`encode_clip` path produced a 1920×1080 H.264/AAC MP4 with the subtitle visibly
burned in. A package-layout probe confirmed that Vulkan/MSVCP/VCRUNTIME140_1
load from `src-tauri/lib/` and VCRUNTIME140 loads beside the executable. Full
Tauri UI playback/export, clean-Windows dependency behavior, other subtitle
controls/display modes, and hardware codec paths remain unverified. The current
FFmpeg config currently enables the D3D11VA device API but disables its
codec-specific decoders. SB approved the next candidate to enable both D3D11VA
and D3D11VA2 hwaccels for H.264, HEVC and AV1 only, retaining software fallback
and leaving other decode APIs out. Hardware decode still needs a rebuilt DLL
and runtime verification on this machine. The custom build's primary playback
goal remains improving torrent data delivery into mpv: no controlled
candidate-versus-upstream Range/read-ahead/seek benchmark has yet proven a
reduction in torrent wait, startup time, rebuffering or seek latency.
SB approved mpv v0.41.0 stable, native Windows LLVM, baseline x64 and the
frozen scope in [ROADMAP.md](ROADMAP.md). Further changes require SB review.
SB selected keeping separate FFmpeg builds and proceeding with local tool
setup/source locking. SB also approved the four core dependency versions below.

The v0.7.0 setup implementation `scripts/prepare-mpv.py` bootstraps
isolated tools from `native/mpv/tools.lock.json`, records local tool identity,
and leaves all downloads/build output under `target/mpv-build/`. The source
lock is recorded separately in `native/mpv/sources.lock.json`, before compilation.
The source lock covers 21 repository archives, 15 Meson source/build definitions
and seven native patches. Their archives/patches are SHA-256 checked; the two
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
candidate DLL. Keep the tracked recipe/source lock, installed DLL and existing
HLS FFmpeg tree separate.

Run `python scripts/build-mpv.py` from the repository root to prepare the locked
tools and sources, build the approved static dependencies and separate FFmpeg,
then configure and compile the shared candidate DLL. The command writes detailed
logs and the candidate to `target/mpv-build/`; it does not install or replace
the app's bundled DLL. `scripts/mpv-build/` contains the individual build stages.

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

SB approved mpv and the four core media dependency versions. The complete
transitive source lock is now verified; compatibility evidence is recorded
separately below and does not cover every codec or GPU path.

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
NVENC API requirement to SDK 13. The candidate configuration resolves this
locked header version; driver/runtime behavior still needs testing.
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
- Independent ASS filtering and true 2× window supersampling are implemented in
  the locked native patches `mpv-ass-overlay-sampling.patch` and
  `libplacebo-ass-overlay-sampling.patch`. When the custom options are available,
  `src/subtitles.ts` selects `blend-subtitles=video`. gpu-next rasterizes ASS at
  2× the window OSD size, downsamples the mask into a video-resolution texture
  with the selected filter, then maps it onto the source-sized overlay. The
  build recipe syncs the separately locked libplacebo source into mpv's Meson
  subproject before configuration, so compilation uses the patched source.
  HLS keeps its existing 1.5× JASSUB path; clip export keeps its ASS encode path.
- FFmpeg: explicitly select approved H.264, HEVC and AV1 software decoders plus
  D3D11VA/D3D11VA2 hwaccels for those three codecs only; software fallback stays
  available. Leave other hardware decode APIs out. Retain AAC, AC-3, E-AC-3,
  FLAC and Opus audio decoders and subtitle
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
Core dependency versions and the first full build are approved. Installed-player
replacement and publication remain later steps.

Stage 1 passed: tool bootstrap completed and pkgconf compiled with LLVM.
Stage 2 passed: the tracked runner verified the full source closure and all
seven native patches. Stage 3 passed: the runner built the static dependencies,
separate FFmpeg and shared candidate DLL with every stage exit code 0. Runtime
checks passed for libmpv initialization/options, H.264/AAC and AV1 playback, and
x264/AAC MP4 export with a visibly burned subtitle. Installing the DLL in the
app, visual comparison of playback-only subtitle filters, hardware-path
acceptance and broader codec/media coverage remain pending.

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
  sources. Load traces must include a bounded sample of Range offsets and
  lengths (first and last 16 requests) with engine waits and byte totals. The
  2026-10-30 log contains 1,295 successful seek summaries; the
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
