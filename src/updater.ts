import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { UPDATED_FROM_KEY } from "./WhatsNew";

const FIRST_CHECK_DELAY_MS = 30_000;
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1_000;
const INITIAL_BACKOFF_MS = 15 * 60 * 1_000;
const MAX_BACKOFF_MS = CHECK_INTERVAL_MS;

export type UpdateState = {
  status: "idle" | "checking" | "available" | "downloading" | "verifying" | "ready" | "error";
  update: Update | null;
  progress: number | null;
  lastError: string | null;
  lastCheckedAt: number | null;
};

const initialState: UpdateState = {
  status: "idle",
  update: null,
  progress: null,
  lastError: null,
  lastCheckedAt: null,
};

let state = initialState;
let checkInFlight: Promise<Update | null> | null = null;
let prepareInFlight: Promise<void> | null = null;
let preparedUpdate: Update | null = null;
let schedulerStarted = false;
let nextAllowedAt = 0;
let failureBackoffMs = INITIAL_BACKOFF_MS;
let schedulerTimer: number | undefined;
const listeners = new Set<(state: UpdateState) => void>();

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function publish(patch: Partial<UpdateState>): void {
  state = { ...state, ...patch };
  listeners.forEach((listener) => listener(state));
}

export function getUpdateState(): UpdateState {
  return state;
}

export function subscribeUpdateState(listener: (state: UpdateState) => void): () => void {
  listeners.add(listener);
  listener(state);
  return () => listeners.delete(listener);
}

/** Checks the signed release manifest. Concurrent callers share one request. */
export function checkForUpdate(): Promise<Update | null> {
  if (!isTauri()) return Promise.resolve(null);
  if (checkInFlight) {
    console.debug("[updater] reusing in-flight check");
    return checkInFlight;
  }

  console.info("[updater] checking for a published update");
  publish({ status: "checking", lastError: null });
  checkInFlight = check({ timeout: 30_000 })
    .then((update) => {
      const checkedAt = Date.now();
      failureBackoffMs = INITIAL_BACKOFF_MS;
      nextAllowedAt = checkedAt + CHECK_INTERVAL_MS;
      const resolvedUpdate = preparedUpdate ?? update ?? null;
      const previousUpdate = state.update;
      if (previousUpdate && previousUpdate !== resolvedUpdate && previousUpdate !== preparedUpdate) {
        void previousUpdate.close().catch((error: unknown) => console.debug("[updater] couldn't release a stale update", { error: String(error) }));
      }
      if (update && update !== resolvedUpdate) {
        void update.close().catch((error: unknown) => console.debug("[updater] couldn't release a duplicate update", { error: String(error) }));
      }
      publish({
        status: resolvedUpdate ? (preparedUpdate?.version === resolvedUpdate.version ? "ready" : "available") : "idle",
        update: resolvedUpdate,
        progress: null,
        lastError: null,
        lastCheckedAt: checkedAt,
      });
      console.info("[updater] check completed", { available: resolvedUpdate?.version ?? null });
      return resolvedUpdate;
    })
    .catch((error: unknown) => {
      const checkedAt = Date.now();
      nextAllowedAt = checkedAt + failureBackoffMs;
      failureBackoffMs = Math.min(MAX_BACKOFF_MS, failureBackoffMs * 2);
      const message = error instanceof Error ? error.message : String(error);
      publish({
        status: state.update ? (preparedUpdate ? "ready" : "available") : "error",
        lastError: message,
        lastCheckedAt: checkedAt,
      });
      console.warn("[updater] check failed", { retryInMs: nextAllowedAt - checkedAt, error: message });
      throw error;
    })
    .finally(() => {
      checkInFlight = null;
    });
  return checkInFlight;
}

