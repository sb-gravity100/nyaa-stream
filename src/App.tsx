import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { getSettings as getSettingsSnapshot } from "./settings";
import {
  episodeLabelText,
  extractSeasonNumber,
  labelOf,
  parseSubmitterFromTitle,
  type EpisodeLabel,
} from "./episodeParser";
import {
  fallbackGetAnimeDetails,
  fallbackGetLatestEpisodes,
  fallbackSearchAnime,
  fallbackSearchTorrentsForAnime,
  isTauriAvailable,
} from "./browserFallback";
import { addToLibrary, getLibrary, isInLibrary, removeFromLibrary } from "./library";
import { loadSavedReleaseResults, saveReleaseResults } from "./releaseCache";
import { addRecentSearch, clearRecentSearches, getRecentSearches, removeRecentSearch } from "./recentSearches";
import { fetchKitsuMetadata, kitsuSnapshot } from "./kitsu";
import { cachedTorrentThumbnail, fetchTorrentThumbnail } from "./torrentThumbnail";
import { episodeNumbersByGroup } from "./releases";
import { AttachCustomSourceDialog } from "./AttachCustomSourceDialog";
import { MediaPage } from "./MediaPage";
import { PlayerView } from "./PlayerView";
import { HomePage } from "./HomePage";
import { SettingsPanel } from "./SettingsPanel";
import { DebugPage, type SourceMappingRecord } from "./DebugPage";
import { BrandMark, SearchIcon, SettingsIcon } from "./icons";
import { Buffering } from "./Buffering";
import { copyToClipboard, openAniListPage, openContextMenu } from "./contextMenuStore";
import { getArtwork, type Artwork } from "./artwork";
import { useProviderCredentials } from "./providerCredentials";
import { goBack, navigate, useRoute, type WatchTarget } from "./router";
import { getUpdateState, startBackgroundUpdateChecks, subscribeUpdateState, type UpdateState } from "./updater";
import {
  displayTitle,
  formatSeason,
  isMovie,
  MOVIE_KEY,
  type AiringEntry,
  type AnimeMedia,
  type CustomSource,
  type KitsuMetadata,
  type NyaaResult,
  type TorrentDetails,
} from "./types";
import "./App.css";

const SEARCH_DEBOUNCE_MS = 350;
const MAX_DROPDOWN_RESULTS = 20;
function isCustomMagnetInput(value: string): boolean {
  const trimmed = value.trim();
  return /^magnet:/i.test(trimmed) || /^[a-f0-9]{40}$/i.test(trimmed);
}
// Results are rendered in chunks as the dropdown is scrolled.
const DROPDOWN_CHUNK = 6;

// "this week and last" — calendar-week-aligned (Monday start), not a
// rolling 14-day window, matching the same calendar-aligned convention as
// the month window this replaced.
function currentAndPreviousWeekWindow(): { from: number; to: number } {
  const now = new Date();
  const daysSinceMonday = (now.getDay() + 6) % 7; // getDay(): 0=Sun..6=Sat -> Mon=0
  const startOfThisWeek = new Date(now.getFullYear(), now.getMonth(), now.getDate() - daysSinceMonday);
  const startOfLastWeek = new Date(
    startOfThisWeek.getFullYear(),
    startOfThisWeek.getMonth(),
    startOfThisWeek.getDate() - 7,
  );
  return { from: Math.floor(startOfLastWeek.getTime() / 1000), to: Math.floor(now.getTime() / 1000) };
}

// Titles that say they're a film even when they also carry a number.
const MOVIE_TITLE = /\b(movie|film|gekijou?ban)\b|劇場版/i;
// Series packs that show up in a movie's nyaa search (same franchise name).
const SERIES_PACK = /\b(batch|complete|season\s*\d|s\d{1,2})\b|\d{1,3}\s*[-~]\s*\d{1,3}/i;
const CUSTOM_EXTRA_FILE = /\b(OVA|OAD|ONA|SP|Specials?|NCOP|NCED|Extras?|PV|Trailer)\d*\b/i;

function isCustomExtraFile(file: CustomSource["files"][number]): boolean {
  return CUSTOM_EXTRA_FILE.test(file.path) || file.label?.kind === "special" || (file.label?.kind === "batch" && file.label.extras === true);
}

/** A movie's page: one "Movie" group of every release that is the film -
 * titles the parser can't number (how movies are named), minus the
 * franchise's TV episodes and season packs that share its name, e.g.
 * Jujutsu Kaisen episodes in a "Jujutsu Kaisen 0" search. */
function movieSources(sources: NyaaResult[]): [string, { label: EpisodeLabel; releases: NyaaResult[] }][] {
  const releases = sources.filter((source) => {
    if (MOVIE_TITLE.test(source.title)) return true;
    const parsed = labelOf(source.label, source.title);
    if (parsed.kind === "episode") return false;
    return !SERIES_PACK.test(source.title);
  });
  console.debug("[media] movie releases", { kept: releases.length, dropped: sources.length - releases.length });
  if (releases.length === 0) return [];
  return [[MOVIE_KEY, { label: { kind: "episode", season: 1, number: 1 }, releases }]];
}

