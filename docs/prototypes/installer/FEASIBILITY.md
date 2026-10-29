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
Start Menu folder/shortcut choice and desktop shortcut choice. The real NSIS
file-copy page reports progress/errors, and its completion page can launch the
app. The generated header/sidebar artwork and the installer use the
dusk-indigo/sakura palette. Rebuild artwork with
`powershell -File scripts/make-nsis-art.ps1`.

Tauri's signed updater payload and `passive` mode remain separate from the
interactive page. The custom page explicitly skips passive updater runs, so
`/UPDATE` continues using Tauri's existing migration and restart behavior.
Do not switch to `quiet` without isolated proof of elevation and install-mode
behavior.

The Windows NSIS bundle completed and produced
`target/release/bundle/nsis/nyaa-stream_0.6.0_x64-setup.exe`. That build used an
empty Start Menu folder setting and emitted a warning about the missing MUI
default-folder macro. The configuration now sets the `nyaa-stream` default so
the shortcut choices appear and can be restored from the uninstall registry;
that final configuration edit was not rebuilt. The artifact was not signed
because the private signing key was not supplied. The interactive page,
install, error recovery, uninstaller and updater flow have not been exercised
on an isolated Windows profile.

## Still requires an isolated Windows environment

No installer was run against SB's active profile. These checks remain open:

- Rebuild and inspect the final template with the configured Start Menu
  folder, then build/inspect a signed updater artifact using the release key.
- Install fresh with Start menu/desktop choices, then verify launch, shortcuts,
  uninstall registration and WebView2 bootstrap on a clean Windows machine.
- Upgrade both existing NSIS and MSI installs; verify their install paths,
  shortcuts, registration, app data, release database, settings and resume
  progress survive. Verify repair, uninstall and reinstall.
- Apply a signed update from each supported install type, including interrupted
  download, invalid signature, failed preparation, deferred restart and
  successful relaunch/version detection.
- Verify elevation behavior and cancellation/recovery while NSIS is writing.

## Tauri references

- [Windows installer customization](https://v2.tauri.app/distribute/windows-installer/)
- [Updater API and Windows install modes](https://v2.tauri.app/plugin/updater/)
- [JavaScript updater download/install API](https://v2.tauri.app/reference/javascript/updater/)
