import { displayTitle, type AiringEntry, type AnimeMedia, type KitsuMetadata } from "./types";

interface Props {
  library: AnimeMedia[];
  latestEpisodes: AiringEntry[];
  latestEpisodesLoading: boolean;
  kitsuByMedia: Record<number, KitsuMetadata | null>;
  torrentThumbnails: Record<string, string | null>;
  onSelectAnime: (anime: AnimeMedia) => void;
}

function formatAiringDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function LatestEpisodeSkeleton() {
  return (
    <li class="latest-episode-card">
      <div class="latest-episode-thumbnail latest-episode-thumbnail-skeleton" />
      <div class="skeleton-line skeleton-title" />
    </li>
  );
}

export function HomePage({
  library,
  latestEpisodes,
  latestEpisodesLoading,
  kitsuByMedia,
  torrentThumbnails,
  onSelectAnime,
}: Props) {
  return (
    <div class="home-page">
      <section class="home-section">
        <h2>Latest Episodes</h2>
        {library.length === 0 && (
          <p class="home-empty">Add anime to your library to see their latest episodes here.</p>
        )}
        {library.length > 0 && !latestEpisodesLoading && latestEpisodes.length === 0 && (
          <p class="home-empty">No episodes aired this week or last for your saved anime.</p>
        )}
        {library.length > 0 && (
          <ul class="latest-episodes-row">
            {latestEpisodesLoading &&
              Array.from({ length: 6 }, (_, i) => <LatestEpisodeSkeleton key={i} />)}
            {!latestEpisodesLoading &&
              latestEpisodes.map((entry) => {
                const kitsu = kitsuByMedia[entry.media.id];
                const episodeThumbnail = kitsu?.episodeThumbnails[entry.episode];
                // Last-resort torrent-captured frame only applies when
                // Kitsu has neither an episode thumbnail nor a backdrop for
                // this show at all (see App.tsx's loadTorrentThumbnail
                // trigger condition) - otherwise it's simply not fetched
                // and this is always undefined.
                const torrentThumbnail = torrentThumbnails[`${entry.media.id}-${entry.episode}`];
                const thumbnail =
                  episodeThumbnail ?? kitsu?.background ?? torrentThumbnail ?? entry.media.coverImage.large;
                return (
                  <li
                    key={`${entry.media.id}-${entry.episode}`}
                    class="latest-episode-card"
                    onClick={() =>
                      onSelectAnime({
                        id: entry.media.id,
                        title: entry.media.title,
                        coverImage: entry.media.coverImage,
                        description: null,
                        episodes: null,
                        averageScore: null,
                        format: null,
                        season: null,
                        seasonYear: null,
                      })
                    }
                  >
                    <div class="latest-episode-thumbnail">
                      {thumbnail && <img src={thumbnail} alt="" />}
                      <div class="latest-episode-badge">Ep {entry.episode}</div>
                    </div>
                    <div class="latest-episode-title">{displayTitle(entry.media.title)}</div>
                    <div class="latest-episode-date">{formatAiringDate(entry.airingAt)}</div>
                  </li>
                );
              })}
          </ul>
        )}
      </section>

      <section class="home-section">
        <h2>Library</h2>
        {library.length === 0 && (
          <p class="home-empty">Your library is empty — search for an anime and add it from its page.</p>
        )}
        {library.length > 0 && (
          <ul class="library-grid">
            {library.map((anime) => (
              <li key={anime.id} class="library-card" onClick={() => onSelectAnime(anime)}>
                <div class="library-thumbnail">
                  {anime.coverImage.large && <img src={anime.coverImage.large} alt="" />}
                </div>
                <div class="library-title">{displayTitle(anime.title)}</div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
