import { useEffect, useState } from "preact/hooks";
import { displayTitle, type AiringEntry, type AnimeMedia, type KitsuMetadata } from "./types";
import { continueWatching, subscribeProgress, type ProgressEntry } from "./watchProgress";
import { PlayIcon } from "./icons";
import { cachedTorrentThumbnail } from "./torrentThumbnail";

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
      cachedTorrentThumbnail(entry.animeId, entry.episode).then((dataUri) => {
        if (dataUri) setLastFrames((current) => (current[key] === dataUri ? current : { ...current, [key]: dataUri }));
      });
    }
  }, [inProgress]);

  return (
    <div class="home-page">
      {inProgress.length > 0 && (
        <section class="home-section">
          <h2>Continue watching</h2>
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
        <h2>New episodes</h2>
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
                      onClick={() =>
                        onSelectEpisode(
                          {
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
                          },
                          entry.episode,
                        )
                      }
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
        <h2>Library</h2>
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
