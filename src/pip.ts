import { useEffect, useState } from "preact/hooks";
import { LogicalPosition, LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { setFullscreen } from "./fullscreen";

// Picture-in-picture: the embedded mpv draws into the window itself (no
// <video> for Chromium's PiP to lift out), so PiP shrinks the whole native
// window to a small always-on-top 16:9 corner and restores it on exit.

const PIP_WIDTH = 480;
const PIP_HEIGHT = 270;
const PIP_MARGIN = 24;

let current = false;
const listeners = new Set<(on: boolean) => void>();
let saved: { size: LogicalSize; position: LogicalPosition; maximized: boolean } | null = null;

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function emit(on: boolean): void {
  current = on;
  for (const listener of listeners) listener(on);
}

export function isPip(): boolean {
  return current;
}

export async function setPip(on: boolean): Promise<void> {
  if (on === current || !isTauri()) return;
  console.debug("[pip] set", { on });
  const win = getCurrentWindow();
  try {
    if (on) {
      setFullscreen(false);
      const [scale, size, position, maximized] = await Promise.all([
        win.scaleFactor(),
        win.innerSize(),
        win.outerPosition(),
        win.isMaximized(),
      ]);
      saved = {
        size: size.toLogical(scale),
        position: position.toLogical(scale),
        maximized,
      };
      if (maximized) await win.unmaximize();
      await win.setSize(new LogicalSize(PIP_WIDTH, PIP_HEIGHT));
      const screenW = window.screen.availWidth;
      const screenH = window.screen.availHeight;
      await win.setPosition(
        new LogicalPosition(screenW - PIP_WIDTH - PIP_MARGIN, screenH - PIP_HEIGHT - PIP_MARGIN - 32),
      );
      await win.setAlwaysOnTop(true);
      emit(true);
    } else {
      await win.setAlwaysOnTop(false);
      if (saved) {
        await win.setSize(saved.size);
        await win.setPosition(saved.position);
        if (saved.maximized) await win.maximize();
        saved = null;
      }
      emit(false);
    }
  } catch (err) {
    console.warn("[pip] failed", { on, err: String(err) });
  }
}

export function togglePip(): void {
  void setPip(!current);
}

export function usePip(): boolean {
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
