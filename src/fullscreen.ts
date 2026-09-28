import { useEffect, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTypingTarget } from "./keyboard";

// App-wide fullscreen: the native window itself, not an element inside the
// webview - the embedded mpv draws into the window, so only that grows the
// video. One state shared by the F hotkey and the player's button/Esc.

let current = false;
const listeners = new Set<(on: boolean) => void>();

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function isFullscreen(): boolean {
  return current;
}

export function setFullscreen(on: boolean): void {
  if (on === current) return;
  console.debug("[fullscreen] set", { on });
  current = on;
  for (const listener of listeners) listener(on);
  if (isTauri()) {
    getCurrentWindow()
      .setFullscreen(on)
      .catch((err) => console.warn("[fullscreen] failed", { on, err: String(err) }));
  } else if (on) {
    // Dev-only browser preview.
    void document.documentElement.requestFullscreen?.().catch(() => undefined);
  } else if (document.fullscreenElement) {
    void document.exitFullscreen();
  }
}

export function toggleFullscreen(): void {
  setFullscreen(!current);
}

export function useFullscreen(): boolean {
  const [on, setOn] = useState(current);
  useEffect(() => {
    listeners.add(setOn);
    setOn(current);
    return () => {
      listeners.delete(setOn);
    };
  }, []);
  return on;
}

/** F toggles fullscreen anywhere in the app, unless typing. Also adopts
 * the window's real state - a page reload keeps the window fullscreen. */
export function installFullscreenHotkey(): void {
  if (isTauri()) {
    getCurrentWindow()
      .isFullscreen()
      .then((on) => {
        current = on;
        for (const listener of listeners) listener(on);
        console.debug("[fullscreen] initial state", { on });
      })
      .catch((err) => console.warn("[fullscreen] state read failed", { err: String(err) }));
  }
  window.addEventListener("keydown", (e) => {
    if (e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key.toLowerCase() !== "f" || isTypingTarget(e.target)) return;
    e.preventDefault();
    toggleFullscreen();
  });
  console.debug("[fullscreen] F hotkey installed");
}
