import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import { continueWatching, subscribeProgress } from "./watchProgress";

// Download cache (PLAN.md "Download cache"): torrents of episodes in the
// Continue watching row are kept past the cache cap, so the backend is told
// which episodes those are whenever the row changes.

let lastSent: string | null = null;

function syncKeep(): void {
  const episodes = continueWatching().map((entry) => `${entry.animeId}:${entry.episodeKey}`);
  const key = JSON.stringify(episodes);
  if (key === lastSent) return;
  lastSent = key;
  console.debug("[downloadCache] keep list changed", { count: episodes.length });
  invoke("set_download_cache_keep", { episodes }).catch((err) => {
    lastSent = null;
    console.warn("[downloadCache] set_download_cache_keep failed", { err: String(err) });
  });
}

export function installDownloadCacheSync(): void {
  if (!isTauriAvailable()) return;
  syncKeep();
  subscribeProgress(syncKeep);
  console.debug("[downloadCache] keep sync installed");
}
