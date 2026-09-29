import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useRef, useState } from "preact/hooks";
import { changesBetween, type ChangelogEntry } from "./changelog";

/** Set by `installUpdate` just before installing (see updater.ts). */
export const UPDATED_FROM_KEY = "nyaa-stream:updatedFrom";
/** The version the app last launched as - bridges updates from versions
 * whose updater didn't write `UPDATED_FROM_KEY` (0.3.1 and older). */
const LAST_VERSION_KEY = "nyaa-stream:lastVersion";
/** The newest version whose updater doesn't write the marker. */
const PRE_MARKER_VERSION = "0.3.1";

interface UpdatedFrom {
  from: string;
  to: string;
}

function readMarker(): UpdatedFrom | null {
  try {
    const raw = localStorage.getItem(UPDATED_FROM_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<UpdatedFrom>;
    return typeof parsed.from === "string" && typeof parsed.to === "string" ? { from: parsed.from, to: parsed.to } : null;
  } catch (err) {
    console.warn("[whats-new] unreadable update marker", { err: String(err) });
    return null;
  }
}

/** An update from a pre-marker version: no `lastVersion` recorded yet, but
 * the app already holds user data (library, progress, settings...) - a
 * fresh install has none. */
function bridgedMarker(running: string): UpdatedFrom | null {
  try {
    if (localStorage.getItem(LAST_VERSION_KEY) != null) return null;
    const hasData = Object.keys(localStorage).some((key) => key.startsWith("nyaa-stream:") && key !== UPDATED_FROM_KEY);
    return hasData ? { from: PRE_MARKER_VERSION, to: running } : null;
  } catch {
    return null;
  }
}

function recordVersion(running: string) {
  try {
    localStorage.setItem(LAST_VERSION_KEY, running);
  } catch (err) {
    console.warn("[whats-new] couldn't record the running version", { err: String(err) });
  }
}

function clearMarker() {
  try {
    localStorage.removeItem(UPDATED_FROM_KEY);
  } catch (err) {
    console.warn("[whats-new] couldn't clear the update marker", { err: String(err) });
  }
}

/** Shown once after an in-app update restarts the app: the changelog
 * entries between the old and the new version. Never on a fresh install or
 * a normal launch (no marker), and a marker from a failed install (its `to`
 * isn't the running version) is dropped silently. Updates from 0.3.1 and
 * older (no marker) are recognized by `bridgedMarker`. */
export function WhatsNew() {
  const [entries, setEntries] = useState<ChangelogEntry[]>([]);
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void getVersion()
      .then((running) => {
        const marker = readMarker() ?? bridgedMarker(running);
        recordVersion(running);
        if (!marker) return;
        clearMarker();
        if (running !== marker.to) {
          console.info("[whats-new] update marker doesn't match the running version, dropped", { ...marker, running });
          return;
        }
        const changes = changesBetween(marker.from, running);
        console.info("[whats-new] showing", { from: marker.from, to: running, entries: changes.length });
        setEntries(changes);
      })
      .catch((err) => console.warn("[whats-new] couldn't read the app version", { err: String(err) }));
  }, []);

  useEffect(() => {
    if (entries.length > 0) closeRef.current?.focus();
  }, [entries.length]);

  if (entries.length === 0) return null;
  const close = () => {
    console.debug("[whats-new] closed");
    setEntries([]);
  };

  return (
    <div
      class="export-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        class="export-dialog whats-new-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="whats-new-title"
        onKeyDown={(e) => {
          if (e.key === "Escape") close();
        }}
      >
        <h2 id="whats-new-title" class="export-title">
          What's new in nyaa-stream {entries[0].version}
        </h2>
        <div class="whats-new-body">
          {entries.map((entry) => (
            <section key={entry.version} class="whats-new-release">
              {entries.length > 1 && <h3>{entry.version}</h3>}
              {entry.features.length > 0 && (
                <>
                  <h4>New</h4>
                  <ul>
                    {entry.features.map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                  </ul>
                </>
              )}
              {entry.fixes.length > 0 && (
                <>
                  <h4>Improved and fixed</h4>
                  <ul>
                    {entry.fixes.map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                  </ul>
                </>
              )}
            </section>
          ))}
        </div>
        <div class="export-actions">
          <button ref={closeRef} class="button button-primary" onClick={close}>
            Got it
          </button>
        </div>
      </div>
    </div>
  );
}
