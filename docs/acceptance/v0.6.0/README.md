# v0.6.0 Windows installer acceptance

Win10Lite acceptance completed on 2026-10-30. Each report contains 17 checks
with zero failures:

| Report | Upgrade path | Result |
|---|---|---|
| [NSIS](report-Nsis.json) | v0.5.8 NSIS → v0.6.0 NSIS | 17/17 passed |
| [MSI](report-Msi.json) | v0.5.8 MSI → v0.6.0 MSI | 17/17 passed |
| [MSI to NSIS](report-MsiToNsis.json) | v0.5.8 MSI → v0.6.0 NSIS | 17/17 passed |

All v0.6.0 installs reported version `0.6.0`, included app-local
`VCRUNTIME140.dll` version `14.44.35211.0`, launched successfully, preserved a
4 KiB release database and a user-data marker through upgrade, and removed app
files while retaining the marker on uninstall. WebView2 `156.0.4314.56` was
present after each run.

The runner staged that runtime beside v0.5.8 only as a test shim so the legacy
app could launch and create its profile before upgrade; the current installers
carry their own runtime. The reports prove direct installer upgrade and
uninstall behavior. They do not prove the in-app updater's signed download,
staging, restart and relaunch path. WebView2 presence was checked after the
install sequence, not before; therefore a clean-machine bootstrap is not
established. Interactive install choices, UAC/elevation, repair, and Add/Remove
Programs registration were also not covered.
