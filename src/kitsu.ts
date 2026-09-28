import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import type { KitsuMetadata } from "./types";

const KITSU_BASE_URL = "https://kitsu.io/api/edge";
const EPISODES_PER_PAGE = 20;
const MAX_EPISODE_PAGES = 10;

interface KitsuMappingResource {
  relationships: { item: { data: { id: string } | null } };
}

interface KitsuAnimeResource {
  attributes: { coverImage: { large: string | null } | null };
}

interface KitsuEpisodeResource {
  attributes: {
    number: number | null;
    thumbnail: { original: string | null } | null;
  };
}

async function resolveKitsuId(anilistId: number): Promise<number | null> {
  // include=item: Kitsu omits the relationship's resource-identifier
  // ("data") linkage unless the related resource is explicitly requested -
  // verified live, without this every mapping's relationships.item.data is
  // simply absent, not null, so resolution silently finds nothing.
  const url = `${KITSU_BASE_URL}/mappings?filter[externalSite]=anilist/anime&filter[externalId]=${anilistId}&include=item`;
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`Kitsu mappings request failed: ${resp.status}`);
  const json = await resp.json();
  const data: KitsuMappingResource[] = json.data;
  for (const mapping of data) {
    const id = mapping.relationships.item.data?.id;
    if (id) return parseInt(id, 10);
  }
  return null;
}

async function fetchBackground(kitsuId: number): Promise<string | null> {
  const resp = await fetch(`${KITSU_BASE_URL}/anime/${kitsuId}`);
  if (!resp.ok) throw new Error(`Kitsu anime request failed: ${resp.status}`);
  const json = await resp.json();
  const resource: KitsuAnimeResource = json.data;
  return resource.attributes.coverImage?.large ?? null;
}

async function fetchEpisodeThumbnails(kitsuId: number): Promise<Record<number, string>> {
  const thumbnails: Record<number, string> = {};
  for (let page = 0; page < MAX_EPISODE_PAGES; page++) {
    const offset = page * EPISODES_PER_PAGE;
    const url = `${KITSU_BASE_URL}/anime/${kitsuId}/episodes?page[limit]=${EPISODES_PER_PAGE}&page[offset]=${offset}&sort=number`;
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Kitsu episodes request failed: ${resp.status}`);
    const json = await resp.json();
    const data: KitsuEpisodeResource[] = json.data;
    for (const ep of data) {
      const number = ep.attributes.number;
      const thumb = ep.attributes.thumbnail?.original;
      if (number != null && thumb) thumbnails[number] = thumb;
    }
    if (data.length < EPISODES_PER_PAGE) break;
  }
  return thumbnails;
}

// Dev-only browser-preview mirror of kitsu_client::KitsuClient::get_metadata
// (crates/kitsu-client/src/lib.rs) — only used when window.__TAURI_INTERNALS__
// is absent.
async function fallbackGetKitsuMetadata(anilistId: number): Promise<KitsuMetadata | null> {
  const kitsuId = await resolveKitsuId(anilistId);
  if (kitsuId === null) return null;
  const [background, episodeThumbnails] = await Promise.all([
    fetchBackground(kitsuId),
    fetchEpisodeThumbnails(kitsuId),
  ]);
  return { background, episodeThumbnails };
}

// Module-level cache + in-flight dedupe: both HomePage (per library anime)
// and MediaPage (selected anime) fetch Kitsu metadata for the same AniList
// ids, so this avoids redundant round trips when both are interested in the
// same show.
const cache = new Map<number, KitsuMetadata | null>();
const inFlight = new Map<number, Promise<KitsuMetadata | null>>();

export function getCachedKitsuMetadata(anilistId: number): KitsuMetadata | null | undefined {
  return cache.get(anilistId);
}

// Last known metadata per anime, persisted so the home page can paint card
// art on the very first frame after launch instead of after the backend
// lookup (which waits behind the AniList requests). Refreshed by every
// successful fetch.
const SNAPSHOT_KEY = "nyaa-stream:kitsu-snapshot";

/** Every anime's last known Kitsu metadata - synchronous, for initial state. */
export function kitsuSnapshot(): Record<number, KitsuMetadata | null> {
  try {
    const raw = localStorage.getItem(SNAPSHOT_KEY);
    return raw ? (JSON.parse(raw) as Record<number, KitsuMetadata | null>) : {};
  } catch (err) {
    console.warn("[kitsu] unreadable snapshot", { err: String(err) });
    return {};
  }
}

function saveToSnapshot(anilistId: number, metadata: KitsuMetadata | null) {
  try {
    const snapshot = kitsuSnapshot();
    snapshot[anilistId] = metadata;
    localStorage.setItem(SNAPSHOT_KEY, JSON.stringify(snapshot));
  } catch (err) {
    console.warn("[kitsu] failed to save snapshot", { err: String(err) });
  }
}

export async function fetchKitsuMetadata(anilistId: number): Promise<KitsuMetadata | null> {
  if (cache.has(anilistId)) return cache.get(anilistId)!;
  const existing = inFlight.get(anilistId);
  if (existing) return existing;

  console.debug("[get_kitsu_metadata] invoked", { anilistId });
  const promise = (isTauriAvailable()
    ? invoke<KitsuMetadata | null>("get_kitsu_metadata", { anilistId })
    : fallbackGetKitsuMetadata(anilistId)
  )
    .then((metadata) => {
      console.info("[get_kitsu_metadata] succeeded", { anilistId, found: metadata !== null });
      cache.set(anilistId, metadata);
      saveToSnapshot(anilistId, metadata);
      return metadata;
    })
    .catch((err) => {
      console.error("[get_kitsu_metadata] failed", { anilistId, err });
      cache.set(anilistId, null);
      return null;
    })
    .finally(() => {
      inFlight.delete(anilistId);
    });

  inFlight.set(anilistId, promise);
  return promise;
}
