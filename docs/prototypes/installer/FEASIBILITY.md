# Installer and updater feasibility

Checked against the current Tauri v2 documentation on 2026-10-29.

## What the current stack supports

- Tauri can replace the complete NSIS `.nsi` template. The existing artwork
  replacement is only WiX/MSI branding and does not change the NSIS wizard.
- The updater API separates `Update.download()` from `Update.install()`. The
  downloaded payload is checked against the configured updater public key
  before `download()` resolves, so release notes and byte progress can stay in
  nyaa-stream until SB chooses to restart.
- On Windows, `install()` launches the updater installer and exits the app.
  Tauri documents this as a Windows installer limitation. It cannot safely
  replace running files in place. The current `passive` mode gives the user a
  small installer progress window; `quiet` removes that feedback and cannot
  request admin privileges, so it is limited to per-user installs or an app
  already running elevated.
- The release config already enables Tauri updater artifacts and points at the
  existing signed GitHub manifest. The updater uses the signed NSIS/MSI
  artifact and can remain separate from the interactive first-install UI.

## Implemented installer path

NSIS remains the install engine. `src-tauri/installer/installer.nsi` now adds a
custom `nsDialogs` page that combines destination selection, folder browsing,
Start Menu folder/shortcut choice and desktop shortcut choice. Existing
shortcut choices are restored on interactive upgrades; unchecked choices
remove only links that target the app. Passive updater installs retain the
stored Start Menu folder. The real NSIS file-copy page reports progress/errors,
with the app palette, branded headings and sakura progress bar; its completion
page can launch the app. The generated header/sidebar artwork also uses the
dusk-indigo/sakura palette. Rebuild artwork with
`powershell -File scripts/make-nsis-art.ps1`.

Tauri's signed updater payload and `passive` mode remain separate from the
interactive page. The custom page explicitly skips passive updater runs, so
`/UPDATE` continues using Tauri's existing migration and restart behavior.
Do not switch to `quiet` without isolated proof of elevation and install-mode
behavior.

The current v0.6.0 code built both
`target/release/bundle/nsis/nyaa-stream_0.6.0_x64-setup.exe` and
`target/release/bundle/msi/nyaa-stream_0.6.0_x64_en-US.msi`, with updater
signatures generated from the local release key. The NSIS script compiled
successfully with the configured `nyaa-stream` Start Menu folder and custom
install choices. These artifacts were not uploaded. Win10Lite acceptance
passed the v0.5.8 NSIS→v0.6.0 NSIS, MSI→MSI and MSI→NSIS direct-installer paths,
17 checks per route, including current-app launch, local runtime presence,
database/marker retention and uninstall. Raw reports are in
[`docs/acceptance/v0.6.0/`](../../acceptance/v0.6.0/).

## Still requires an isolated Windows environment

No installer was run against SB's active profile. These checks remain open:

- Verify clean-machine WebView2 bootstrap, interactive shortcut/install-path
  choices, UAC/elevation, repair and Add/Remove Programs registration.
- Preserve settings and playback/resume state through existing-install
  upgrades and controlled restarts, beyond the database/marker checks already
  covered by the installer matrix.
- Apply a signed update from each supported install type, including interrupted
  download, invalid signature, failed preparation, deferred restart and
  successful relaunch/version detection.
- Verify elevation behavior and cancellation/recovery while NSIS is writing.

## Tauri references

- [Windows installer customization](https://v2.tauri.app/distribute/windows-installer/)
- [Updater API and Windows install modes](https://v2.tauri.app/plugin/updater/)
- [JavaScript updater download/install API](https://v2.tauri.app/reference/javascript/updater/)