/** Download and signature verification happen while the running app stays open. */
export function prepareUpdate(update = state.update): Promise<void> {
  if (!update) return Promise.reject(new Error("There is no update to download"));
  if (preparedUpdate?.version === update.version) return Promise.resolve();
  if (prepareInFlight) return prepareInFlight;

  console.info("[updater] preparing update", { version: update.version });
  publish({ status: "downloading", progress: null, lastError: null, update });
  let total = 0;
  let received = 0;
  prepareInFlight = update
    .download((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
        received = 0;
        publish({ status: "downloading", progress: null });
        console.debug("[updater] download started", { version: update.version, totalBytes: total || null });
      } else if (event.event === "Progress") {
        received += event.data.chunkLength;
        publish({
          status: "downloading",
          progress: total > 0 ? Math.min(100, Math.floor((received / total) * 100)) : null,
        });
      } else if (event.event === "Finished") {
        publish({ status: "verifying", progress: null });
        console.info("[updater] download finished; updater signature verification is completing", { version: update.version });
      }
    })
    .then(() => {
      preparedUpdate = update;
      publish({ status: "ready", update, progress: null, lastError: null });
      console.info("[updater] update downloaded and verified", { version: update.version });
    })
    .catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      publish({ status: "error", progress: null, lastError: message, update });
      console.error("[updater] download or signature verification failed", { version: update.version, error: message });
      throw error;
    })
    .finally(() => {
      prepareInFlight = null;
    });
  return prepareInFlight;
}

/** Starts the platform updater only after an explicit restart action. */
export async function applyPreparedUpdate(): Promise<void> {
  if (!preparedUpdate) throw new Error("Download the update before restarting");
  const update = preparedUpdate;
  let marker: string | null = null;
  console.info("[updater] applying prepared update", { version: update.version });
  try {
    const from = await getVersion();
    marker = JSON.stringify({ from, to: update.version });
    localStorage.setItem(UPDATED_FROM_KEY, marker);
    console.debug("[updater] What's new marker stored", { from, to: update.version });
  } catch (error) {
    console.warn("[updater] couldn't store the What's new marker", { error: String(error) });
  }

  try {
    // On Windows, Tauri exits the app when it launches its signed installer.
    // Its NSIS updater mode relaunches after installation; elsewhere this
    // explicit relaunch starts the newly installed version.
    await update.install();
    console.info("[updater] installer returned; relaunching into the updated app");
    await relaunch();
  } catch (error) {
    if (marker) {
      try {
        if (localStorage.getItem(UPDATED_FROM_KEY) === marker) localStorage.removeItem(UPDATED_FROM_KEY);
      } catch (storageError) {
        console.warn("[updater] couldn't clear the update marker after install failure", { error: String(storageError) });
      }
    }
    const message = error instanceof Error ? error.message : String(error);
    publish({ status: "ready", lastError: message, update });
    console.error("[updater] installer launch failed", { version: update.version, error: message });
    throw error;
  }
}

/** Starts quiet checks: 30s after readiness, then every six hours at most. */
export function startBackgroundUpdateChecks(): () => void {
  if (!isTauri() || schedulerStarted) return () => undefined;
  schedulerStarted = true;
  console.info("[updater] background checks enabled", { firstCheckDelayMs: FIRST_CHECK_DELAY_MS, intervalMs: CHECK_INTERVAL_MS });

  const schedule = (delay: number) => {
    if (schedulerTimer !== undefined) window.clearTimeout(schedulerTimer);
    schedulerTimer = window.setTimeout(() => {
      if (Date.now() < nextAllowedAt) {
        schedule(nextAllowedAt - Date.now());
        return;
      }
      void checkForUpdate().catch(() => undefined).finally(() => schedule(Math.max(1_000, nextAllowedAt - Date.now())));
    }, Math.max(1_000, delay));
  };
  const onResume = () => {
    if (document.visibilityState !== "visible" || Date.now() < nextAllowedAt || checkInFlight) return;
    console.info("[updater] eligible resume check");
    void checkForUpdate().catch(() => undefined).finally(() => schedule(Math.max(1_000, nextAllowedAt - Date.now())));
  };

  nextAllowedAt = Date.now() + FIRST_CHECK_DELAY_MS;
  schedule(FIRST_CHECK_DELAY_MS);
  window.addEventListener("focus", onResume);
  document.addEventListener("visibilitychange", onResume);
  return () => {
    if (schedulerTimer !== undefined) window.clearTimeout(schedulerTimer);
    window.removeEventListener("focus", onResume);
    document.removeEventListener("visibilitychange", onResume);
    schedulerStarted = false;
    console.debug("[updater] background checks stopped");
  };
}
