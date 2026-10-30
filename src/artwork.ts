import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import { getProviderCredentials, subscribeProviderCredentials } from "./providerCredentials";

export interface Artwork {
  backdrop: string | null;
  fanartBackdrop: string | null;
  logo: string | null;
  source: string | null;
  attribution: string | null;
}

export interface EpisodeStill {
  url: string;
  source: string;
}

const artworkCache = new Map<string, Promise<Artwork>>();
const stillCache = new Map<string, Promise<EpisodeStill | null>>();

subscribeProviderCredentials(() => {
  console.info("[artwork] provider credentials changed; clearing in-memory results");
  artworkCache.clear();
  stillCache.clear();
});

export function getArtwork(anilistId: number, season: string | null, seasonYear: number | null): Promise<Artwork> {
  const startMonth = ({ WINTER: "01", SPRING: "04", SUMMER: "07", FALL: "10" } as Record<string, string>)[season?.toUpperCase() ?? ""];
  const seasonStartDate = startMonth && seasonYear ? `${seasonYear}-${startMonth}-01` : null;
  const cacheKey = `${anilistId}:${seasonStartDate ?? ""}`;
  console.debug("[artwork] get artwork", { anilistId, seasonStartDate, tauri: isTauriAvailable() });
  const cached = artworkCache.get(cacheKey);
  if (cached) return cached;
  const promise = isTauriAvailable()
    ? invoke<Artwork>("get_artwork", { anilistId, seasonStartDate, credentials: getProviderCredentials() })
        .catch((error) => {
          console.warn("[artwork] artwork lookup failed", { anilistId, error: String(error) });
          return { backdrop: null, fanartBackdrop: null, logo: null, source: null, attribution: null };
        })
    : Promise.resolve({ backdrop: null, fanartBackdrop: null, logo: null, source: null, attribution: null });
  artworkCache.set(cacheKey, promise);
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
