# src-tauri/lib

Bundled native libraries (Tauri `bundle.resources`, installed next to the exe
under `lib/`).

`libmpv-2.dll` goes here (gitignored). Use a GPL build such as
https://github.com/zhongfly/mpv-winbuild (`mpv-dev-x86_64-*.7z` contains
`libmpv-2.dll`). `mpv-player` loads it at runtime, checking the Settings
override, then `<exe dir>`, `<exe dir>/lib`, then PATH; without it the app
falls back to the HLS player.