function App() {
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [animeResults, setAnimeResults] = useState<AnimeMedia[]>([]);
  const [directPlay, setDirectPlay] = useState<{ anime: AnimeMedia; source: NyaaResult; episode: number | null; season: number | null } | null>(null);
  const [directMagnetError, setDirectMagnetError] = useState<string | null>(null);
  const [attachmentInput, setAttachmentInput] = useState<string | null>(null);
  const [attachmentSource, setAttachmentSource] = useState<CustomSource | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  // Which page is showing comes from the URL (see router.ts); this is the
  // anime that page is about, once loaded.
  const route = useRoute();
  const [selectedAnime, setSelectedAnime] = useState<AnimeMedia | null>(null);
  // Anime objects the user navigated from (search, library, cards), so
  // opening their route doesn't need an AniList round trip. A reload or a
  // pasted route falls back to get_anime_details.
  const knownAnime = useRef(new Map<number, AnimeMedia>());
  const [routeError, setRouteError] = useState<string | null>(null);
  const [sources, setSources] = useState<NyaaResult[]>([]);
  const [sourcesLoading, setSourcesLoading] = useState(false);
  /** The backend's season per entry (TVDB, else titles, else first season): titles alone call "The Second Plate" season 1. */
  const [entrySeasonByMedia, setEntrySeasonByMedia] = useState<Record<number, number>>({});
  // AniDB -> TVDB episode offset per entry (from the AnimeTosho dataset); stored only, not applied yet.
  const [, setEntryEpisodeOffsetByMedia] = useState<Record<number, number>>({});
  const [details, setDetails] = useState<Record<string, TorrentDetails>>({});
  // Persisted per-anime results let library pages reopen without a full
  // Nyaa search. The anime page's Refresh action replaces this snapshot.
  const [sourcesByMedia, setSourcesByMedia] = useState<Record<number, NyaaResult[]>>(loadSavedReleaseResults);
  const [customSourcesByMedia, setCustomSourcesByMedia] = useState<Record<number, CustomSource[]>>({});
  const [detailsByMedia, setDetailsByMedia] = useState<Record<number, Record<string, TorrentDetails>>>({});
  // Read synchronously: the route effect below looks anime up in it on the
  // very first render (a reload on an anime route).
  const [library, setLibrary] = useState<AnimeMedia[]>(getLibrary);
  const [artworkByMedia, setArtworkByMedia] = useState<Record<number, Artwork>>({});
  const providerCredentials = useProviderCredentials();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsShowUpdates, setSettingsShowUpdates] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState>(getUpdateState);
  const [latestEpisodes, setLatestEpisodes] = useState<AiringEntry[]>([]);
  const [latestEpisodesLoading, setLatestEpisodesLoading] = useState(false);
  // Shared by MediaPage (selected anime's backdrop + video thumbnails) and
  // HomePage (per-library-anime latest-episode thumbnails) - keyed by
  // AniList id, `null` meaning "fetched, Kitsu has no mapping" so callers
  // don't re-fetch a known miss.
  // Seeded from the last session's snapshot so card art shows on the first
  // paint; loadKitsuMetadata refreshes each entry once per session.
  const [kitsuByMedia, setKitsuByMedia] = useState<Record<number, KitsuMetadata | null>>(kitsuSnapshot);
  // Last-resort thumbnails pulled from the torrent itself, keyed by
  // "{mediaId}-{episode}" - only populated for latest-episode cards where
  // Kitsu and AniList both came up completely empty (see loadTorrentThumbnail
  // below); most cards never touch this at all.
  const [torrentThumbnails, setTorrentThumbnails] = useState<Record<string, string | null>>({});
  // Cumulative prior-season episode count for the anime currently being
  // browsed (see get_absolute_episode_offset/groupedSources) - keyed by
  // AniList id, 0 meaning either "no prior seasons" or "not loaded yet",
  // which are both safe defaults for the correction to be a no-op.
  const [episodeOffsetByMedia, setEpisodeOffsetByMedia] = useState<Record<number, number>>({});

  const [recentSearches, setRecentSearches] = useState(getRecentSearches);
  const [visibleResults, setVisibleResults] = useState(DROPDOWN_CHUNK);
  const resultsWrapRef = useRef<HTMLDivElement>(null);
  const moreSentinelRef = useRef<HTMLLIElement>(null);
  const skipNextSearch = useRef(false);
  // Bumped per openAnime call: slower phases of an earlier open must not overwrite a newer one.
  const openSeq = useRef(0);
  const blurTimeout = useRef<number | undefined>(undefined);

  useEffect(() => {
    const unsubscribe = subscribeUpdateState(setUpdateState);
    const stopBackgroundChecks = startBackgroundUpdateChecks();
    return () => {
      unsubscribe();
      stopBackgroundChecks();
    };
  }, []);

  useEffect(() => {
    loadLatestEpisodes();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [library]);

  useEffect(() => {
    let cancelled = false;
    const ids = [...new Set([...library.map((anime) => anime.id), ...(selectedAnime ? [selectedAnime.id] : [])])];
    console.info("[artwork] refreshing app artwork", { shows: ids.length, hasTmdb: !!providerCredentials.tmdbReadAccessToken, hasFanart: !!providerCredentials.fanartApiKey });
    setArtworkByMedia({});
    const load = async () => {
      for (const id of ids) {
        if (cancelled) return;
        const artwork = await getArtwork(id);
        if (!cancelled) setArtworkByMedia((current) => ({ ...current, [id]: artwork }));
      }
    };
    void load();
    return () => { cancelled = true; };
  }, [library, selectedAnime?.id, providerCredentials.tmdbReadAccessToken, providerCredentials.fanartApiKey]);

  useEffect(() => {
    if (import.meta.env.DEV && location.hash === "#/debug") {
      skipNextSearch.current = false;
      return;
    }
    if (skipNextSearch.current) {
      skipNextSearch.current = false;
      return;
    }
    if (!query.trim()) {
      setAnimeResults([]);
      setLoading(false);
      setDropdownOpen(false);
      return;
    }
    setDropdownOpen(true);
    if (isCustomMagnetInput(query)) { setAnimeResults([]); setLoading(false); return; }
    const timer = window.setTimeout(() => runSearch(query), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  // Reveals the next chunk when the sentinel under the list scrolls into view.
  useEffect(() => {
    const sentinel = moreSentinelRef.current;
    if (!sentinel || !("IntersectionObserver" in window)) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          console.debug("[search] revealing more results");
          setVisibleResults((n) => n + DROPDOWN_CHUNK);
        }
      },
      { root: resultsWrapRef.current, rootMargin: "80px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [animeResults, visibleResults, dropdownOpen]);

  async function runSearch(q: string) {
    if (isCustomMagnetInput(q)) return;
    console.debug("[search_anime] invoked", { query: q });
    setLoading(true);
    setError(null);
    try {
      const results = isTauriAvailable()
        ? await invoke<AnimeMedia[]>("search_anime", { query: q })
        : await fallbackSearchAnime(q);
      console.info("[search_anime] succeeded", { query: q, count: results.length });
      setAnimeResults(results.slice(0, MAX_DROPDOWN_RESULTS));
      setVisibleResults(DROPDOWN_CHUNK);
    } catch (err) {
      console.error("[search_anime] failed", { query: q, err });
      setError(`Search failed: couldn't reach AniList (${err instanceof Error ? err.message : String(err)}).`);
      setAnimeResults([]);
    } finally {
      setLoading(false);
    }
  }

  async function playCustomMagnet(input: string) {
    setDirectMagnetError(null);
    try {
      const parsed = await invoke<{ infoHash: string; name: string | null; magnet: string; label: EpisodeLabel }>("parse_custom_magnet", { input });
      const title = parsed.name?.trim() || `Torrent ${parsed.infoHash.slice(0, 8)}`;
      const anime: AnimeMedia = { id: -1, title: { romaji: title, english: null, native: null }, description: null, episodes: null, coverImage: { large: null, extraLarge: null }, averageScore: null, format: null, season: null, seasonYear: null, duration: null };
      const source: NyaaResult = { title, magnet: parsed.magnet, torrent_url: "", view_url: `magnet://${parsed.infoHash}`, size: "", seeders: 0, leechers: 0, published: "", label: parsed.label ?? { kind: "unknown" } };
      setDirectPlay({
        anime,
        source,
        episode: parsed.label.kind === "episode" ? parsed.label.number : null,
        season: "season" in parsed.label ? parsed.label.season : null,
      });
      setDropdownOpen(false);
    } catch (err) {
      setDirectMagnetError(err instanceof Error ? err.message : String(err));
    }
  }

  function showCustomMagnetContextMenu(event: MouseEvent, input: string) {
    console.debug("[search] open magnet context menu", { inputLength: input.length });
    openContextMenu(event, [
      { label: "Play magnet", action: () => void playCustomMagnet(input) },
      { label: "Attach to episode…", action: () => { setAttachmentSource(null); setAttachmentInput(input); setDropdownOpen(false); } },
      { label: "Copy magnet", action: () => copyToClipboard(input, "magnet") },
    ]);
  }

  function pickAnime(anime: AnimeMedia) {
    console.info("[search] selected anime", { animeId: anime.id });
    knownAnime.current.set(anime.id, anime);
    setRecentSearches(addRecentSearch(query));
    setDropdownOpen(false);
    navigate({ name: "anime", id: anime.id, watch: null });
  }

  // Latest Episodes / Continue watching: straight to that episode's player.
  function selectEpisode(anime: AnimeMedia, episode: number) {
    knownAnime.current.set(anime.id, anime);
    navigate({ name: "anime", id: anime.id, watch: { episode } });
  }

  // Loads whichever anime the route names - from what the user clicked
  // when possible, else from AniList (reload, back/forward into an anime
  // this session hasn't seen yet).
  const routeAnimeId = route.name === "anime" ? route.id : null;
  useEffect(() => {
    setRouteError(null);
    if (import.meta.env.DEV && location.hash === "#/debug") return;
    if (routeAnimeId == null) {
      if (selectedAnime) backToSearch();
      return;
    }
    if (selectedAnime?.id === routeAnimeId) return;
    const known = knownAnime.current.get(routeAnimeId) ?? library.find((a) => a.id === routeAnimeId);
    if (known) {
      void openAnime(known);
      return;
    }
    let cancelled = false;
    console.debug("[router] loading anime for route", { id: routeAnimeId });
    (isTauriAvailable() ? invoke<AnimeMedia>("get_anime_details", { id: routeAnimeId }) : fallbackGetAnimeDetails(routeAnimeId))
      .then((anime) => {
        if (cancelled) return;
        knownAnime.current.set(anime.id, anime);
        void openAnime(anime);
      })
      .catch((err) => {
        console.error("[router] couldn't load anime for route", { id: routeAnimeId, err });
        if (!cancelled) setRouteError(`Couldn't load this anime from AniList (${err instanceof Error ? err.message : String(err)}).`);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routeAnimeId]);

  async function openAnime(anime: AnimeMedia, forceDebugSearch = false, refreshResults = false) {
    const seq = ++openSeq.current;
    const releaseQuery = displayTitle(anime.title);
    skipNextSearch.current = true;
    setQuery(releaseQuery);
    setDropdownOpen(false);
    setSelectedAnime(anime);
    setError(null);
    if (isTauriAvailable()) {
      void invoke<CustomSource[]>("list_custom_sources", { animeId: anime.id })
        .then((items) => setCustomSourcesByMedia((current) => ({ ...current, [anime.id]: items })))
        .catch((err) => console.warn("[custom sources] failed to load", { animeId: anime.id, err: String(err) }));
    }

    const cachedSources = sourcesByMedia[anime.id];
    if (cachedSources && !forceDebugSearch && !refreshResults) {
      console.debug("[search_torrents_for_anime] served from cache", { anime: releaseQuery, count: cachedSources.length });
      setSources(cachedSources);
      setDetails(detailsByMedia[anime.id] ?? {});
      setSourcesLoading(false);
      if (!detailsByMedia[anime.id] && !isMovie(anime)) void loadDetails(anime.id, cachedSources);
    } else {
      console.debug("[search_torrents_for_anime] invoked", { anime: releaseQuery, title: anime.title });
      if (!refreshResults) {
        setSources([]);
        setDetails({});
      }
      setSourcesLoading(true);
      let shown = false;
      let fullLoaded = false;
      // Earlier phases only ever paint sooner than the full search; they never replace its result.
      const showEarly = (phase: string, early: NyaaResult[]) => {
        if (refreshResults || fullLoaded || openSeq.current !== seq || early.length === 0) return;
        console.info(`[${phase}] shown before the full search`, { anime: releaseQuery, count: early.length });
        shown = true;
        setSources(early);
        setSourcesLoading(false);
      };
      const preferredFansubber = getSettingsSnapshot().preferredFansubber.trim();
      const args = { title: anime.title, synonyms: anime.synonyms ?? [], anilistId: anime.id };
      if (isTauriAvailable()) {
        void invoke<number | null>("get_entry_season", args)
          .then((season) => {
            if (season != null) setEntrySeasonByMedia((current) => ({ ...current, [anime.id]: season }));
          })
          .catch((err) => console.warn("[get_entry_season] failed", { err }));
        void invoke<number | null>("get_entry_episode_offset", { anilistId: anime.id })
          .then((offset) => {
            if (offset != null) setEntryEpisodeOffsetByMedia((current) => ({ ...current, [anime.id]: offset }));
          })
          .catch((err) => console.warn("[get_entry_episode_offset] failed", { err }));
        // 1) what the local database already holds (instant), 2) only the preferred fansubber's uploads (a few requests).
        void invoke<NyaaResult[]>("search_local_releases", args)
          .then((local) => showEarly("search_local_releases", local))
          .catch((err) => console.warn("[search_local_releases] failed", { err }));
        if (preferredFansubber) {
          void invoke<NyaaResult[]>("search_fansubber_releases", { ...args, fansubber: preferredFansubber })
            .then((mine) => showEarly("search_fansubber_releases", mine))
            .catch((err) => console.warn("[search_fansubber_releases] failed", { err }));
        }
      }
      try {
        const results = isTauriAvailable()
          ? await invoke<NyaaResult[]>("search_torrents_for_anime", { ...args, fansubber: preferredFansubber || null, diagnostics: forceDebugSearch })
          : await fallbackSearchTorrentsForAnime(anime.title);
        fullLoaded = true;
        if (openSeq.current !== seq) return;
        console.info("[search_torrents_for_anime] succeeded", { anime: releaseQuery, count: results.length });
        setSources(results);
        setSourcesByMedia((current) => ({ ...current, [anime.id]: results }));
        if (!forceDebugSearch) saveReleaseResults(anime.id, results);
        setSourcesLoading(false);
        // A movie's releases all go in one group, so there's no batch
        // ambiguity worth a view-page scrape per title (which also ran
        // into nyaa.si's rate limit - every movie title parses "unknown").
        if (!isMovie(anime)) await loadDetails(anime.id, results);
      } catch (err) {
        fullLoaded = true;
        console.error("[search_torrents_for_anime] failed", { anime: releaseQuery, err });
        // Releases from the local database/fansubber search are still usable.
        if (!shown && openSeq.current === seq) {
          setError(`${refreshResults ? "Refresh failed; saved releases are still shown" : "Couldn't load releases from nyaa.si"} (${err instanceof Error ? err.message : String(err)}).`);
        }
        setSourcesLoading(false);
      }
    }
    // Fire-and-forget: enriches the already-shown page with per-episode
    // thumbnails once they arrive, doesn't block anything above.
    loadAnimeDetails(anime.id);
    loadKitsuMetadata(anime.id);
    loadEpisodeOffset(anime.id);
  }

  // Some fansub groups number episodes absolutely across a whole franchise
  // instead of restarting from 1 each season (e.g. "Season 4 Episode 92"
  // meaning the franchise's 92nd episode overall, not the 92nd of season
  // 4) - episodeParser.ts can't detect or correct this from title text
  // alone, since it has no notion of a franchise's other seasons. This
  // fetches how many episodes aired before the currently-browsed season,
  // from AniList's relations graph (see get_absolute_episode_offset's doc
  // comment), so groupedSources can recognize and correct it.
  async function loadEpisodeOffset(id: number) {
    if (id in episodeOffsetByMedia) return;
    const offset = isTauriAvailable() ? await invoke<number>("get_absolute_episode_offset", { id }).catch(() => 0) : 0;
    setEpisodeOffsetByMedia((current) => (id in current ? current : { ...current, [id]: offset }));
  }

  async function loadKitsuMetadata(id: number) {
    // Not skipped when the snapshot already has this id: fetchKitsuMetadata
    // dedupes per session, and the refresh picks up new episode art.
    const metadata = await fetchKitsuMetadata(id);
    setKitsuByMedia((current) => (JSON.stringify(current[id]) === JSON.stringify(metadata) ? current : { ...current, [id]: metadata }));
  }

  async function loadTorrentThumbnail(entry: AiringEntry) {
    const key = `${entry.media.id}-${entry.episode}`;
    if (key in torrentThumbnails) return;
    const url = await fetchTorrentThumbnail(entry.media.id, entry.episode, entry.media.title, entry.media.duration);
    setTorrentThumbnails((current) => (key in current ? current : { ...current, [key]: url }));
  }

  // Frames captured in earlier sessions show up immediately: the disk cache
  // is checked for every card as soon as it's listed, not after the Kitsu
  // lookup the capture fallback below waits on.
  useEffect(() => {
    for (const entry of latestEpisodes) {
      const key = `${entry.media.id}-${entry.episode}`;
      if (torrentThumbnails[key]) continue;
      cachedTorrentThumbnail(entry.media.id, entry.episode).then((url) => {
        if (url) setTorrentThumbnails((current) => (current[key] ? current : { ...current, [key]: url }));
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [latestEpisodes]);

  // Once Kitsu metadata is known for a show, check whether any of its
  // currently-shown latest-episode cards have no thumbnail from either
  // Kitsu or AniList at all - those are the only ones worth the expensive
  // torrent-capture fallback (see the "last-resort only" scoping decision).
  useEffect(() => {
    for (const entry of latestEpisodes) {
      const kitsu = kitsuByMedia[entry.media.id];
      if (kitsu === undefined) continue; // still loading, wait for it
      const hasKitsuThumbnail = kitsu !== null && kitsu.episodeThumbnails[entry.episode] != null;
      const hasKitsuBackground = kitsu !== null && kitsu.background != null;
      if (!hasKitsuThumbnail && !hasKitsuBackground) {
        loadTorrentThumbnail(entry);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [latestEpisodes, kitsuByMedia]);

  async function loadAnimeDetails(id: number) {
    console.debug("[get_anime_details] invoked", { id });
    try {
      const details = isTauriAvailable()
        ? await invoke<AnimeMedia>("get_anime_details", { id })
        : await fallbackGetAnimeDetails(id);
      console.info("[get_anime_details] succeeded", { id });
      setSelectedAnime((current) => (current?.id === id ? { ...current, ...details } : current));
    } catch (err) {
      console.error("[get_anime_details] failed", { id, err });
    }
  }

  function backToSearch() {
    setSelectedAnime(null);
    setSources([]);
    setDetails({});
    setError(null);
  }

  async function loadLatestEpisodes() {
    if (library.length === 0) {
      setLatestEpisodes([]);
      return;
    }
    const { from, to } = currentAndPreviousWeekWindow();
    const mediaIds = library.map((a) => a.id);
    console.debug("[get_latest_episodes] invoked", { count: mediaIds.length, from, to });
    setLatestEpisodesLoading(true);
    try {
      const entries = isTauriAvailable()
        ? await invoke<AiringEntry[]>("get_latest_episodes", { mediaIds, from, to })
        : await fallbackGetLatestEpisodes(mediaIds, from, to);
      console.info("[get_latest_episodes] succeeded", { count: entries.length });
      setLatestEpisodes(entries);
      // Fire-and-forget, one per distinct show in the row (not per episode).
      const uniqueMediaIds = new Set(entries.map((e) => e.media.id));
      uniqueMediaIds.forEach((id) => loadKitsuMetadata(id));
    } catch (err) {
      console.error("[get_latest_episodes] failed", { err });
      setLatestEpisodes([]);
    } finally {
      setLatestEpisodesLoading(false);
    }
  }

  function handleAddToLibrary(anime: AnimeMedia) {
    console.info("[library] add anime requested", { animeId: anime.id });
    setLibrary((current) => addToLibrary(anime, current));
  }

  function handleRemoveFromLibrary(id: number) {
    console.info("[library] remove anime requested", { animeId: id });
    setLibrary((current) => removeFromLibrary(id, current));
  }

  function showSearchAnimeContextMenu(event: MouseEvent, anime: AnimeMedia) {
    const saved = isInLibrary(anime.id, library);
    console.debug("[search] open anime context menu", { animeId: anime.id, saved });
    openContextMenu(event, [
      { label: "Open", action: () => pickAnime(anime) },
      { label: saved ? "Remove from library" : "Add to library", action: () => saved ? handleRemoveFromLibrary(anime.id) : handleAddToLibrary(anime) },
      { label: "Copy title", action: () => copyToClipboard(displayTitle(anime.title), "anime-title") },
      { label: "Open on AniList", action: () => openAniListPage(anime.id) },
    ]);
  }

  async function loadDetails(mediaId: number, results: NyaaResult[]) {
    if (!isTauriAvailable()) return;
    // A view-page scrape is only worth its cost when the title itself
    // doesn't already tell us what we need: the group tag covers
    // "submitter" for display (see parseSubmitterFromTitle's doc comment
    // for accuracy caveats), and the title-regex parser already resolves
    // episode/batch for ~99% of real releases (verified against a 75-item
    // nyaa.si scrape). Scraping only the leftover ambiguous titles cuts
    // requests to nyaa.si from one-per-result down to a handful.
    const needsScrape = results.filter(
      (r) => parseSubmitterFromTitle(r.title) === null || labelOf(r.label, r.title).kind === "unknown",
    );
    if (needsScrape.length === 0) {
      console.debug("[get_torrent_details_batch] skipped, nothing ambiguous", { total: results.length });
      return;
    }
    const viewUrls = needsScrape.map((r) => r.view_url);
    console.debug("[get_torrent_details_batch] invoked", { count: viewUrls.length, total: results.length });
    try {
      const batch = await invoke<(TorrentDetails | null)[]>("get_torrent_details_batch", { viewUrls });
      const merged: Record<string, TorrentDetails> = {};
      batch.forEach((d, i) => {
        if (d) merged[viewUrls[i]] = d;
      });
      console.info("[get_torrent_details_batch] succeeded", {
        enriched: Object.keys(merged).length,
        scraped: viewUrls.length,
        skipped: results.length - viewUrls.length,
      });
      setDetails(merged);
      setDetailsByMedia((current) => ({ ...current, [mediaId]: merged }));
    } catch (err) {
      console.error("[get_torrent_details_batch] failed", { err });
    }
  }

  function handleInputFocus() {
    if (blurTimeout.current) window.clearTimeout(blurTimeout.current);
    if (query.trim() ? animeResults.length > 0 || loading : recentSearches.length > 0) setDropdownOpen(true);
  }

  function handleInputBlur() {
    // Delay so a dropdown item's onMouseDown fires before the dropdown closes.
    blurTimeout.current = window.setTimeout(() => setDropdownOpen(false), 150);
  }

  // nyaa.si's search isn't a strict phrase match - verified live that
  // browsing e.g. "That Time I Got Reincarnated as a Slime Season 4"
  // still returns plenty of season 1/2/3 releases too (its own titles
  // parse to their own correct season via the same logic below, but they
  // don't belong on THIS anime's page at all). Only treated as a real
  // filter when the currently-browsed anime's own title text actually
  // names a season >1 - a franchise's unnumbered "season 1" entry is
  // exactly the ambiguous default extractSeasonNumber falls back to for
  // genuinely unparseable titles too, so filtering there would risk
  // hiding real matches instead of removing wrong-season noise.
  const currentAnimeSeason = useMemo(() => {
    if (!selectedAnime) return 1;
    const known = entrySeasonByMedia[selectedAnime.id];
    if (known != null) return known;
    const fromEnglish = selectedAnime.title.english ? extractSeasonNumber(selectedAnime.title.english) : 1;
    if (fromEnglish !== 1) return fromEnglish;
    return selectedAnime.title.romaji ? extractSeasonNumber(selectedAnime.title.romaji) : 1;
  }, [selectedAnime, entrySeasonByMedia]);

  const currentEpisodeOffset = selectedAnime ? (episodeOffsetByMedia[selectedAnime.id] ?? 0) : 0;

  const debugActive = import.meta.env.DEV && location.hash === "#/debug";
  const sourceAnalysis = useMemo(() => {
    const mapping = new Map<string, SourceMappingRecord>();
    if (debugActive) for (const source of sources) {
      mapping.set(source.view_url, { viewUrl: source.view_url, title: source.title, status: "unresolved", parsedLabel: episodeLabelText(labelOf(source.label, source.title)), mappedAs: [], reason: "Awaiting source grouping", checks: [] });
    }
    const check = (source: NyaaResult, name: string, outcome: SourceMappingRecord["checks"][number]["outcome"], evidence: string) => {
      if (!debugActive) return;
      mapping.get(source.view_url)?.checks.push({ name, outcome, evidence });
    };
    const reject = (source: NyaaResult, reason: string, checkName: string, evidence: string) => {
      const row = mapping.get(source.view_url);
      if (row) { row.status = "filtered"; row.reason = reason; }
      check(source, checkName, "failed", evidence);
    };
    if (selectedAnime && isMovie(selectedAnime)) {
      const movieGroups = movieSources(sources);
      if (!debugActive) return { groups: movieGroups, mapping: [] };
      const movieReleases = new Set(movieGroups.flatMap(([, group]) => group.releases.map((source) => source.view_url)));
      for (const source of sources) {
        check(source, "Anime show match", "passed", "Returned by the backend matcher for the selected AniList entry");
        const parsed = labelOf(source.label, source.title);
        const isMovieSource = MOVIE_TITLE.test(source.title) || (parsed.kind !== "episode" && !SERIES_PACK.test(source.title));
        check(source, "Movie-entry release", isMovieSource ? "passed" : "failed", isMovieSource ? "Movie title or non-episode, non-series-pack release" : "Episode or series pack excluded from this movie entry");
        const row = mapping.get(source.view_url)!;
        if (movieReleases.has(source.view_url)) { row.status = "mapped"; row.mappedAs = ["Movie"]; row.reason = "Included in the movie source group"; }
        else { row.status = "filtered"; row.reason = "Excluded by the movie-entry release rule"; }
      }
      return { groups: movieGroups, mapping: Array.from(mapping.values()) };
    }
    const groups = new Map<string, { label: EpisodeLabel; releases: NyaaResult[] }>();
    const packs: { source: NyaaResult; label: Extract<EpisodeLabel, { kind: "batch" }>; explicit: boolean }[] = [];
    const addToGroup = (label: EpisodeLabel, source: NyaaResult) => {
      const key = episodeLabelText(label);
      const bucket = groups.get(key) ?? { label, releases: [] };
      bucket.releases.push(source);
      groups.set(key, bucket);
    };
    // Episode numbers that season-1 (or unmarked) releases cover: a release
    // explicitly marked season 2+ for one of those same numbers is a sequel's
    // episode (Frieren's "S02E05"), not this entry's.
    const seasonOneNumbers = new Set<number>();
    for (const source of sources) {
      const label = labelOf(source.label, source.title);
      if (label.kind === "episode" && label.season === 1) seasonOneNumbers.add(label.number);
    }
    const groupNumbers = episodeNumbersByGroup(sources);
    for (const source of sources) {
      check(source, "Anime show match", "passed", "Returned by the backend matcher for the selected AniList entry");
      const parsed = labelOf(source.label, source.title);
      check(source, "Title episode label", parsed.kind === "unknown" ? "unresolved" : "passed", parsed.kind === "unknown" ? "No episode or batch could be read from the release title" : episodeLabelText(parsed));
      // The title-regex parser can't tell a batch apart from a genuinely
      // unparseable title. When we have ground truth from the torrent's
      // view page (file_count > 1), trust that over the "Unknown" guess.
      // A title reaching "unknown" already means no season marker was
      // found in the text either, so season 1 here matches what
      // extractSeasonNumber's default would have produced anyway.
      let label: EpisodeLabel = parsed;
      const scraped = details[source.view_url];
      if (scraped && (parsed.kind === "batch" || parsed.kind === "unknown")) {
        check(source, "Torrent file list", "passed", `${scraped.file_count} files; detected seasons ${(scraped.seasons ?? []).join(", ") || "unmarked"}`);
      }
      // AnimeTosho's AniDB episode id: a release that parsed no episode takes
      // the number its sibling releases of the same episode agree on.
      const grouped = parsed.kind === "unknown" && source.episode_group ? groupNumbers.get(source.episode_group) : undefined;
      if (grouped !== undefined) {
        label = { kind: "episode", season: currentAnimeSeason, number: grouped };
        check(source, "AniDB episode group", "passed", `Episode id ${source.episode_group} maps this title to episode ${grouped}`);
      } else if (parsed.kind === "unknown" && scraped?.is_batch !== false) {
        // A matched release whose title names no episode is a season batch of
        // this entry (the matcher already tied it to the show). Once its view
        // page's file list is known, the seasons its files carry decide: a
        // release with none of this entry's season is another season's pack.
        // (A single-file release stays unknown.)
        const fileSeasons = scraped?.seasons ?? [];
        if (fileSeasons.length > 0 && !fileSeasons.includes(currentAnimeSeason)) {
          reject(source, "Torrent files belong to another season", "Scraped file seasons", `Files report seasons ${fileSeasons.join(", ")}; selected season is ${currentAnimeSeason}`);
          continue;
        }
        label = { kind: "batch", season: currentAnimeSeason, episodeRange: null };
        check(source, "Batch fallback", "passed", scraped ? `${scraped.file_count} files identify this as a batch` : "Unknown title is provisionally grouped as a batch until file details say otherwise");
      } else if (parsed.kind === "unknown") {
        check(source, "Torrent file count", "passed", "Scraped details identify a single-file release; it remains Unknown");
      }

      // See loadEpisodeOffset's doc comment - a release numbered
      // absolutely across the whole franchise (its episode number is too
      // high to be season-relative and matches "offset + a real in-season
      // number" instead) gets corrected to that in-season number so it
      // merges into the same bucket as every other group's
      // correctly-numbered release for the same actual episode, rather
      // than sitting alone in a bogus "Episode 92"-style bucket. Checked
      // *before* the season filter below: a release like ToonsHub's, which
      // carries no season marker in the title at all (so extractSeasonNumber
      // defaulted label.season to 1), would otherwise get dropped as
      // "wrong season" noise before ever reaching this correction -
      // verified live against real Tensura Season 4 data (absolute episode
      // 93 = Season 4's episode 21, offset 72). Once the math confirms it
      // belongs to the currently-browsed season, snap label.season to match
      // so the filter below doesn't then drop it anyway.
      let absoluteNumber = false;
      if (
        label.kind === "episode" &&
        currentEpisodeOffset > 0 &&
        selectedAnime?.episodes != null &&
        label.number > selectedAnime.episodes
      ) {
        const relative = label.number - currentEpisodeOffset;
        if (relative >= 1 && relative <= selectedAnime.episodes) {
          const originalNumber = label.number;
          label.number = relative;
          label.season = currentAnimeSeason;
          absoluteNumber = true;
          check(source, "Franchise episode offset", "passed", `Episode ${originalNumber} − offset ${currentEpisodeOffset} = episode ${relative}`);
        }
      }

      // Keep the legacy fallback for results restored from older cached paths;
      // the backend now rejects unmarked names shared with franchise siblings.
      if (
        source.season_ambiguous &&
        !source.aid_trusted &&
        !absoluteNumber &&
        currentAnimeSeason !== 1 &&
        (parsed.kind === "episode" || parsed.kind === "batch") &&
        !(parsed.seasons ? parsed.seasons.length > 0 : parsed.season !== 1) &&
        !(scraped?.seasons ?? []).includes(currentAnimeSeason)
      ) {
        reject(source, "Unmarked title is ambiguous for this season", "Season ambiguity", "No trusted AniDB mapping, absolute-number correction, or matching scraped season resolved it");
        continue;
      }

      // The dataset's trusted AniDB id already put this release on this entry,
      // whatever season number its title carries (Erai-raws files a split
      // cour as "2nd Season"; a sequel numbered "S02E13"): file it under the
      // entry's season instead of letting the filters below drop it. Season 0
      // (specials) stays out.
      if (
        source.aid_trusted &&
        (label.kind === "episode" || label.kind === "batch") &&
        label.season !== 0 &&
        label.season !== currentAnimeSeason &&
        !(label.seasons ?? []).includes(currentAnimeSeason)
      ) {
        label = { ...label, season: currentAnimeSeason, seasons: [currentAnimeSeason] };
        check(source, "Trusted AniDB season", "passed", `Trusted dataset match assigns this release to season ${currentAnimeSeason}`);
      }

      // Season 0 is a franchise's specials/movies, which are their own
      // AniList entries with their own pages - not episodes of a TV series
      // (e.g. Fate/strange Fake's "S00E01 Whispers of Dawn").
      if (label.kind === "episode" && label.season === 0 && (selectedAnime?.format === "TV" || selectedAnime?.format === "TV_SHORT")) {
        reject(source, "Season 0 special excluded from TV episodes", "TV season rule", "Season 0 is routed to its own franchise entry");
        continue;
      }

      // The first season's own page: releases explicitly marked season 2+ are
      // later seasons of the franchise (currentAnimeSeason's filter below only
      // runs for season 2+ entries, since an unmarked title defaults to 1). A
      // sequel is only dropped when this entry's episode of the same number
      // exists, so a part 2 that numbers on as "S02E13" still shows.
      if (
        currentAnimeSeason === 1 &&
        (label.kind === "episode" || label.kind === "batch") &&
        label.season > 1 &&
        (label.seasons ?? []).length < 2 &&
        (label.kind === "episode" ? seasonOneNumbers.has(label.number) : seasonOneNumbers.size > 0)
      ) {
        reject(source, "Same-number release belongs to a later season", "First-season collision check", `Season ${label.season} episode number already exists on season 1`);
        continue;
      }

      // See currentAnimeSeason's doc comment - drop releases we're
      // confident belong to a different season of the same franchise
      // rather than this anime's own episodes.
      // (A multi-season pack - "S1-S5" - belongs to every season it names.)
      // Only an explicit season marker counts: a release with none ("Symphogear
      // XV - 06") parses as season 1 by default and belongs to the entry (the
      // matcher already tied it to the show).
      const marked = label.kind === "episode" || label.kind === "batch" ? (label.seasons ? label.seasons.length > 0 : label.season !== 1) : false;
      if (
        currentAnimeSeason !== 1 &&
        marked &&
        (label.kind === "episode" || label.kind === "batch") &&
        label.season !== currentAnimeSeason &&
        !(label.seasons ?? []).includes(currentAnimeSeason)
      ) {
        reject(source, "Release season does not match this page", "Selected-season check", `Release season ${label.season}; selected season ${currentAnimeSeason}`);
        continue;
      }
      const mappedSeasons = label.kind === "episode" || label.kind === "batch"
        ? (label.seasons?.length ? label.seasons : [label.season])
        : [];
      const seasonEvidence = label.kind !== "episode" && label.kind !== "batch"
        ? "No episode-season rule applies to this release label"
        : currentAnimeSeason === 1 && marked && label.season > 1
          ? `Later-season release retained because episode ${label.kind === "episode" ? label.number : "pack"} does not collide with a season 1 episode`
          : marked
            ? `Release season ${mappedSeasons.join(", ")} includes selected season ${currentAnimeSeason}`
            : `No explicit season marker; title match allows it on season ${currentAnimeSeason}`;
      check(source, "Selected season", label.kind === "episode" || label.kind === "batch" ? "passed" : "not needed", seasonEvidence);

      // A batch with an explicit episode range (e.g. "E15-E28") actually
      // covers those specific episodes, so spread it into each of those
      // episode rows' source counts instead of one undifferentiated
      // "Batch" bucket — a batch without a stated range (most of them)
      // still can't be attributed to specific episodes, so it stays as-is.
      if (label.kind === "batch" && label.episodeRange !== null) {
        const [min, max] = label.episodeRange;
        check(source, "Episode range", "passed", `Batch covers episodes ${min}–${max}`);
        for (let n = min; n <= max; n++) {
          addToGroup({ kind: "episode", season: label.season, number: n }, source);
        }
        continue;
      }

      // `explicit`: the title itself says it is a pack (not a guess for an unmarked release).
      if (label.kind === "batch") packs.push({ source, label, explicit: parsed.kind === "batch" });
      addToGroup(label, source);
    }
    // A pack with no episode range (a season pack, "S1-S5", "Complete
    // Series") also serves each episode of the seasons it holds - the player
    // picks the episode's file inside it. With its file list scraped, a file
    // must exist for that season and episode; without one, the title must
    // name the season itself.
    for (const group of groups.values()) {
      if (group.label.kind !== "episode") continue;
      const { number } = group.label;
      // Every group on an entry's page is one of its episodes: the entry's season.
      const season = currentAnimeSeason;
      for (const { source, label, explicit } of packs) {
        if (label.episodeRange !== null || group.releases.includes(source)) continue;
        const files = details[source.view_url]?.files ?? [];
        // A file with no season anywhere takes its pack's single named
        // season, else season 1: an unmarked pack numbers from the start
        // (absolute numbering would otherwise hand season 1's file to any season).
        const named = label.seasons ?? [];
        const unmarkedSeason = named.length === 1 ? named[0] : 1;
        const covers =
          files.length > 0
            ? files.some((f) => f.episode === number && (f.season ?? unmarkedSeason) === season)
            : explicit && (label.season === season || (label.seasons ?? []).includes(season));
        if (covers) group.releases.push(source);
      }
    }
    // Rank episodes before batches before genuinely unknown titles; within
    // episodes, sort by season then episode number so a title search that
    // legitimately spans multiple seasons (see SEASON_EPISODE_PATTERN's doc
    // comment) doesn't interleave them by raw episode number.
    const rank = (label: EpisodeLabel) => (label.kind === "episode" ? 0 : label.kind === "batch" ? 1 : 2);
    const groupEntries = Array.from(groups.entries());
    if (debugActive) for (const source of sources) {
      const row = mapping.get(source.view_url)!;
      if (row.status === "filtered") continue;
      row.mappedAs = groupEntries.filter(([, group]) => group.releases.some((release) => release.view_url === source.view_url)).map(([key]) => key);
      if (row.mappedAs.length > 0 && !row.mappedAs.every((key) => key === "Unknown")) {
        row.status = "mapped";
        row.reason = row.mappedAs.length > 1 ? "Mapped to multiple episode groups by its batch range or files" : `Mapped to ${row.mappedAs[0]}`;
        const pack = packs.find((entry) => entry.source.view_url === source.view_url);
        const destinationEvidence = row.mappedAs.map((destination) => {
          const group = groupEntries.find(([key]) => key === destination)?.[1];
          if (pack && group && group.label.kind === "episode") {
            const files = details[source.view_url]?.files ?? [];
            const namedSeason = (pack.label.seasons?.length ?? 0) === 1 ? pack.label.seasons?.[0] : 1;
            const episodeNumber = group.label.number;
            const file = files.find((candidate) => candidate.episode === episodeNumber && (candidate.season ?? namedSeason) === currentAnimeSeason);
            if (file) return `${destination} ← ${file.path}`;
            if (pack.explicit) return `${destination} ← pack title explicitly covers this season`;
          }
          return destination;
        });
        check(source, "Episode destination", "passed", destinationEvidence.join("; "));
      } else {
        row.status = "unresolved";
        row.reason = "Kept by the show matcher, but no episode number or matching pack file identifies a page row";
        check(source, "Episode group", "unresolved", row.reason);
      }
    }
    const sortedGroups = groupEntries.sort(([, a], [, b]) => {
      const rankDiff = rank(a.label) - rank(b.label);
      if (rankDiff !== 0) return rankDiff;
      if (a.label.kind === "episode" && b.label.kind === "episode") {
        const seasonDiff = a.label.season - b.label.season;
        if (seasonDiff !== 0) return seasonDiff;
        return a.label.number - b.label.number;
      }
      if (a.label.kind === "batch" && b.label.kind === "batch") {
        // Plain season packs before packs bundling extras (movies/specials).
        return a.label.season - b.label.season || Number(a.label.extras === true) - Number(b.label.extras === true);
      }
      return 0;
    });
    return { groups: sortedGroups, mapping: Array.from(mapping.values()) };
  }, [sources, details, currentAnimeSeason, currentEpisodeOffset, selectedAnime?.episodes, selectedAnime?.format, debugActive]);

  const customSources = selectedAnime ? customSourcesByMedia[selectedAnime.id] ?? [] : [];
  const customReleases = useMemo<NyaaResult[]>(() => customSources.flatMap((source) => {
    const season = source.season ?? currentAnimeSeason;
    const releaseFor = (label: EpisodeLabel): NyaaResult => ({
      title: source.title,
      magnet: source.magnet,
      torrent_url: "",
      view_url: `custom://${source.id}`,
      size: "",
      seeders: 0,
      leechers: 0,
      published: "",
      label,
    });
    if (source.kind === "episode" && source.episodeStart != null) {
      return [releaseFor({ kind: "episode", season, number: source.episodeStart, seasons: [season] })];
    }

    const batch: EpisodeLabel = {
      kind: "batch",
      season,
      episodeRange: source.episodeStart != null && source.episodeEnd != null ? [source.episodeStart, source.episodeEnd] : null,
      extras: false,
      seasons: [season],
    };
    if (source.kind !== "pack") return [releaseFor(batch)];

    // A stored pack is offered on each numbered episode row covered by its
    // metadata-only file list. Keep a Batch row when the pack has no usable
    // numbering; never let extras create episode rows or widen its range.
    const episodes = new Set<number>();
    for (const file of source.files) {
      if (!file.isVideo || isCustomExtraFile(file)) continue;
      const namedSeasons = file.label && "seasons" in file.label ? (file.label.seasons ?? []) : [];
      if (namedSeasons.length > 0 && !namedSeasons.includes(season)) continue;
      const numbers = file.episode != null
        ? [file.episode]
        : file.episodeRange != null && file.episodeRange[1] >= file.episodeRange[0]
          ? Array.from({ length: Math.min(file.episodeRange[1] - file.episodeRange[0] + 1, 1000) }, (_, index) => file.episodeRange![0] + index)
          : [];
      for (const number of numbers) {
        if (source.episodeStart != null && number < source.episodeStart) continue;
        if (source.episodeEnd != null && number > source.episodeEnd) continue;
        episodes.add(number);
      }
    }
    if (episodes.size === 0) return [releaseFor(batch)];
    return Array.from(episodes).sort((a, b) => a - b).map((number) => releaseFor({
      kind: "episode",
      season,
      number,
      seasons: [season],
    }));
  }), [customSources, currentAnimeSeason]);
  const groupedSources = useMemo(() => {
    const groups = sourceAnalysis.groups.map(([key, group]) => [key, { ...group, releases: [...group.releases] }] as [string, { label: EpisodeLabel; releases: NyaaResult[] }]);
    for (const release of customReleases) {
      const label = labelOf(release.label, release.title);
      const key = episodeLabelText(label);
      const existing = groups.find(([groupKey]) => groupKey === key);
      if (existing) existing[1].releases.push(release);
      else groups.push([key, { label, releases: [release] }]);
    }
    const rank = (label: EpisodeLabel) => label.kind === "episode" ? 0 : label.kind === "batch" ? 1 : 2;
    return groups.sort(([, a], [, b]) => {
      const rankDiff = rank(a.label) - rank(b.label);
      if (rankDiff) return rankDiff;
      if (a.label.kind === "episode" && b.label.kind === "episode") return a.label.season - b.label.season || a.label.number - b.label.number;
      if (a.label.kind === "batch" && b.label.kind === "batch") return a.label.season - b.label.season;
      return 0;
    });
  }, [sourceAnalysis.groups, customReleases]);

  async function saveCustomSource(input: string, title: string, kind: "episode" | "pack", season: number | null, start: number | null, end: number | null, sourceId?: number) {
    if (!selectedAnime || !isTauriAvailable()) throw new Error("Custom sources are available in the desktop app.");
    const saved = sourceId != null
      ? await invoke<CustomSource | null>("update_custom_source", { animeId: selectedAnime.id, id: sourceId, targetAnimeId: selectedAnime.id, input, title, kind, season, episodeStart: start, episodeEnd: end })
      : await invoke<CustomSource>("save_custom_source", { animeId: selectedAnime.id, input, title, kind, season, episodeStart: start, episodeEnd: end });
    if (!saved) throw new Error("The custom source no longer exists.");
    setCustomSourcesByMedia((current) => ({ ...current, [selectedAnime.id]: [...(current[selectedAnime.id] ?? []).filter((source) => source.id !== saved.id && source.infoHash !== saved.infoHash), saved] }));
  }
  async function removeCustomSource(id: number) {
    if (!selectedAnime || !isTauriAvailable()) return;
    await invoke<boolean>("remove_custom_source", { animeId: selectedAnime.id, id });
    setCustomSourcesByMedia((current) => ({ ...current, [selectedAnime.id]: (current[selectedAnime.id] ?? []).filter((source) => source.id !== id) }));
  }

  async function seasonForAnime(anime: AnimeMedia): Promise<number> {
    const mapped = entrySeasonByMedia[anime.id];
    if (mapped != null) return mapped;
    const englishSeason = anime.title.english ? extractSeasonNumber(anime.title.english) : 1;
    const fallback = englishSeason !== 1 ? englishSeason : extractSeasonNumber(anime.title.romaji ?? "");
    if (!isTauriAvailable()) return fallback;
    try {
      const resolved = await invoke<number | null>("get_entry_season", { title: anime.title, synonyms: anime.synonyms ?? [], anilistId: anime.id });
      return resolved ?? fallback;
    } catch (err) {
      console.warn("[attach-custom-source] season lookup failed", { animeId: anime.id, err: String(err) });
      return fallback;
    }
  }

  async function attachCustomSource(source: {
    anime: AnimeMedia;
    input: string;
    title: string;
    kind: "episode" | "pack";
    season: number;
    episodeStart: number | null;
    episodeEnd: number | null;
    sourceId?: number;
    sourceAnimeId?: number;
  }) {
    if (!isTauriAvailable()) throw new Error("Custom sources are available in the desktop app.");
    knownAnime.current.set(source.anime.id, source.anime);
    const saved = source.sourceId != null && source.sourceAnimeId != null
      ? await invoke<CustomSource | null>("update_custom_source", {
          animeId: source.sourceAnimeId,
          id: source.sourceId,
          targetAnimeId: source.anime.id,
          input: source.input,
          title: source.title,
          kind: source.kind,
          season: source.season,
          episodeStart: source.episodeStart,
          episodeEnd: source.episodeEnd,
        })
      : await invoke<CustomSource>("save_custom_source", {
          animeId: source.anime.id,
          input: source.input,
          title: source.title,
          kind: source.kind,
          season: source.season,
          episodeStart: source.episodeStart,
          episodeEnd: source.episodeEnd,
        });
    if (!saved) throw new Error("The custom source no longer exists.");
    setCustomSourcesByMedia((current) => {
      const next = { ...current };
      if (source.sourceId != null && source.sourceAnimeId != null && source.sourceAnimeId !== source.anime.id) {
        next[source.sourceAnimeId] = (next[source.sourceAnimeId] ?? []).filter((item) => item.id !== source.sourceId);
      }
      next[source.anime.id] = [...(next[source.anime.id] ?? []).filter((item) => item.id !== saved.id && item.infoHash !== saved.infoHash), saved];
      return next;
    });
  }

  const attachmentDialog = attachmentInput !== null && <AttachCustomSourceDialog
    input={attachmentInput}
    source={attachmentSource ?? undefined}
    seasonForAnime={seasonForAnime}
    onClose={() => { setAttachmentInput(null); setAttachmentSource(null); }}
    onAttach={attachCustomSource}
  />;

  const settingsPanel = settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} playbackActive={route.name === "anime" && route.watch != null} showUpdates={settingsShowUpdates} />;
  const settingsButton = (
    <button class="icon-button app-settings-button" onClick={() => { setSettingsShowUpdates(false); setSettingsOpen(true); }} aria-label="Settings" title="Settings">
      <SettingsIcon />
    </button>
  );
  const updateButton = updateState.update && (
    <button class="button button-update" onClick={() => { setSettingsShowUpdates(true); setSettingsOpen(true); }} title={`nyaa-stream v${updateState.update.version} is available`}>
      {updateState.status === "ready" ? "Restart update" : "Update"}
    </button>
  );
  const debugButton = import.meta.env.DEV && (
    <button class="icon-button app-debug-button" onClick={() => { location.hash = "#/debug"; }} aria-label="Debug" title="Source mapping analyzer">
      Debug
    </button>
  );

  if (import.meta.env.DEV && location.hash === "#/debug") return (
    <DebugPage
      query={query}
      animeResults={animeResults}
      searching={loading}
      selectedAnime={selectedAnime}
      entrySeason={currentAnimeSeason}
      sources={sources}
      sourcesLoading={sourcesLoading}
      error={error}
      sourceMapping={sourceAnalysis.mapping}
      onQueryChange={setQuery}
      onSearch={(q) => { backToSearch(); setQuery(q); return runSearch(q); }}
      onSelectAnime={(anime) => { knownAnime.current.set(anime.id, anime); void openAnime(anime, true); }}
      onReanalyze={() => { if (selectedAnime) void openAnime(selectedAnime, true); }}
    />
  );

  if (directPlay) return <main class="container"><PlayerView
    key={directPlay.source.view_url}
    anime={directPlay.anime}
    episodeKey="Custom source"
    progressKey="Custom source"
    episode={directPlay.episode}
    season={directPlay.season}
    releases={[directPlay.source]}
    onAttachCustomSource={(release) => { setAttachmentSource(null); setAttachmentInput(release.magnet); }}
    onClose={() => setDirectPlay(null)}
    onNext={null}
    nextLabel={null}
    playlist={[]}
    onSelectEpisode={() => {}}
  />{attachmentDialog}</main>;

  if (route.name === "anime" && selectedAnime?.id !== route.id) {
    // The route's anime is still loading (reload / back-forward into it).
    return (
      <div class={route.watch ? "player-view hls-player" : "container route-loading"}>
        {routeError ? (
          <div class="player-message player-error" role="alert">
            <p>{routeError}</p>
            <button class="button button-quiet" onClick={() => navigate({ name: "home" }, { replace: true })}>
              Go home
            </button>
          </div>
        ) : (
          <div class="player-loading">
            <Buffering progress={0} />
          </div>
        )}
      </div>
    );
  }

  if (route.name === "anime" && selectedAnime) {
    const animeId = selectedAnime.id;
    return (
      <main class="container">
        <div class="media-settings-anchor">{debugButton}{updateButton}{settingsButton}</div>
        <MediaPage
          key={animeId}
          anime={selectedAnime}
          kitsu={kitsuByMedia[selectedAnime.id] ?? null}
          artwork={artworkByMedia[selectedAnime.id] ?? null}
          groupedSources={groupedSources}
          customSources={customSources}
          onSaveCustomSource={saveCustomSource}
          onRemoveCustomSource={removeCustomSource}
          onMoveCustomSource={(source) => { setAttachmentSource(source); setAttachmentInput(source.magnet); }}
          entrySeason={entrySeasonByMedia[selectedAnime.id] ?? null}
          sourcesLoading={sourcesLoading}
          sourcesCount={sources.length}
          onRefreshReleases={() => { void openAnime(selectedAnime, false, true); }}
          watch={route.watch}
          onWatch={(watch: WatchTarget, replace?: boolean) => navigate({ name: "anime", id: animeId, watch }, { replace })}
          onCloseWatch={() => goBack({ name: "anime", id: animeId, watch: null })}
          error={error}
          onBack={() => goBack({ name: "home" })}
          inLibrary={isInLibrary(selectedAnime.id, library)}
          onAddToLibrary={() => handleAddToLibrary(selectedAnime)}
          onRemoveFromLibrary={() => handleRemoveFromLibrary(selectedAnime.id)}
        />
        {settingsPanel}
        {attachmentDialog}
      </main>
    );
  }

  return (
    <main class="container home-container">
      <header class="app-bar">
        <div class="app-brand" aria-label="nyaa-stream">
          <BrandMark />
          <span class="app-brand-word">
            nyaa<span>stream</span>
          </span>
        </div>
        <div class="search-bar-wrap">
          <SearchIcon size={18} class="search-bar-icon" />
          <input
            id="anime-search-input"
            class="search-bar"
            value={query}
            onInput={(e) => { setDirectMagnetError(null); setQuery(e.currentTarget.value); }}
            onKeyDown={(e) => { if (e.key === "Enter" && isCustomMagnetInput(query)) { e.preventDefault(); void playCustomMagnet(query); } }}
            onFocus={handleInputFocus}
            onBlur={handleInputBlur}
            placeholder="Search anime"
            autocomplete="off"
            role="combobox"
            aria-expanded={dropdownOpen}
            aria-controls="search-results"
          />

          {dropdownOpen && !query.trim() && recentSearches.length > 0 && (
            <div class="search-results-wrap" id="search-results">
              <div class="search-recent-header">
                <span>Recent searches</span>
                <button class="link-button" onMouseDown={(e) => e.preventDefault()} onClick={() => setRecentSearches(clearRecentSearches())}>
                  Clear
                </button>
              </div>
              <ul class="search-results">
                {recentSearches.map((recent) => (
                  <li key={recent} role="option" class="search-recent-item">
                    <a onMouseDown={(e) => e.preventDefault()} onClick={() => setQuery(recent)}>
                      <SearchIcon size={14} />
                      <span class="result-title">{recent}</span>
                    </a>
                    <button
                      class="search-recent-remove"
                      aria-label={`Remove ${recent}`}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => setRecentSearches(removeRecentSearch(recent))}
                    >
                      ×
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {dropdownOpen && query.trim() && (
            <div class="search-results-wrap" id="search-results" ref={resultsWrapRef}>
              {isCustomMagnetInput(query) && <ul class="search-results">
                <li><button class="search-result-row" onMouseDown={(e) => e.preventDefault()} onContextMenu={(event) => showCustomMagnetContextMenu(event, query)} onClick={() => void playCustomMagnet(query)}>
                  <span class="result-title">Play magnet</span><span class="result-status">Open this torrent directly</span>
                </button></li>
                {directMagnetError && <li><div class="search-status search-status-error" role="alert">{directMagnetError}</div></li>}
              </ul>}
              {!isCustomMagnetInput(query) && loading && (
                <ul class="search-results">
                  {Array.from({ length: 4 }, (_, i) => (
                    <li key={i}>
                      <div class="search-result-skeleton-row">
                        <div class="result-thumbnail result-thumbnail-skeleton" />
                        <div class="result-metadata">
                          <div class="skeleton-line skeleton-title" />
                          <div class="skeleton-line skeleton-meta" />
                        </div>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
              {!isCustomMagnetInput(query) && !loading && error && <div class="search-status search-status-error">{error}</div>}
              {!isCustomMagnetInput(query) && !loading && !error && animeResults.length === 0 && (
                <div class="search-status">No anime matches “{query}”. Try the romaji or English title.</div>
              )}
              {!isCustomMagnetInput(query) && !loading && animeResults.length > 0 && (
                <ul class="search-results" role="listbox">
                  {animeResults.slice(0, visibleResults).map((anime) => {
                    const seasonLabel = formatSeason(anime.season, anime.seasonYear);
                    const facts = [
                      anime.format?.replace("_", " "),
                      isMovie(anime)
                        ? anime.duration != null
                          ? `${anime.duration} min`
                          : null
                        : anime.episodes != null
                          ? `${anime.episodes} episodes`
                          : null,
                      seasonLabel,
                    ].filter(Boolean);
                    return (
                      <li key={anime.id} role="option" onContextMenu={(event) => showSearchAnimeContextMenu(event, anime)}>
                        <a onMouseDown={(e) => e.preventDefault()} onClick={() => pickAnime(anime)}>
                          <div class="result-thumbnail">
                            {anime.coverImage.large && <img src={anime.coverImage.large} alt="" loading="lazy" decoding="async" />}
                          </div>
                          <div class="result-metadata">
                            <div class="result-title">{displayTitle(anime.title)}</div>
                            <div class="result-status">
                              {anime.averageScore != null && <span class="result-score">{(anime.averageScore / 10).toFixed(1)}</span>}
                              {facts.join(", ")}
                            </div>
                          </div>
                        </a>
                      </li>
                    );
                  })}
                  {visibleResults < animeResults.length && <li ref={moreSentinelRef} class="search-more-sentinel" aria-hidden="true" />}
                </ul>
              )}
            </div>
          )}
        </div>
        <div class="app-bar-actions">{updateButton}{debugButton}{settingsButton}</div>
      </header>

      <HomePage
        library={library}
        latestEpisodes={latestEpisodes}
        latestEpisodesLoading={latestEpisodesLoading}
        kitsuByMedia={kitsuByMedia}
        artworkByMedia={artworkByMedia}
        torrentThumbnails={torrentThumbnails}
        onSelectAnime={pickAnime}
        onSelectEpisode={selectEpisode}
        onAddToLibrary={handleAddToLibrary}
        onRemoveFromLibrary={handleRemoveFromLibrary}
      />
      {settingsPanel}
      {attachmentDialog}
    </main>
  );
}

export default App;
