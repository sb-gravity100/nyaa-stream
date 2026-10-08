// Dev-only fallback for testing search_anime / search_torrents in a plain
// browser tab (no Tauri IPC bridge available there). Only used when
// window.__TAURI_INTERNALS__ is absent — never runs inside the real app.
// Mirrors crates/anilist-client and crates/nyaa-client server-side logic.

const ANILIST_URL = "https://graphql.anilist.co";
const NYAA_BASE_URL = "https://nyaa.si";

const SEARCH_QUERY = `
query ($search: String, $perPage: Int) {
  Page(page: 1, perPage: $perPage) {
    media(search: $search, type: ANIME, sort: START_DATE_DESC) {
      id
      title { romaji english native }
      description(asHtml: false)
      episodes
      coverImage { large extraLarge }
      bannerImage
      averageScore
      format
      season
      seasonYear
      duration
    }
  }
}
`;

export function isTauriAvailable(): boolean {
  return typeof (window as any).__TAURI_INTERNALS__ !== "undefined";
}

export async function fallbackSearchAnime(query: string): Promise<any[]> {
  const resp = await fetch(ANILIST_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify({ query: SEARCH_QUERY, variables: { search: query, perPage: 20 } }),
  });
  if (!resp.ok) throw new Error(`AniList request failed: ${resp.status}`);
  const json = await resp.json();
  return json.data.Page.media;
}

const MEDIA_BY_ID_QUERY = `
query ($id: Int) {
  Media(id: $id, type: ANIME) {
    id
    title { romaji english native }
    description(asHtml: false)
    episodes
    coverImage { large extraLarge }
    bannerImage
    averageScore
    format
    season
    seasonYear
    duration
  }
}
`;

export async function fallbackGetAnimeDetails(id: number): Promise<any> {
  const resp = await fetch(ANILIST_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify({ query: MEDIA_BY_ID_QUERY, variables: { id } }),
  });
  if (!resp.ok) throw new Error(`AniList request failed: ${resp.status}`);
  const json = await resp.json();
  return json.data.Media;
}

const AIRING_SCHEDULES_QUERY = `
query ($ids: [Int], $from: Int, $to: Int, $perPage: Int) {
  Page(perPage: $perPage) {
    airingSchedules(mediaId_in: $ids, airingAt_greater: $from, airingAt_lesser: $to, sort: TIME_DESC) {
      episode
      airingAt
      media { id title { romaji english native } coverImage { large extraLarge } }
    }
  }
}
`;

export async function fallbackGetLatestEpisodes(mediaIds: number[], from: number, to: number): Promise<any[]> {
  if (mediaIds.length === 0) return [];
  const resp = await fetch(ANILIST_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify({ query: AIRING_SCHEDULES_QUERY, variables: { ids: mediaIds, from, to, perPage: 50 } }),
  });
  if (!resp.ok) throw new Error(`AniList request failed: ${resp.status}`);
  const json = await resp.json();
  return json.data.Page.airingSchedules;
}

// Normalizes "smart" Unicode punctuation that AniList titles use into the
// ASCII form nyaa.si's search tokenizer expects. Mirrors
// crates/nyaa-client's sanitize_query — see that function's doc comment for
// the empirical justification (a curly apostrophe alone drops nyaa.si
// matches from 75 to 1 for "Frieren: Beyond Journey's End").
function sanitizeQuery(query: string): string {
  const normalized = query
    .replace(/[‘’‛＇]/g, "'")
    .replace(/[“”‟＂]/g, '"')
    .replace(/[–—]/g, "-");
  return normalized.split(/\s+/).filter(Boolean).join(" ");
}

export async function fallbackSearchTorrents(query: string): Promise<any[]> {
  const sanitized = sanitizeQuery(query);
  const url = `${NYAA_BASE_URL}/?page=rss&c=1_0&f=0&q=${encodeURIComponent(sanitized)}`;
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`nyaa.si request failed: ${resp.status}`);
  const text = await resp.text();
  const doc = new DOMParser().parseFromString(text, "text/xml");
  const items = Array.from(doc.querySelectorAll("item"));

  const nyaaNs = "https://nyaa.si/xmlns/nyaa";
  const getNyaaField = (item: Element, name: string): string => {
    const el = item.getElementsByTagNameNS(nyaaNs, name)[0];
    return el?.textContent ?? "";
  };

  return items.map((item) => {
    const enclosure = item.querySelector("enclosure");
    return {
      title: item.querySelector("title")?.textContent ?? "",
      magnet: getNyaaField(item, "magnetUrl"),
      torrent_url: enclosure?.getAttribute("url") ?? "",
      view_url: item.querySelector("guid")?.textContent ?? "",
      size: getNyaaField(item, "size"),
      seeders: parseInt(getNyaaField(item, "seeders"), 10) || 0,
      leechers: parseInt(getNyaaField(item, "leechers"), 10) || 0,
      published: item.querySelector("pubDate")?.textContent ?? "",
    };
  });
}

// Searches both the English and romaji titles and merges the results
// (deduplicated by view_url). Mirrors search_torrents_for_anime on the Rust
// side - that used to search English first and only fall back to romaji if
// it returned too few results, but many fansub groups title releases in
// romaji only with no English cross-reference text at all, so a real show's
// English-title search almost never triggered the fallback in practice
// while still missing a large fraction of its actual releases (verified
// live - see search_torrents_for_anime's doc comment for the numbers).
export async function fallbackSearchTorrentsForAnime(title: {
  english: string | null;
  romaji: string | null;
}): Promise<any[]> {
  const candidates = [title.english, title.romaji].filter(
    (t, i, arr): t is string => t !== null && arr.indexOf(t) === i,
  );
  if (candidates.length === 0) return [];

  const seenViewUrls = new Set<string>();
  const merged: any[] = [];
  for (const candidate of candidates) {
    const results = await fallbackSearchTorrents(candidate);
    for (const result of results) {
      if (!seenViewUrls.has(result.view_url)) {
        seenViewUrls.add(result.view_url);
        merged.push(result);
      }
    }
  }
  return merged;
}
