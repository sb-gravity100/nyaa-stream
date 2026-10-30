import type { NyaaResult } from "./types";

const STORAGE_KEY = "nyaa-stream:release-results";
const MAX_CACHED_ANIME = 50;

interface ReleaseCacheEntry {
  savedAt: number;
  results: NyaaResult[];
}

interface ReleaseCache {
  version: 1;
  entries: Record<string, ReleaseCacheEntry>;
}

function readCache(): ReleaseCache {
  console.debug("[release-cache] reading persisted results");
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { version: 1, entries: {} };
    const parsed = JSON.parse(raw) as Partial<ReleaseCache>;
    if (parsed.version !== 1 || !parsed.entries || typeof parsed.entries !== "object") return { version: 1, entries: {} };
    return {
      version: 1,
      entries: Object.fromEntries(Object.entries(parsed.entries).filter(([, entry]) => Array.isArray(entry?.results))),
    } as ReleaseCache;
  } catch (err) {
    console.warn("[release-cache] failed to read saved search results", { err: String(err) });
    return { version: 1, entries: {} };
  }
}

export function loadSavedReleaseResults(): Record<number, NyaaResult[]> {
  const cache = readCache();
  const results = Object.fromEntries(Object.entries(cache.entries).filter(([id]) => Number.isSafeInteger(Number(id))).map(([id, entry]) => [Number(id), entry.results]));
  console.info("[release-cache] loaded saved results", { anime: Object.keys(results).length });
  return results;
}

export function saveReleaseResults(animeId: number, results: NyaaResult[]): void {
  console.debug("[release-cache] saving search results", { animeId, count: results.length });
  const cache = readCache();
  cache.entries[String(animeId)] = { savedAt: Date.now(), results };
  const recent = Object.entries(cache.entries)
    .sort(([, left], [, right]) => right.savedAt - left.savedAt)
    .slice(0, MAX_CACHED_ANIME);
  cache.entries = Object.fromEntries(recent);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(cache));
  } catch (err) {
    console.warn("[release-cache] failed to save search results", { animeId, count: results.length, err: String(err) });
  }
}
