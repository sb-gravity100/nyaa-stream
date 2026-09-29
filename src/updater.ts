import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { UPDATED_FROM_KEY } from "./WhatsNew";

// Auto-update via GitHub Releases (tauri-plugin-updater): `check` reads the
// signed latest.json at the endpoint in tauri.conf.json.

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** The pending update, or null when up to date (or outside the app). */
export async function checkForUpdate(): Promise<Update | null> {
  if (!isTauri()) return null;
  console.debug("[updater] checking");
  const update = await check();
  console.info("[updater] check done", { available: update?.version ?? null });
  return update;
}

/** Downloads and installs `update`, then restarts into the new version. */
export async function installUpdate(update: Update, onProgress?: (percent: number | null) => void): Promise<void> {
  let total = 0;
  let received = 0;
  console.info("[updater] installing", { version: update.version });
  // Written before installing: on Windows the installer takes over inside
  // downloadAndInstall and the app may never reach relaunch(). The next
  // launch shows the What's new dialog (WhatsNew.tsx).
  try {
    const from = await getVersion();
    localStorage.setItem(UPDATED_FROM_KEY, JSON.stringify({ from, to: update.version }));
    console.debug("[updater] update marker stored", { from, to: update.version });
  } catch (err) {
    console.warn("[updater] couldn't store the update marker", { err: String(err) });
  }
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength ?? 0;
    else if (event.event === "Progress") {
      received += event.data.chunkLength;
      onProgress?.(total ? Math.min(100, Math.round((received / total) * 100)) : null);
    }
  });
  console.info("[updater] installed, relaunching");
  await relaunch();
}
