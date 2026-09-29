import { useEffect, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useFullscreen } from "./fullscreen";
import { BrandMark } from "./icons";
import { usePip } from "./pip";

// Replaces the system title bar (main window is `decorations: false`). The
// strip blends with the app bar below it; in the player it fades with the
// player's top controls (CSS in App.css), and it is hidden in fullscreen
// and PiP.

const isTauri = "__TAURI_INTERNALS__" in window;

function run(action: "minimize" | "toggleMaximize" | "close") {
  console.debug("[titlebar] action", { action });
  getCurrentWindow()
    [action]()
    .catch((err) => console.warn("[titlebar] action failed", { action, err: String(err) }));
}

export function TitleBar() {
  const fullscreen = useFullscreen();
  const pip = usePip();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    const win = getCurrentWindow();
    const sync = () =>
      win
        .isMaximized()
        .then(setMaximized)
        .catch((err) => console.warn("[titlebar] isMaximized failed", { err: String(err) }));
    void sync();
    const unlisten = win.onResized(() => void sync());
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  if (!isTauri || fullscreen || pip) return null;

  return (
    <div class="titlebar" data-tauri-drag-region>
      <div class="titlebar-title" data-tauri-drag-region>
        <BrandMark />
        <span data-tauri-drag-region>nyaa-stream</span>
      </div>
      <div class="titlebar-controls">
        <button type="button" class="titlebar-btn" aria-label="Minimize" title="Minimize" onClick={() => run("minimize")}>
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5h8" /></svg>
        </button>
        <button
          type="button"
          class="titlebar-btn"
          aria-label={maximized ? "Restore" : "Maximize"}
          title={maximized ? "Restore" : "Maximize"}
          onClick={() => run("toggleMaximize")}
        >
          {maximized ? (
            <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 2.5h6v6h-6zM4 2.5V1h5v5H7.5" /></svg>
          ) : (
            <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5h7v7h-7z" /></svg>
          )}
        </button>
        <button type="button" class="titlebar-btn titlebar-close" aria-label="Close" title="Close" onClick={() => run("close")}>
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" /></svg>
        </button>
      </div>
    </div>
  );
}
