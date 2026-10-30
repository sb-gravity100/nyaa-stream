# App-local Visual C++ runtime

`vcruntime140.dll` is the x64 Microsoft Visual C++ v14 redistributable runtime
imported by `native/sbtl/sbtl.dll`. `tauri.conf.json` installs it beside
`nyaa-stream.exe`, where Windows resolves the native bridge dependency on clean
machines without a separately installed Visual C++ Redistributable.

The checked-in binary is version 14.44.35211.0, copied from the licensed
Visual Studio 2022 Community redist directory
`VC/Redist/MSVC/14.44.35112/x64/Microsoft.VC143.CRT`. Keep this file current
with the latest supported x64 v14 Redistributable when updating the MSVC build
toolchain. Microsoft permits app-local deployment of individual redistributable
DLLs under the Visual Studio license terms, but recommends central deployment
for easier servicing:

- <https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files>
- <https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist>
