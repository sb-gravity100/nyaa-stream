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
import { fetchKitsuMetadata } from "./kitsu";
import { fetchTorrentThumbnail } from "./torrentThumbnail";
import { MediaPage } from "./MediaPage";
import { HomePage } from "./HomePage";
import {
  displayTitle,
  formatSeason,
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

function App() {
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [animeResults, setAnimeResults] = useState<AnimeMedia[]>([]);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [selectedAnime, setSelectedAnime] = useState<AnimeMedia | null>(null);
  // Set when the user clicks a specific episode (the Latest Episodes row)
  // rather than an anime in general (Library grid, search) - MediaPage
  // auto-plays this episode once its sources finish loading, then reports
  // back via onAutoplayHandled so it only fires once per selection.
  const [autoplayEpisode, setAutoplayEpisode] = useState<number | null>(null);
  const [sources, setSources] = useState<NyaaResult[]>([]);
  const [sourcesLoading, setSourcesLoading] = useState(false);
  const [details, setDetails] = useState<Record<string, TorrentDetails>>({});
  const [library, setLibrary] = useState<AnimeMedia[]>([]);
  const [latestEpisodes, setLatestEpisodes] = useState<AiringEntry[]>([]);
  const [latestEpisodesLoading, setLatestEpisodesLoading] = useState(false);
  // Shared by MediaPage (selected anime's backdrop + video thumbnails) and
  // HomePage (per-library-anime latest-episode thumbnails) - keyed by
  // AniList id, `null` meaning "fetched, Kitsu has no mapping" so callers
  // don't re-fetch a known miss.
  const [kitsuByMedia, setKitsuByMedia] = useState<Record<number, KitsuMetadata | null>>({});
  // Last-resort thumbnails pulled from the torrent itself, keyed by
  // "{mediaId}-{episode}" - only populated for latest-episode cards where
  // Kitsu and AniList both came up completely empty (see loadTorrentThumbnail
  // below); most cards never touch this at all.
  const [torrentThumbnails, setTorrentThumbnails] = useState<Record<string, string | null>>({});

  const skipNextSearch = useRef(false);
  const blurTimeout = useRef<number | undefined>(undefined);

  useEffect(() => {
    setLibrary(getLibrary());
  }, []);

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
      setError(String(err));
      setAnimeResults([]);
    } finally {
      setLoading(false);
    }
  }

  async function pickAnime(anime: AnimeMedia) {
    const releaseQuery = displayTitle(anime.title);
    console.debug("[search_torrents_for_anime] invoked", { anime: releaseQuery, title: anime.title });
    skipNextSearch.current = true;
    setQuery(releaseQuery);
    setDropdownOpen(false);
    setSelectedAnime(anime);
    setAutoplayEpisode(null);
    setSources([]);
    setDetails({});
    setSourcesLoading(true);
    setError(null);
    try {
      const results = isTauriAvailable()
        ? await invoke<NyaaResult[]>("search_torrents_for_anime", { title: anime.title })
        : await fallbackSearchTorrentsForAnime(anime.title);
      console.info("[search_torrents_for_anime] succeeded", { anime: releaseQuery, count: results.length });
      setSources(results);
      setSourcesLoading(false);
      await loadDetails(results);
    } catch (err) {
      console.error("[search_torrents_for_anime] failed", { anime: releaseQuery, err });
      setError(String(err));
      setSourcesLoading(false);
    }
    // Fire-and-forget: enriches the already-shown page with per-episode
    // thumbnails once they arrive, doesn't block anything above.
    loadAnimeDetails(anime.id);
    loadKitsuMetadata(anime.id);
  }

  // Latest Episodes row: jump straight to that episode's player rather than
  // just opening the anime's page - pickAnime resets autoplayEpisode to
  // null as part of its own state reset, so this has to set it *after*
  // calling pickAnime to win the batched update.
  function selectEpisode(anime: AnimeMedia, episode: number) {
    pickAnime(anime);
    setAutoplayEpisode(episode);
  }

  async function loadKitsuMetadata(id: number) {
    if (id in kitsuByMedia) return;
    const metadata = await fetchKitsuMetadata(id);
    setKitsuByMedia((current) => (id in current ? current : { ...current, [id]: metadata }));
  }

  async function loadTorrentThumbnail(entry: AiringEntry) {
    const key = `${entry.media.id}-${entry.episode}`;
    if (key in torrentThumbnails) return;
    const dataUri = await fetchTorrentThumbnail(entry.media.id, entry.episode, entry.media.title);
    setTorrentThumbnails((current) => (key in current ? current : { ...current, [key]: dataUri }));
  }

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

  async function loadDetails(results: NyaaResult[]) {
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

  const groupedSources = useMemo(() => {
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
  }, [sources, details, currentAnimeSeason]);

  if (selectedAnime) {
    return (
      <main class="container">
        <MediaPage
          anime={selectedAnime}
          kitsu={kitsuByMedia[selectedAnime.id] ?? null}
          groupedSources={groupedSources}
          sourcesLoading={sourcesLoading}
          sourcesCount={sources.length}
          autoplayEpisode={autoplayEpisode}
          onAutoplayHandled={() => setAutoplayEpisode(null)}
          error={error}
          onBack={backToSearch}
          inLibrary={isInLibrary(selectedAnime.id, library)}
          onAddToLibrary={() => handleAddToLibrary(selectedAnime)}
          onRemoveFromLibrary={() => handleRemoveFromLibrary(selectedAnime.id)}
        />
      </main>
    );
  }

  return (
    <main class="container">
      <div class="search-bar-wrap">
        <input
          id="anime-search-input"
          class="search-bar"
          value={query}
          onInput={(e) => setQuery(e.currentTarget.value)}
          onFocus={handleInputFocus}
          onBlur={handleInputBlur}
          placeholder="Search an anime title..."
          autocomplete="off"
        />

        {dropdownOpen && (
          <div class="search-results-wrap">
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
              <div class="search-status">No results</div>
            )}
            {!loading && animeResults.length > 0 && (
              <ul class="search-results">
                {animeResults.map((anime) => {
                  const seasonLabel = formatSeason(anime.season, anime.seasonYear);
                  return (
                    <li key={anime.id}>
                      <a onMouseDown={(e) => e.preventDefault()} onClick={() => pickAnime(anime)}>
                        <div class="result-thumbnail">
                          {anime.coverImage.large && <img src={anime.coverImage.large} alt="" />}
                        </div>
                        <div class="result-metadata">
                          <div class="result-title">{displayTitle(anime.title)}</div>
                          <div class="result-status">
                            {anime.averageScore != null && `★ ${(anime.averageScore / 10).toFixed(1)} ∙ `}
                            {anime.format && <strong>{anime.format}</strong>}
                            {anime.episodes != null && ` ∙ ${anime.episodes} Eps`}
                            {seasonLabel && ` ∙ ${seasonLabel}`}
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

      <HomePage
        library={library}
        latestEpisodes={latestEpisodes}
        latestEpisodesLoading={latestEpisodesLoading}
        kitsuByMedia={kitsuByMedia}
        torrentThumbnails={torrentThumbnails}
        onSelectAnime={pickAnime}
        onSelectEpisode={selectEpisode}
      />
    </main>
  );
}

export default App;
