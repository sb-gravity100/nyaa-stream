import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import { getProviderCredentials, subscribeProviderCredentials } from "./providerCredentials";

export interface Artwork {
  backdrop: string | null;
  logo: string | null;
  source: string | null;
  attribution: string | null;
}

export interface EpisodeStill {
  url: string;
  source: string;
}

const artworkCache = new Map<number, Promise<Artwork>>();
const stillCache = new Map<string, Promise<EpisodeStill | null>>();

subscribeProviderCredentials(() => {
  console.info("[artwork] provider credentials changed; clearing in-memory results");
  artworkCache.clear();
  stillCache.clear();
});

export function getArtwork(anilistId: number): Promise<Artwork> {
  console.debug("[artwork] get artwork", { anilistId, tauri: isTauriAvailable() });
  const cached = artworkCache.get(anilistId);
  if (cached) return cached;
  const promise = isTauriAvailable()
    ? invoke<Artwork>("get_artwork", { anilistId, credentials: getProviderCredentials() })
        .catch((error) => {
          console.warn("[artwork] artwork lookup failed", { anilistId, error: String(error) });
          return { backdrop: null, logo: null, source: null, attribution: null };
        })
    : Promise.resolve({ backdrop: null, logo: null, source: null, attribution: null });
  artworkCache.set(anilistId, promise);
  return promise;
}

export function getEpisodeStill(anilistId: number, episode: number): Promise<EpisodeStill | null> {
  const key = `${anilistId}:${episode}`;
  console.debug("[artwork] get episode still", { anilistId, episode, tauri: isTauriAvailable() });
  const cached = stillCache.get(key);
  if (cached) return cached;
  const promise = isTauriAvailable()
    ? invoke<EpisodeStill | null>("get_episode_still", { anilistId, episode, credentials: getProviderCredentials() })
        .catch((error) => {
          console.warn("[artwork] episode still lookup failed", { anilistId, episode, error: String(error) });
          return null;
        })
    : Promise.resolve(null);
  stillCache.set(key, promise);
  return promise;
}
