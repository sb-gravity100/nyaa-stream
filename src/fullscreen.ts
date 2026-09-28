import { useEffect, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

const TEXT_INPUT_TYPES = new Set(["", "text", "search", "email", "url", "password", "number", "tel"]);

/** Typing into a field - where F is a letter, not a hotkey. */
function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(target.type.toLowerCase());
  return false;
}

/** F toggles fullscreen anywhere in the app, unless typing. */
export function installFullscreenHotkey(): void {
  window.addEventListener("keydown", (e) => {
    if (e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key.toLowerCase() !== "f" || isTypingTarget(e.target)) return;
    e.preventDefault();
    toggleFullscreen();
  });
  console.debug("[fullscreen] F hotkey installed");
}
