import { useEffect, useMemo, useState } from "preact/hooks";
import { displayTitle, type AiringEntry, type AnimeMedia, type KitsuMetadata } from "./types";
import { continueWatching, subscribeProgress, type ProgressEntry } from "./watchProgress";
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

function Hero({ featured }: { featured: Featured | null }) {
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
    <section class="home-hero" key={anime.id}>
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

  const featured = useMemo((): Featured | null => {
    const resume = inProgress[0];
    if (resume) {
      const kitsu = kitsuByMedia[resume.animeId];
      const anime = asAnime(resume);
      return {
        reason: "Pick up where you left off",
        anime,
        art: kitsu?.background ?? anime.coverImage.extraLarge ?? anime.coverImage.large,
        meta: `${resume.episodeKey} · ${Math.max(1, Math.round((resume.duration - resume.position) / 60))} min left`,
        progress: Math.min(1, resume.position / resume.duration),
        action: `Resume ${resume.episodeKey}`,
        onPlay: () => (resume.episode != null ? onSelectEpisode(anime, resume.episode) : onSelectAnime(anime)),
      };
    }
    const latest = latestEpisodes[0];
    if (latest) {
      const kitsu = kitsuByMedia[latest.media.id];
      const anime = airingAsAnime(latest);
      return {
        reason: "New episode",
        anime,
        art: kitsu?.background ?? anime.coverImage.extraLarge ?? anime.coverImage.large,
        meta: `Episode ${latest.episode} · aired ${formatAiringDate(latest.airingAt).toLowerCase()}`,
        progress: null,
        action: `Play episode ${latest.episode}`,
        onPlay: () => onSelectEpisode(anime, latest.episode),
      };
    }
    const saved = library[0];
    if (saved) {
      const kitsu = kitsuByMedia[saved.id];
      return {
        reason: "From your library",
        anime: saved,
        art: kitsu?.background ?? saved.coverImage.extraLarge ?? saved.coverImage.large,
        meta: [saved.format?.replace("_", " "), saved.episodes != null ? `${saved.episodes} episodes` : null].filter(Boolean).join(" · "),
        progress: null,
        action: "View episodes",
        onPlay: () => onSelectAnime(saved),
      };
    }
    return null;
  }, [inProgress, latestEpisodes, library, kitsuByMedia]);

  return (
    <div class="home-page">
      {featured?.art && (
        <div class="ambient" aria-hidden="true">
          <img key={featured.art} src={featured.art} alt="" />
        </div>
      )}
      <Hero featured={featured} />

      {inProgress.length > 0 && (
        <section class="home-section">
          <h2>
            Continue watching <span class="home-section-count">{inProgress.length}</span>
          </h2>
          <ul class="card-row">
            {inProgress.map((entry) => {
              const kitsu = kitsuByMedia[entry.animeId];
              const thumbnail =
                (entry.episode != null ? lastFrames[`${entry.animeId}-${entry.episode}`] : undefined) ??
                (entry.episode != null ? kitsu?.episodeThumbnails[entry.episode] : undefined) ??
                kitsu?.background ??
                entry.anime.coverImage.extraLarge ??
                entry.anime.coverImage.large;
              const fraction = Math.min(1, entry.position / entry.duration);
              return (
                <li key={`${entry.animeId}-${entry.episodeKey}`} class="episode-card">
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
              latestEpisodes.map((entry) => {
                const kitsu = kitsuByMedia[entry.media.id];
                const episodeThumbnail = kitsu?.episodeThumbnails[entry.episode];
                // Last-resort torrent-captured frame, only fetched when
                // Kitsu has neither an episode thumbnail nor a backdrop
                // (see App.tsx's loadTorrentThumbnail trigger condition).
                const torrentThumbnail = torrentThumbnails[`${entry.media.id}-${entry.episode}`];
                const thumbnail = episodeThumbnail ?? kitsu?.background ?? torrentThumbnail ?? entry.media.coverImage.large;
                return (
                  <li key={`${entry.media.id}-${entry.episode}`} class="episode-card">
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
          <ul class="library-grid">
            {library.map((anime) => (
              <li key={anime.id} class="library-card">
                <button class="card-button" onClick={() => onSelectAnime(anime)}>
                  <div class="library-thumbnail">{anime.coverImage.large && <img src={anime.coverImage.large} alt="" loading="lazy" />}</div>
                  <div class="library-title">{displayTitle(anime.title)}</div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
