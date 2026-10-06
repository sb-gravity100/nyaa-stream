import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import { labelOf } from "./episodeParser";
import type { AnimeTitle, NyaaResult } from "./types";

// Module-level cache + in-flight dedupe, same pattern as kitsu.ts. Values
// are image URLs (`thumb://` from the backend's disk cache, or a blob URL
// for a frame saved this session) - never base64. `null` means "tried, no
// thumbnail available" (no matching release found, or the capture itself
// failed) so a card doesn't retry every re-render.
const cache = new Map<string, string | null>();

type SavedListener = (key: string, url: string) => void;
const savedListeners = new Set<SavedListener>();

/** Called with `{anilistId}-{episode}` and the new URL whenever the player
 * saves a last frame - the encode is async, so a view that looked the
 * thumbnail up as the player closed would otherwise keep the old one. */
export function subscribeFrameSaved(listener: SavedListener): () => void {
  savedListeners.add(listener);
  return () => savedListeners.delete(listener);
}

/** `cached_torrent_thumbnail`'s reply. */
interface CachedThumbnail {
  url: string | null;
  /** A capture failed recently (backend-side marker) - don't search nyaa
   * for another attempt yet. */
  failedRecently: boolean;
}
const inFlight = new Map<string, Promise<string | null>>();

function cacheKey(anilistId: number, episode: number): string {
  return `${anilistId}-${episode}`;
}

// Picks the best candidate release to pull a frame from: must be a
// standalone episode release (not a batch — batches are usually one file
// per episode too, but we have no per-file listing from nyaa itself to
// pick the right one out of a multi-file torrent, so skipping batches
// avoids guessing) whose parsed episode number matches, then the one with
// the most seeders for the fastest/most reliable minimal download.
function pickCandidate(results: NyaaResult[], episode: number): NyaaResult | null {
  const matches = results.filter((r) => {
    const label = labelOf(r.label, r.title);
    return label.kind === "episode" && label.number === episode;
  });
  if (matches.length === 0) return null;
  return matches.reduce((best, r) => (r.seeders > best.seeders ? r : best));
}

/** Saves the player's last frame (a JPEG blob) as the episode's thumbnail
 * (disk cache, shared with torrent captures) and makes it the in-memory
 * entry right away, so the home page shows it as soon as the player
 * closes. The bytes go over IPC raw, not base64. */
export async function saveFrameThumbnail(anilistId: number, episode: number, jpeg: Blob): Promise<void> {
  const key = cacheKey(anilistId, episode);
  // Blob URLs are never revoked: cards may still hold this one, and it's
  // one small frame per closed episode.
  const objectUrl = URL.createObjectURL(jpeg);
  cache.set(key, objectUrl);
  for (const listener of savedListeners) listener(key, objectUrl);
  if (!isTauriAvailable()) return;
  try {
    const bytes = new Uint8Array(await jpeg.arrayBuffer());
    const url = await invoke<string>("save_frame_thumbnail", bytes, { headers: { "x-cache-key": key } });
    console.debug("[save_frame_thumbnail] saved", { anilistId, episode, bytes: bytes.length, url });
  } catch (err) {
    console.warn("[save_frame_thumbnail] failed", { anilistId, episode, err: String(err) });
  }
}

/** A previously captured frame from the backend's disk cache, or null -
 * never searches nyaa or captures, so it's cheap enough to ask for every
 * card as soon as it's listed rather than after the Kitsu/AniList
 * fallback chain has run. A hit also fills this module's cache. */
export async function cachedTorrentThumbnail(anilistId: number, episode: number): Promise<string | null> {
  if (!isTauriAvailable()) return null;
  const key = cacheKey(anilistId, episode);
  if (cache.get(key)) return cache.get(key)!;
  try {
    const { url } = await invoke<CachedThumbnail>("cached_torrent_thumbnail", { cacheKey: key });
    if (url) cache.set(key, url);
    return url;
  } catch (err) {
    console.debug("[cached_torrent_thumbnail] failed", { anilistId, episode, err });
    return null;
  }
}

// Last-resort thumbnail source — see the doc comment on the Rust
// `capture_torrent_thumbnail` command (src-tauri/src/lib.rs) for the full
// pipeline (add torrent, let mpv seek/play headlessly to roughly the
// episode's midpoint, screenshot, remove torrent). Only runs in the real
// Tauri app: torrenting isn't available in the dev-only browser preview.
export async function fetchTorrentThumbnail(
  anilistId: number,
  episode: number,
  title: AnimeTitle,
  durationMinutes: number | null,
): Promise<string | null> {
  if (!isTauriAvailable()) return null;

  const key = cacheKey(anilistId, episode);
  if (cache.has(key)) return cache.get(key)!;
  const existing = inFlight.get(key);
  if (existing) return existing;

  const promise = (async () => {
    try {
      // A frame captured in any earlier session: no nyaa search needed.
      const cached = await invoke<CachedThumbnail>("cached_torrent_thumbnail", { cacheKey: key });
      if (cached.url) {
        console.debug("[capture_torrent_thumbnail] served from disk cache", { anilistId, episode });
        return cached.url;
      }
      if (cached.failedRecently) {
        console.debug("[capture_torrent_thumbnail] skipped, failed recently", { anilistId, episode });
        return null;
      }
      console.debug("[capture_torrent_thumbnail] resolving candidate release", { anilistId, episode });
      const results = await invoke<NyaaResult[]>("search_torrents_for_anime", { title });
      const candidate = pickCandidate(results, episode);
      if (!candidate) {
        console.debug("[capture_torrent_thumbnail] no matching release found", { anilistId, episode });
        return null;
      }
      console.debug("[capture_torrent_thumbnail] invoked", { anilistId, episode, title: candidate.title });
      const url = await invoke<string | null>("capture_torrent_thumbnail", {
        magnet: candidate.magnet,
        cacheKey: key,
        durationMinutes,
      });
      console.info("[capture_torrent_thumbnail] succeeded", { anilistId, episode, found: url !== null });
      return url;
    } catch (err) {
      // Deferred because something is playing: leave it uncached so the
      // next request (after playback) tries again.
      if (String(err).includes("thumbnail-deferred")) {
        console.debug("[capture_torrent_thumbnail] deferred during playback", { anilistId, episode });
        return undefined;
      }
      console.error("[capture_torrent_thumbnail] failed", { anilistId, episode, err });
      return null;
    }
  })()
    .then((result) => {
      if (result === undefined) return null;
      cache.set(key, result);
      return result;
    })
    .finally(() => {
      inFlight.delete(key);
    });

  inFlight.set(key, promise);
  return promise;
}
