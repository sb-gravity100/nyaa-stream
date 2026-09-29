import { useEffect, useMemo, useState } from "preact/hooks";
import { displayTitle, type AiringEntry, type AnimeMedia, type KitsuMetadata } from "./types";
import { continueWatching, dismissContinueWatching, isEpisodeWatched, libraryStats, subscribeProgress, type ProgressEntry } from "./watchProgress";
import { PlayIcon, SearchIcon } from "./icons";
import { cachedTorrentThumbnail, subscribeFrameSaved } from "./torrentThumbnail";

interface Props {
  library: AnimeMedia[];
  latestEpisodes: AiringEntry[];
  latestEpisodesLoading: boolean;
  kitsuByMedia: Record<number, KitsuMetadata | null>;
  torrentThumbnails: Record<string, string | null>;
  onSelectAnime: (anime: AnimeMedia) => void;
  onSelectEpisode: (anime: AnimeMedia, episode: number) => void;
}

function formatAiringDate(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const days = Math.floor((Date.now() - date.getTime()) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function LatestEpisodeSkeleton() {
  return (
    <li class="episode-card">
      <div class="episode-card-thumbnail skeleton-block" />
      <div class="skeleton-line skeleton-title" />
    </li>
  );
}

function useContinueWatching(): ProgressEntry[] {
  const [entries, setEntries] = useState(() => continueWatching());
  useEffect(() => subscribeProgress(() => setEntries(continueWatching())), []);
  return entries;
}

type LibrarySort = "added" | "title" | "watched" | "unwatched";
const SORT_KEY = "nyaa-stream:library-sort";

function loadSort(): LibrarySort {
  try {
    const value = localStorage.getItem(SORT_KEY);
    return value === "title" || value === "watched" || value === "unwatched" ? value : "added";
  } catch {
    return "added";
  }
}

function useLibraryStats() {
  const [stats, setStats] = useState(() => libraryStats());
  useEffect(() => subscribeProgress(() => setStats(libraryStats())), []);
  return stats;
}

function asAnime(entry: ProgressEntry): AnimeMedia {
  return { ...entry.anime };
}

/** What the hero features: where the user left off, else the newest
 * episode from their library, else a library show. */
interface Featured {
  reason: string;
  anime: AnimeMedia;
  art: string | null;
  meta: string;
  progress: number | null;
  action: string;
  onPlay: () => void;
}

function focusSearch() {
  document.getElementById("anime-search-input")?.focus();
}

/** Seconds each candidate stays up before the hero moves to the next. */
const HERO_CYCLE_MS = 9000;

function Hero({
  featured,
  count,
  index,
  onSelect,
  onHoverChange,
}: {
  featured: Featured | null;
  count: number;
  index: number;
  onSelect: (index: number) => void;
  onHoverChange: (hovering: boolean) => void;
}) {
  if (!featured) {
    return (
      <section class="home-hero home-hero-empty">
        <div class="home-hero-body">
          <h1 class="home-hero-title">Find something to watch</h1>
          <p class="home-hero-meta">Search AniList for a show, add it to your library, and new episodes from nyaa.si will show up here as they air.</p>
          <div class="home-hero-actions">
            <button class="button button-primary" onClick={focusSearch}>
              <SearchIcon size={16} /> Search anime
            </button>
          </div>
        </div>
      </section>
    );
  }
  const { anime } = featured;
  const title = displayTitle(anime.title);
  return (
    <section class="home-hero" key={anime.id} onMouseEnter={() => onHoverChange(true)} onMouseLeave={() => onHoverChange(false)} onFocusIn={() => onHoverChange(true)} onFocusOut={() => onHoverChange(false)}>
      {featured.art && <img class="home-hero-art" src={featured.art} alt="" />}
      <div class="home-hero-body">
        <div class="home-hero-reason">{featured.reason}</div>
        <h1 class="home-hero-title">{title}</h1>
        {anime.title.native && anime.title.native !== title && (
          <div class="home-hero-native" lang="ja">
            {anime.title.native}
          </div>
        )}
        <p class="home-hero-meta">{featured.meta}</p>
        {featured.progress != null && (
          <div class="home-hero-progress">
            <span style={{ width: `${featured.progress * 100}%` }} />
          </div>
        )}
        <div class="home-hero-actions">
          <button class="button button-primary" onClick={featured.onPlay}>
            <PlayIcon size={16} /> {featured.action}
          </button>
        </div>
        {count > 1 && (
          <div class="home-hero-dots" role="tablist" aria-label="Featured shows">
            {Array.from({ length: count }, (_, i) => (
              <button
                key={i}
                role="tab"
                aria-selected={i === index}
                aria-label={`Show ${i + 1} of ${count}`}
                class={`home-hero-dot${i === index ? " active" : ""}`}
                onClick={() => onSelect(i)}
              >
                {i === index && <span style={{ animationDuration: `${HERO_CYCLE_MS}ms` }} />}
              </button>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function airingAsAnime(entry: AiringEntry): AnimeMedia {
  return {
    id: entry.media.id,
    title: entry.media.title,
    coverImage: entry.media.coverImage,
    description: null,
    episodes: null,
    averageScore: null,
    format: null,
    season: null,
    seasonYear: null,
    duration: entry.media.duration,
  };
}

export function HomePage({
  library,
  latestEpisodes,
  latestEpisodesLoading,
  kitsuByMedia,
  torrentThumbnails,
  onSelectAnime,
  onSelectEpisode,
}: Props) {
  const inProgress = useContinueWatching();
  const stats = useLibraryStats();
  const [sort, setSort] = useState(loadSort);
  const [filter, setFilter] = useState("");
  const shownLibrary = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const matches = needle
      ? library.filter((a) => [a.title.english, a.title.romaji, a.title.native].some((t) => t?.toLowerCase().includes(needle)))
      : [...library];
    const watched = (a: AnimeMedia) => stats.get(a.id)?.watched ?? 0;
    const unwatched = (a: AnimeMedia) => (a.episodes != null ? a.episodes - watched(a) : Infinity);
    if (sort === "title") matches.sort((a, b) => displayTitle(a.title).localeCompare(displayTitle(b.title)));
    else if (sort === "watched") matches.sort((a, b) => (stats.get(b.id)?.lastWatched ?? 0) - (stats.get(a.id)?.lastWatched ?? 0));
    else if (sort === "unwatched") matches.sort((a, b) => Number(unwatched(a) <= 0) - Number(unwatched(b) <= 0) || watched(b) - watched(a));
    return matches;
  }, [library, filter, sort, stats]);
  // Library shows with an episode out that hasn't been watched yet.
  const newEpisodeIds = useMemo(() => {
    const ids = new Set<number>();
    for (const entry of latestEpisodes) if (!isEpisodeWatched(entry.media.id, entry.episode)) ids.add(entry.media.id);
    return ids;
  }, [latestEpisodes, stats]);
  // Last frames saved by the player (or earlier captures) for the
  // Continue watching cards - where the user actually left off.
  const [lastFrames, setLastFrames] = useState<Record<string, string>>({});
  useEffect(() => {
    for (const entry of inProgress) {
      if (entry.episode == null) continue;
      const key = `${entry.animeId}-${entry.episode}`;
      cachedTorrentThumbnail(entry.animeId, entry.episode).then((url) => {
        if (url) setLastFrames((current) => (current[key] === url ? current : { ...current, [key]: url }));
      });
    }
  }, [inProgress]);
  useEffect(
    () => subscribeFrameSaved((key, url) => setLastFrames((current) => ({ ...current, [key]: url }))),
    [],
  );

  // Everything the hero can feature, best first: where the user left off,
  // then the newest library episodes, then library shows. It cycles through
  // them (paused while hovered) instead of pinning the first.
  const candidates = useMemo((): Featured[] => {
    const list: Featured[] = [];
    const seen = new Set<number>();
    const artFor = (id: number, anime: AnimeMedia) => kitsuByMedia[id]?.background ?? anime.bannerImage ?? anime.coverImage.extraLarge ?? anime.coverImage.large;
    for (const resume of inProgress.slice(0, 2)) {
      const anime = asAnime(resume);
      seen.add(anime.id);
      list.push({
        reason: "Pick up where you left off",
        anime,
        art: artFor(anime.id, anime),
        meta: `${resume.episodeKey} · ${Math.max(1, Math.round((resume.duration - resume.position) / 60))} min left`,
        progress: Math.min(1, resume.position / resume.duration),
        action: `Resume ${resume.episodeKey}`,
        onPlay: () => (resume.episode != null ? onSelectEpisode(anime, resume.episode) : onSelectAnime(anime)),
      });
    }
    for (const latest of latestEpisodes) {
      if (list.length >= 5) break;
      if (seen.has(latest.media.id)) continue;
      seen.add(latest.media.id);
      const anime = airingAsAnime(latest);
      list.push({
        reason: "New episode",
        anime,
        art: artFor(anime.id, anime),
        meta: `Episode ${latest.episode} · aired ${formatAiringDate(latest.airingAt).toLowerCase()}`,
        progress: null,
        action: `Play episode ${latest.episode}`,
        onPlay: () => onSelectEpisode(anime, latest.episode),
      });
    }
    for (const saved of library) {
      if (list.length >= 6) break;
      if (seen.has(saved.id)) continue;
      seen.add(saved.id);
      list.push({
        reason: "From your library",
        anime: saved,
        art: artFor(saved.id, saved),
        meta: [saved.format?.replace("_", " "), saved.episodes != null ? `${saved.episodes} episodes` : null].filter(Boolean).join(" · "),
        progress: null,
        action: "View episodes",
        onPlay: () => onSelectAnime(saved),
      });
    }
    return list;
  }, [inProgress, latestEpisodes, library, kitsuByMedia]);

  const [heroIndex, setHeroIndex] = useState(0);
  const [heroHovered, setHeroHovered] = useState(false);
  // Restarts the countdown after a manual pick or a change of candidates.
  const [heroEpoch, setHeroEpoch] = useState(0);
  const featured = candidates[Math.min(heroIndex, Math.max(0, candidates.length - 1))] ?? null;
  useEffect(() => {
    if (heroIndex >= candidates.length) setHeroIndex(0);
  }, [candidates.length]);
  useEffect(() => {
    if (candidates.length < 2 || heroHovered) return;
    const timer = window.setTimeout(() => setHeroIndex((i) => (i + 1) % candidates.length), HERO_CYCLE_MS);
    return () => window.clearTimeout(timer);
  }, [candidates.length, heroHovered, heroIndex, heroEpoch]);

  return (
    <div class="home-page">
      {featured?.art && (
        <div class="ambient" aria-hidden="true">
          <img key={featured.art} src={featured.art} alt="" />
        </div>
      )}
      <Hero
        featured={featured}
        count={candidates.length}
        index={heroIndex}
        onSelect={(i) => {
          setHeroIndex(i);
          setHeroEpoch((e) => e + 1);
        }}
        onHoverChange={setHeroHovered}
      />

      {inProgress.length > 0 && (
        <section class="home-section">
          <h2>
            Continue watching <span class="home-section-count">{inProgress.length}</span>
          </h2>
          <ul class="card-row">
            {inProgress.map((entry, index) => {
              const kitsu = kitsuByMedia[entry.animeId];
              const thumbnail =
                (entry.episode != null ? lastFrames[`${entry.animeId}-${entry.episode}`] : undefined) ??
                (entry.episode != null ? kitsu?.episodeThumbnails[entry.episode] : undefined) ??
                kitsu?.background ??
                entry.anime.bannerImage ??
                entry.anime.coverImage.extraLarge ??
                entry.anime.coverImage.large;
              const fraction = Math.min(1, entry.position / entry.duration);
              return (
                <li key={`${entry.animeId}-${entry.episodeKey}`} class="episode-card" style={{ "--i": index }}>
                  <button
                    class="card-button"
                    onClick={() => (entry.episode != null ? onSelectEpisode(asAnime(entry), entry.episode) : onSelectAnime(asAnime(entry)))}
                  >
                    <div class="episode-card-thumbnail">
                      {thumbnail && <img src={thumbnail} alt="" loading="lazy" />}
                      <span class="episode-card-play">
                        <PlayIcon size={26} />
                      </span>
                      <span class="video-progress">
                        <span style={{ width: `${fraction * 100}%` }} />
                      </span>
                    </div>
                    <div class="episode-card-title">{displayTitle(entry.anime.title)}</div>
                    <div class="episode-card-meta">
                      {entry.episodeKey} · {Math.max(1, Math.round((entry.duration - entry.position) / 60))} min left
                    </div>
                  </button>
                  <button
                    class="card-dismiss"
                    onClick={() => dismissContinueWatching(entry.animeId)}
                    aria-label={`Remove ${displayTitle(entry.anime.title)} from Continue watching`}
                    title="Remove from Continue watching"
                  >
                    ×
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      <section class="home-section">
        <h2>
          New episodes {latestEpisodes.length > 0 && <span class="home-section-count">{latestEpisodes.length}</span>}
        </h2>
        {library.length === 0 && (
          <p class="empty-state">Add shows to your library and their newest episodes will show up here as they air.</p>
        )}
        {library.length > 0 && !latestEpisodesLoading && latestEpisodes.length === 0 && (
          <p class="empty-state">Nothing new from your library in the last two weeks.</p>
        )}
        {library.length > 0 && (
          <ul class="card-row">
            {latestEpisodesLoading && Array.from({ length: 6 }, (_, i) => <LatestEpisodeSkeleton key={i} />)}
            {!latestEpisodesLoading &&
              latestEpisodes.map((entry, index) => {
                const kitsu = kitsuByMedia[entry.media.id];
                const episodeThumbnail = kitsu?.episodeThumbnails[entry.episode];
                // Last-resort torrent-captured frame, only fetched when
                // Kitsu has neither an episode thumbnail nor a backdrop
                // (see App.tsx's loadTorrentThumbnail trigger condition).
                const torrentThumbnail = torrentThumbnails[`${entry.media.id}-${entry.episode}`];
                const thumbnail = episodeThumbnail ?? kitsu?.background ?? torrentThumbnail ?? entry.media.coverImage.large;
                return (
                  <li key={`${entry.media.id}-${entry.episode}`} class="episode-card" style={{ "--i": index }}>
                    <button
                      class="card-button"
                      onClick={() => onSelectEpisode(airingAsAnime(entry), entry.episode)}
                    >
                      <div class="episode-card-thumbnail">
                        {thumbnail && <img src={thumbnail} alt="" loading="lazy" />}
                        <span class="episode-card-badge">Episode {entry.episode}</span>
                        <span class="episode-card-play">
                          <PlayIcon size={26} />
                        </span>
                      </div>
                      <div class="episode-card-title">{displayTitle(entry.media.title)}</div>
                      <div class="episode-card-meta">{formatAiringDate(entry.airingAt)}</div>
                    </button>
                  </li>
                );
              })}
          </ul>
        )}
      </section>

      <section class="home-section">
        <h2>
          Library {library.length > 0 && <span class="home-section-count">{library.length}</span>}
        </h2>
        {library.length === 0 && <p class="empty-state">Search for a show above, then add it to your library from its page.</p>}
        {library.length > 0 && (
          <div class="library-controls">
            <input
              class="library-filter"
              type="search"
              placeholder="Filter library"
              value={filter}
              onInput={(e) => setFilter(e.currentTarget.value)}
              aria-label="Filter library"
            />
            <select
              class="library-sort"
              value={sort}
              aria-label="Sort library"
              onChange={(e) => {
                const value = e.currentTarget.value as LibrarySort;
                setSort(value);
                try {
                  localStorage.setItem(SORT_KEY, value);
                } catch {
                  // Per-viewer convenience only.
                }
              }}
            >
              <option value="added">Recently added</option>
              <option value="title">Title</option>
              <option value="watched">Last watched</option>
              <option value="unwatched">Unwatched first</option>
            </select>
          </div>
        )}
        {library.length > 0 && shownLibrary.length === 0 && <p class="empty-state">Nothing in your library matches “{filter}”.</p>}
        {shownLibrary.length > 0 && (
          <ul class="library-grid">
            {shownLibrary.map((anime, index) => {
              const watched = stats.get(anime.id)?.watched ?? 0;
              return (
                <li key={anime.id} class="library-card" style={{ "--i": index }}>
                  <button class="card-button" onClick={() => onSelectAnime(anime)}>
                    <div class="library-thumbnail">
                      {anime.coverImage.large && <img src={anime.coverImage.large} alt="" loading="lazy" />}
                      {watched > 0 && (
                        <span class="library-badge">
                          {watched}
                          {anime.episodes != null ? `/${anime.episodes}` : ""}
                        </span>
                      )}
                      {newEpisodeIds.has(anime.id) && <span class="library-new-dot" title="New episode" aria-label="New episode" />}
                    </div>
                    <div class="library-title">{displayTitle(anime.title)}</div>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
