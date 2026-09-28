import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import {
  episodeLabelText,
  extractSeasonNumber,
  parseEpisode,
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
import { fetchKitsuMetadata, kitsuSnapshot } from "./kitsu";
import { cachedTorrentThumbnail, fetchTorrentThumbnail } from "./torrentThumbnail";
import { MediaPage } from "./MediaPage";
import { HomePage } from "./HomePage";
import { SettingsPanel } from "./SettingsPanel";
import { SearchIcon, SettingsIcon } from "./icons";
import { Buffering } from "./Buffering";
import { goBack, navigate, useRoute, type WatchTarget } from "./router";
import {
  displayTitle,
  formatSeason,
  isMovie,
  MOVIE_KEY,
  type AiringEntry,
  type AnimeMedia,
  type KitsuMetadata,
  type NyaaResult,
  type TorrentDetails,
} from "./types";
import "./App.css";

const SEARCH_DEBOUNCE_MS = 350;
const MAX_DROPDOWN_RESULTS = 15;

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

/** A movie's page: one "Movie" group of every release that is the film -
 * titles the parser can't number (how movies are named), minus the
 * franchise's TV episodes and season packs that share its name, e.g.
 * Jujutsu Kaisen episodes in a "Jujutsu Kaisen 0" search. */
function movieSources(sources: NyaaResult[]): [string, { label: EpisodeLabel; releases: NyaaResult[] }][] {
  const releases = sources.filter((source) => {
    if (MOVIE_TITLE.test(source.title)) return true;
    const parsed = parseEpisode(source.title);
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
  const [details, setDetails] = useState<Record<string, TorrentDetails>>({});
  // Session-lifetime cache of both the nyaa.si search and the per-torrent
  // view-page scrape (see loadDetails), keyed by AniList id - same
  // never-invalidated-within-a-session pattern as kitsuByMedia/
  // episodeOffsetByMedia below. Revisiting an anime (back-and-forth from
  // the library/latest-episodes, or picking the same search result twice)
  // used to re-run the full nyaa.si search (now up to three queries, see
  // strip_season_suffix) and re-scrape every ambiguous title's view page
  // from scratch every single time.
  const [sourcesByMedia, setSourcesByMedia] = useState<Record<number, NyaaResult[]>>({});
  const [detailsByMedia, setDetailsByMedia] = useState<Record<number, Record<string, TorrentDetails>>>({});
  // Read synchronously: the route effect below looks anime up in it on the
  // very first render (a reload on an anime route).
  const [library, setLibrary] = useState<AnimeMedia[]>(getLibrary);
  const [settingsOpen, setSettingsOpen] = useState(false);
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

  const skipNextSearch = useRef(false);
  const blurTimeout = useRef<number | undefined>(undefined);

  useEffect(() => {
    loadLatestEpisodes();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [library]);

  useEffect(() => {
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
    const timer = window.setTimeout(() => runSearch(query), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  async function runSearch(q: string) {
    console.debug("[search_anime] invoked", { query: q });
    setLoading(true);
    setError(null);
    try {
      const results = isTauriAvailable()
        ? await invoke<AnimeMedia[]>("search_anime", { query: q })
        : await fallbackSearchAnime(q);
      console.info("[search_anime] succeeded", { query: q, count: results.length });
      setAnimeResults(results.slice(0, MAX_DROPDOWN_RESULTS));
    } catch (err) {
      console.error("[search_anime] failed", { query: q, err });
      setError(`Search failed: couldn't reach AniList (${err instanceof Error ? err.message : String(err)}).`);
      setAnimeResults([]);
    } finally {
      setLoading(false);
    }
  }

  function pickAnime(anime: AnimeMedia) {
    knownAnime.current.set(anime.id, anime);
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

  async function openAnime(anime: AnimeMedia) {
    const releaseQuery = displayTitle(anime.title);
    skipNextSearch.current = true;
    setQuery(releaseQuery);
    setDropdownOpen(false);
    setSelectedAnime(anime);
    setError(null);

    const cachedSources = sourcesByMedia[anime.id];
    if (cachedSources) {
      console.debug("[search_torrents_for_anime] served from cache", { anime: releaseQuery, count: cachedSources.length });
      setSources(cachedSources);
      setDetails(detailsByMedia[anime.id] ?? {});
      setSourcesLoading(false);
    } else {
      console.debug("[search_torrents_for_anime] invoked", { anime: releaseQuery, title: anime.title });
      setSources([]);
      setDetails({});
      setSourcesLoading(true);
      try {
        const results = isTauriAvailable()
          ? await invoke<NyaaResult[]>("search_torrents_for_anime", { title: anime.title })
          : await fallbackSearchTorrentsForAnime(anime.title);
        console.info("[search_torrents_for_anime] succeeded", { anime: releaseQuery, count: results.length });
        setSources(results);
        setSourcesByMedia((current) => ({ ...current, [anime.id]: results }));
        setSourcesLoading(false);
        // A movie's releases all go in one group, so there's no batch
        // ambiguity worth a view-page scrape per title (which also ran
        // into nyaa.si's rate limit - every movie title parses "unknown").
        if (!isMovie(anime)) await loadDetails(anime.id, results);
      } catch (err) {
        console.error("[search_torrents_for_anime] failed", { anime: releaseQuery, err });
        setError(`Couldn't load releases from nyaa.si (${err instanceof Error ? err.message : String(err)}). Check your connection and reopen this page.`);
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
    setLibrary((current) => addToLibrary(anime, current));
  }

  function handleRemoveFromLibrary(id: number) {
    setLibrary((current) => removeFromLibrary(id, current));
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
      (r) => parseSubmitterFromTitle(r.title) === null || parseEpisode(r.title).kind === "unknown",
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
    if (query.trim() && (animeResults.length > 0 || loading)) setDropdownOpen(true);
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
    const fromEnglish = selectedAnime.title.english ? extractSeasonNumber(selectedAnime.title.english) : 1;
    if (fromEnglish !== 1) return fromEnglish;
    return selectedAnime.title.romaji ? extractSeasonNumber(selectedAnime.title.romaji) : 1;
  }, [selectedAnime]);

  const currentEpisodeOffset = selectedAnime ? (episodeOffsetByMedia[selectedAnime.id] ?? 0) : 0;

  const groupedSources = useMemo(() => {
    if (selectedAnime && isMovie(selectedAnime)) return movieSources(sources);
    const groups = new Map<string, { label: EpisodeLabel; releases: NyaaResult[] }>();
    const addToGroup = (label: EpisodeLabel, source: NyaaResult) => {
      const key = episodeLabelText(label);
      const bucket = groups.get(key) ?? { label, releases: [] };
      bucket.releases.push(source);
      groups.set(key, bucket);
    };
    for (const source of sources) {
      const parsed = parseEpisode(source.title);
      // The title-regex parser can't tell a batch apart from a genuinely
      // unparseable title. When we have ground truth from the torrent's
      // view page (file_count > 1), trust that over the "Unknown" guess.
      // A title reaching "unknown" already means no season marker was
      // found in the text either, so season 1 here matches what
      // extractSeasonNumber's default would have produced anyway.
      const label: EpisodeLabel =
        parsed.kind === "unknown" && details[source.view_url]?.is_batch
          ? { kind: "batch", season: 1, episodeRange: null }
          : parsed;

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
      if (
        label.kind === "episode" &&
        currentEpisodeOffset > 0 &&
        selectedAnime?.episodes != null &&
        label.number > selectedAnime.episodes
      ) {
        const relative = label.number - currentEpisodeOffset;
        if (relative >= 1 && relative <= selectedAnime.episodes) {
          label.number = relative;
          label.season = currentAnimeSeason;
        }
      }

      // See currentAnimeSeason's doc comment - drop releases we're
      // confident belong to a different season of the same franchise
      // rather than this anime's own episodes.
      if (currentAnimeSeason !== 1 && label.kind !== "unknown" && label.season !== currentAnimeSeason) {
        continue;
      }

      // A batch with an explicit episode range (e.g. "E15-E28") actually
      // covers those specific episodes, so spread it into each of those
      // episode rows' source counts instead of one undifferentiated
      // "Batch" bucket — a batch without a stated range (most of them)
      // still can't be attributed to specific episodes, so it stays as-is.
      if (label.kind === "batch" && label.episodeRange !== null) {
        const [min, max] = label.episodeRange;
        for (let n = min; n <= max; n++) {
          addToGroup({ kind: "episode", season: label.season, number: n }, source);
        }
        continue;
      }

      addToGroup(label, source);
    }
    // Rank episodes before batches before genuinely unknown titles; within
    // episodes, sort by season then episode number so a title search that
    // legitimately spans multiple seasons (see SEASON_EPISODE_PATTERN's doc
    // comment) doesn't interleave them by raw episode number.
    const rank = (label: EpisodeLabel) => (label.kind === "episode" ? 0 : label.kind === "batch" ? 1 : 2);
    return Array.from(groups.entries()).sort(([, a], [, b]) => {
      const rankDiff = rank(a.label) - rank(b.label);
      if (rankDiff !== 0) return rankDiff;
      if (a.label.kind === "episode" && b.label.kind === "episode") {
        const seasonDiff = a.label.season - b.label.season;
        if (seasonDiff !== 0) return seasonDiff;
        return a.label.number - b.label.number;
      }
      if (a.label.kind === "batch" && b.label.kind === "batch") {
        return a.label.season - b.label.season;
      }
      return 0;
    });
  }, [sources, details, currentAnimeSeason, currentEpisodeOffset, selectedAnime?.episodes, selectedAnime?.format]);

  const settingsPanel = settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />;
  const settingsButton = (
    <button class="icon-button app-settings-button" onClick={() => setSettingsOpen(true)} aria-label="Settings" title="Settings">
      <SettingsIcon />
    </button>
  );

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
        <div class="media-settings-anchor">{settingsButton}</div>
        <MediaPage
          key={animeId}
          anime={selectedAnime}
          kitsu={kitsuByMedia[selectedAnime.id] ?? null}
          groupedSources={groupedSources}
          sourcesLoading={sourcesLoading}
          sourcesCount={sources.length}
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
      </main>
    );
  }

  return (
    <main class="container home-container">
      <header class="app-bar">
        <div class="app-brand" aria-label="nyaa-stream">
          nyaa<span>stream</span>
        </div>
        <div class="search-bar-wrap">
          <SearchIcon size={18} class="search-bar-icon" />
          <input
            id="anime-search-input"
            class="search-bar"
            value={query}
            onInput={(e) => setQuery(e.currentTarget.value)}
            onFocus={handleInputFocus}
            onBlur={handleInputBlur}
            placeholder="Search anime"
            autocomplete="off"
            role="combobox"
            aria-expanded={dropdownOpen}
            aria-controls="search-results"
          />

          {dropdownOpen && (
            <div class="search-results-wrap" id="search-results">
              {loading && (
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
              {!loading && error && <div class="search-status search-status-error">{error}</div>}
              {!loading && !error && animeResults.length === 0 && (
                <div class="search-status">No anime matches “{query}”. Try the romaji or English title.</div>
              )}
              {!loading && animeResults.length > 0 && (
                <ul class="search-results" role="listbox">
                  {animeResults.map((anime) => {
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
                      <li key={anime.id} role="option">
                        <a onMouseDown={(e) => e.preventDefault()} onClick={() => pickAnime(anime)}>
                          <div class="result-thumbnail">
                            {anime.coverImage.large && <img src={anime.coverImage.large} alt="" />}
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
                </ul>
              )}
            </div>
          )}
        </div>
        {settingsButton}
      </header>

      <HomePage
        library={library}
        latestEpisodes={latestEpisodes}
        latestEpisodesLoading={latestEpisodesLoading}
        kitsuByMedia={kitsuByMedia}
        torrentThumbnails={torrentThumbnails}
        onSelectAnime={pickAnime}
        onSelectEpisode={selectEpisode}
      />
      {settingsPanel}
    </main>
  );
}

export default App;
