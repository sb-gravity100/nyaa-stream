import type { AnimeMedia } from "./types";

// localStorage, not a Rust-side store: PLAN.md marks the real persistence
// layer (sqlite vs. flat file) as an explicit open TBD, and this is a test
// page — localStorage needs no backend work, behaves identically in the
// real Tauri webview and the browser-only dev fallback, and doesn't
// commit to that unresolved decision.
const STORAGE_KEY = "nyaa-stream:library";

export function getLibrary(): AnimeMedia[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch (err) {
    console.error("[library] failed to read from localStorage", { err });
    return [];
  }
}

function saveLibrary(items: AnimeMedia[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch (err) {
    console.error("[library] failed to write to localStorage", { err });
  }
}

export function isInLibrary(id: number, library: AnimeMedia[]): boolean {
  return library.some((a) => a.id === id);
}

export function addToLibrary(anime: AnimeMedia, library: AnimeMedia[]): AnimeMedia[] {
  if (isInLibrary(anime.id, library)) return library;
  const next = [...library, anime];
  saveLibrary(next);
  return next;
}

export function removeFromLibrary(id: number, library: AnimeMedia[]): AnimeMedia[] {
  const next = library.filter((a) => a.id !== id);
  saveLibrary(next);
  return next;
}
