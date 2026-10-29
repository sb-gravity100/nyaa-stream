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

## Direction

Keep NSIS as the packaging and install engine, with a custom NSIS template for
the reviewed dusk-indigo/sakura first-install experience. Keep the Tauri
updater's signed payload for in-app updates; do not route it through the
interactive first-install pages. Preserve `passive` until isolated tests show
that every supported per-user/per-machine install can use `quiet` safely.
The current updater UI stages and verifies in-app, then asks the user to
restart; the Windows apply step still needs an isolated-machine decision about
the passive installer progress window versus a dedicated helper.

## Still requires an isolated Windows environment

No installer was run against SB's active profile. These checks remain open:

- Build the custom template and inspect its generated NSIS script and signed
  updater artifact.
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
