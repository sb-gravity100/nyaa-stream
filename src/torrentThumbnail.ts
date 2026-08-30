import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import { parseEpisode } from "./episodeParser";
import type { AnimeTitle, NyaaResult } from "./types";

// Module-level cache + in-flight dedupe, same pattern as kitsu.ts. `null`
// means "tried, no thumbnail available" (no matching release found, or the
// capture itself failed) so a card doesn't retry every re-render.
const cache = new Map<string, string | null>();
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
    const label = parseEpisode(r.title);
    return label.kind === "episode" && label.number === episode;
  });
  if (matches.length === 0) return null;
  return matches.reduce((best, r) => (r.seeders > best.seeders ? r : best));
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
    console.debug("[capture_torrent_thumbnail] resolving candidate release", { anilistId, episode });
    try {
      const results = await invoke<NyaaResult[]>("search_torrents_for_anime", { title });
      const candidate = pickCandidate(results, episode);
      if (!candidate) {
        console.debug("[capture_torrent_thumbnail] no matching release found", { anilistId, episode });
        return null;
      }
      console.debug("[capture_torrent_thumbnail] invoked", { anilistId, episode, title: candidate.title });
      const dataUri = await invoke<string | null>("capture_torrent_thumbnail", {
        magnet: candidate.magnet,
        cacheKey: key,
        durationMinutes,
      });
      console.info("[capture_torrent_thumbnail] succeeded", { anilistId, episode, found: dataUri !== null });
      return dataUri;
    } catch (err) {
      console.error("[capture_torrent_thumbnail] failed", { anilistId, episode, err });
      return null;
    }
  })()
    .then((result) => {
      cache.set(key, result);
      return result;
    })
    .finally(() => {
      inFlight.delete(key);
    });

  inFlight.set(key, promise);
  return promise;
}
