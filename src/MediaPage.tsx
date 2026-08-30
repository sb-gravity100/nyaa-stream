import { useEffect, useState } from "preact/hooks";
import { displayTitle, formatSeason, type AnimeMedia, type KitsuMetadata, type NyaaResult } from "./types";
import type { EpisodeLabel } from "./episodeParser";
import { PlayerView } from "./PlayerView";

interface Props {
  anime: AnimeMedia;
  kitsu: KitsuMetadata | null;
  groupedSources: [string, { label: EpisodeLabel; releases: NyaaResult[] }][];
  sourcesLoading: boolean;
  sourcesCount: number;
  /** Set when the user clicked a specific episode (the Latest Episodes
   * row) rather than the anime in general - auto-plays it once sources
   * finish loading, then reports back via onAutoplayHandled. */
  autoplayEpisode: number | null;
  onAutoplayHandled: () => void;
  error: string | null;
  onBack: () => void;
  inLibrary: boolean;
  onAddToLibrary: () => void;
  onRemoveFromLibrary: () => void;
}

// AniList's `description(asHtml: false)` mostly already strips markup, but
// has been observed to still leave stray <br> tags — normalize those to
// paragraph breaks and drop anything else that looks like a tag rather
// than trust the API's html-stripping to be complete.
function descriptionParagraphs(description: string): string[] {
  return description
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .split(/\n+/)
    .map((p) => p.trim())
    .filter(Boolean);
}

// Mirrors stremio-web's Video/VideoPlaceholder row shape (verified against
// its source): a thumbnail box + title, one row per episode. Clicking a
// row does nothing yet — this panel is deliberately just the episode list
// (stremio-web's VideosList), not the source picker (StreamsList). Actual
// per-release source selection is meant to live as a dropdown on the video
// player once one exists, not here.
function VideoRowSkeleton() {
  return (
    <div class="video-row">
      <div class="video-thumbnail video-thumbnail-skeleton" />
      <div class="video-info">
        <div class="skeleton-line skeleton-title" />
      </div>
    </div>
  );
}

interface PlayerSession {
  title: string;
  releases: NyaaResult[];
}

export function MediaPage({
  anime,
  kitsu,
  groupedSources,
  sourcesLoading,
  sourcesCount,
  autoplayEpisode,
  onAutoplayHandled,
  error,
  onBack,
  inLibrary,
  onAddToLibrary,
  onRemoveFromLibrary,
}: Props) {
  const seasonLabel = formatSeason(anime.season, anime.seasonYear);
  const [player, setPlayer] = useState<PlayerSession | null>(null);

  function handlePlay(title: string, releases: NyaaResult[]) {
    setPlayer({ title, releases });
  }

  // Latest Episodes row: jump straight into the player for the clicked
  // episode once its sources have loaded, instead of leaving the user on
  // the episode list to click play themselves.
  useEffect(() => {
    if (sourcesLoading || autoplayEpisode == null) return;
    const match = groupedSources.find(
      ([, group]) => group.label.kind === "episode" && group.label.number === autoplayEpisode,
    );
    if (match) {
      const [key, group] = match;
      handlePlay(key, group.releases);
    }
    onAutoplayHandled();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourcesLoading, groupedSources, autoplayEpisode]);
  // stremio-web has no separate anime "logo" source (Cinemeta/Fanart.tv
  // supply that for movies/series; AniList and Kitsu, the anime-metadata
  // sources, don't), so its real detail page falls back to plain text with
  // no image at all there. We keep our poster instead since AniList gives
  // us real art and dropping it would be a strict downgrade, not fidelity.
  //
  // Backdrop and per-episode thumbnails both come from Kitsu (see
  // src/kitsu.ts / crates/kitsu-client) rather than AniList: Kitsu's
  // coverImage is a real 3360x800 wide banner (AniList's is a stretched
  // poster), and its episode thumbnails had full coverage for a show
  // AniList had none for at all (verified live, see kitsu-client's doc
  // comments). Falls back to AniList's poster when Kitsu has no mapping.
  const backdrop = kitsu?.background ?? anime.coverImage.extraLarge ?? anime.coverImage.large;
  const thumbnails = kitsu?.episodeThumbnails ?? {};

  return (
    <div class="media-page">
      {backdrop && (
        <div class="media-background-layer">
          <img class="media-background-image" src={backdrop} alt="" />
        </div>
      )}

      <button class="back-button" onClick={onBack} aria-label="Back to search">
        ← Back
      </button>

      <div class="media-content">
        <div class="media-info-column">
          <div class="media-poster">{anime.coverImage.large && <img src={anime.coverImage.large} alt="" />}</div>
          <h1 class="media-title">{displayTitle(anime.title)}</h1>
          <button class="library-toggle-button" onClick={inLibrary ? onRemoveFromLibrary : onAddToLibrary}>
            {inLibrary ? "✓ In Library" : "+ Add to Library"}
          </button>
          <div class="media-meta">
            {anime.averageScore != null && `★ ${(anime.averageScore / 10).toFixed(1)} ∙ `}
            {anime.format && <strong>{anime.format}</strong>}
            {anime.episodes != null && ` ∙ ${anime.episodes} Eps`}
            {seasonLabel && ` ∙ ${seasonLabel}`}
          </div>
          {anime.description && (
            <div class="media-description">
              {descriptionParagraphs(anime.description).map((p, i) => (
                <p key={i}>{p}</p>
              ))}
            </div>
          )}
        </div>

        <div class="media-spacing" />

        <div class="videos-list-panel">
          {error && <p class="error-banner">{error}</p>}
          {!sourcesLoading && sourcesCount === 0 && <p>No releases found on nyaa.si.</p>}

          {sourcesLoading &&
            Array.from({ length: 6 }, (_, i) => <VideoRowSkeleton key={i} />)}

          {!sourcesLoading &&
            groupedSources.map(([key, group]) => {
              const thumbnail = group.label.kind === "episode" ? thumbnails[group.label.number] : undefined;
              return (
                <div class="video-row" key={key}>
                  <div class="video-thumbnail">{thumbnail && <img src={thumbnail} alt="" />}</div>
                  <div class="video-info">
                    <div class="video-title">{key}</div>
                  </div>
                  <button class="video-play-button" onClick={() => handlePlay(key, group.releases)} aria-label={`Play ${key}`}>
                    ▶
                  </button>
                  <div class="video-sources-badge">{group.releases.length}</div>
                </div>
              );
            })}
        </div>
      </div>

      {player && (
        <PlayerView
          key={player.title}
          title={player.title}
          releases={player.releases}
          estimatedDurationMinutes={anime.duration}
          onClose={() => setPlayer(null)}
        />
      )}
    </div>
  );
}
