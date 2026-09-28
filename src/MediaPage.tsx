import { useEffect, useMemo, useState } from "preact/hooks";
import { displayTitle, formatSeason, type AnimeMedia, type KitsuMetadata, type NyaaResult } from "./types";
import type { EpisodeLabel } from "./episodeParser";
import { PlayerView } from "./PlayerView";
import { progressForAnime, setWatched, subscribeProgress, type ProgressEntry } from "./watchProgress";
import { BackIcon, CheckIcon, PlayIcon, PlusIcon } from "./icons";
import { Buffering } from "./Buffering";

type SourceGroup = [string, { label: EpisodeLabel; releases: NyaaResult[] }];

interface Props {
  anime: AnimeMedia;
  kitsu: KitsuMetadata | null;
  groupedSources: SourceGroup[];
  sourcesLoading: boolean;
  sourcesCount: number;
  /** Set when the user clicked a specific episode (Latest Episodes /
   * Continue Watching) rather than the anime in general - auto-plays it
   * once sources finish loading, then reports back via onAutoplayHandled. */
  autoplayEpisode: number | null;
  onAutoplayHandled: () => void;
  error: string | null;
  onBack: () => void;
  inLibrary: boolean;
  onAddToLibrary: () => void;
  onRemoveFromLibrary: () => void;
}

// AniList's `description(asHtml: false)` has been observed to still leave
// stray <br> tags - normalize those to paragraph breaks and drop anything
// else that looks like a tag.
function descriptionParagraphs(description: string): string[] {
  return description
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .split(/\n+/)
    .map((p) => p.trim())
    .filter(Boolean);
}

function VideoRowSkeleton() {
  return (
    <div class="video-row video-row-skeleton">
      <div class="video-thumbnail video-thumbnail-skeleton" />
      <div class="video-info">
        <div class="skeleton-line skeleton-title" />
        <div class="skeleton-line skeleton-meta" />
      </div>
    </div>
  );
}

function useAnimeProgress(animeId: number): Record<string, ProgressEntry> {
  const [progress, setProgress] = useState(() => progressForAnime(animeId));
  useEffect(() => {
    setProgress(progressForAnime(animeId));
    return subscribeProgress(() => setProgress(progressForAnime(animeId)));
  }, [animeId]);
  return progress;
}

function remainingLabel(entry: ProgressEntry): string {
  const minutes = Math.max(1, Math.round((entry.duration - entry.position) / 60));
  return `${minutes} min left`;
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
  // Index into groupedSources of the group being played.
  const [playingKey, setPlayingKey] = useState<string | null>(null);
  const progress = useAnimeProgress(anime.id);
  const [expanded, setExpanded] = useState(false);
  // Entered from an episode card (Continue watching / New episodes): go
  // straight to the player without ever showing this page, and closing
  // the player goes back where the user came from rather than here.
  const [directEpisode] = useState(autoplayEpisode);
  const [directMissing, setDirectMissing] = useState(false);
  const direct = directEpisode != null;

  // Esc leaves the loading shell the same way it closes the player.
  useEffect(() => {
    if (!direct || playingKey != null || directMissing) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onBack();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [direct, playingKey, directMissing, onBack]);

  const playingIndex = playingKey == null ? -1 : groupedSources.findIndex(([key]) => key === playingKey);
  const playing = playingIndex >= 0 ? groupedSources[playingIndex] : null;

  // Next = the following numbered episode of the same season, if its
  // releases are listed.
  const next = useMemo(() => {
    if (!playing || playing[1].label.kind !== "episode") return null;
    const { season, number } = playing[1].label;
    return (
      groupedSources.find(([, g]) => g.label.kind === "episode" && g.label.season === season && g.label.number === number + 1) ?? null
    );
  }, [playing, groupedSources]);

  // The first episode that isn't finished, after the most recently
  // watched one - what the primary button plays.
  const continueTarget = useMemo(() => {
    const episodes = groupedSources.filter(([, g]) => g.label.kind === "episode");
    if (episodes.length === 0) return groupedSources[0] ?? null;
    const latest = Object.values(progress).sort((a, b) => b.updatedAt - a.updatedAt)[0];
    if (!latest) return episodes[0];
    const latestIndex = episodes.findIndex(([key]) => key === latest.episodeKey);
    if (latestIndex < 0) return episodes[0];
    if (!latest.completed) return episodes[latestIndex];
    return episodes[latestIndex + 1] ?? episodes[latestIndex];
  }, [groupedSources, progress]);

  useEffect(() => {
    if (sourcesLoading || autoplayEpisode == null) return;
    const match = groupedSources.find(([, group]) => group.label.kind === "episode" && group.label.number === autoplayEpisode);
    if (match) setPlayingKey(match[0]);
    else {
      console.warn("[media] autoplay episode has no releases", { autoplayEpisode });
      if (direct) setDirectMissing(true);
    }
    onAutoplayHandled();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourcesLoading, groupedSources, autoplayEpisode]);

  // Backdrop and episode thumbnails come from Kitsu (real wide banner and
  // better per-episode coverage than AniList - see src/kitsu.ts), falling
  // back to AniList's poster.
  const backdrop = kitsu?.background ?? anime.coverImage.extraLarge ?? anime.coverImage.large;
  const thumbnails = kitsu?.episodeThumbnails ?? {};
  const paragraphs = anime.description ? descriptionParagraphs(anime.description) : [];
  const watchedCount = Object.values(progress).filter((p) => p.completed).length;
  const continueEntry = continueTarget ? progress[continueTarget[0]] : undefined;
  const continueLabel = continueTarget
    ? continueEntry && !continueEntry.completed
      ? `Resume ${continueTarget[0]}`
      : `Play ${continueTarget[0]}`
    : null;

  const player = playing && (
    <PlayerView
      key={playing[0]}
      anime={anime}
      episodeKey={playing[0]}
      episode={playing[1].label.kind === "episode" ? playing[1].label.number : null}
      releases={playing[1].releases}
      onClose={() => (direct ? onBack() : setPlayingKey(null))}
      onNext={next ? () => setPlayingKey(next[0]) : null}
      nextLabel={next ? next[0] : null}
      playlist={groupedSources.map(([key, group]) => {
        const number = group.label.kind === "episode" ? group.label.number : null;
        return {
          key,
          episode: number,
          thumbnail: number != null ? (thumbnails[number] ?? null) : null,
          releaseCount: group.releases.length,
          progress: progress[key] ?? null,
        };
      })}
      onSelectEpisode={setPlayingKey}
    />
  );

  if (direct && !directMissing) {
    // Player shell while nyaa.si releases load - same full-viewport frame
    // the player uses, so the hand-off to PlayerView doesn't flash.
    return (
      player || (
        <div class="player-view controls-shown">
          <div class="player-topbar">
            <button class="player-icon-button" onClick={onBack} aria-label="Back" title="Back (Esc)">
              <BackIcon size={22} />
            </button>
            <div class="player-heading">
              <div class="player-heading-title">{displayTitle(anime.title)}</div>
              <div class="player-heading-sub">Episode {directEpisode}</div>
            </div>
          </div>
          {error ? (
            <div class="player-message player-error" role="alert">
              <p>{error}</p>
              <button class="button button-quiet" onClick={onBack}>
                Go back
              </button>
            </div>
          ) : (
            <div class="player-loading">
              <Buffering progress={0} />
              <div class="player-loading-status">Finding releases on nyaa.si…</div>
            </div>
          )}
        </div>
      )
    );
  }

  return (
    <div class="media-page">
      {backdrop && (
        <div class="media-background-layer" aria-hidden="true">
          <img class="media-background-image" src={backdrop} alt="" />
        </div>
      )}

      <button class="back-button" onClick={onBack}>
        <BackIcon size={18} /> Home
      </button>

      <div class="media-content">
        <div class="media-info-column">
          {anime.coverImage.large && (
            <div class="media-poster">
              <img src={anime.coverImage.large} alt="" />
            </div>
          )}
          <h1 class="media-title">{displayTitle(anime.title)}</h1>
          {anime.title.native && anime.title.native !== displayTitle(anime.title) && (
            <div class="media-native-title" lang="ja">
              {anime.title.native}
            </div>
          )}
          <dl class="media-facts">
            {anime.averageScore != null && (
              <div>
                <dt>Score</dt>
                <dd>{(anime.averageScore / 10).toFixed(1)}</dd>
              </div>
            )}
            {anime.format && (
              <div>
                <dt>Format</dt>
                <dd>{anime.format.replace("_", " ")}</dd>
              </div>
            )}
            {anime.episodes != null && (
              <div>
                <dt>Episodes</dt>
                <dd>
                  {watchedCount > 0 ? `${watchedCount}/` : ""}
                  {anime.episodes}
                </dd>
              </div>
            )}
            {seasonLabel && (
              <div>
                <dt>Aired</dt>
                <dd>{seasonLabel}</dd>
              </div>
            )}
          </dl>
          <div class="media-actions">
            {continueTarget && !sourcesLoading && (
              <button class="button button-primary" onClick={() => setPlayingKey(continueTarget[0])}>
                <PlayIcon size={16} /> {continueLabel}
              </button>
            )}
            <button class={`button ${inLibrary ? "button-quiet" : "button-secondary"}`} onClick={inLibrary ? onRemoveFromLibrary : onAddToLibrary}>
              {inLibrary ? <CheckIcon size={16} /> : <PlusIcon size={16} />}
              {inLibrary ? "In library" : "Add to library"}
            </button>
          </div>
          {paragraphs.length > 0 && (
            <div class={`media-description${expanded ? " expanded" : ""}`}>
              {paragraphs.map((p, i) => (
                <p key={i}>{p}</p>
              ))}
            </div>
          )}
          {paragraphs.join(" ").length > 420 && (
            <button class="link-button" onClick={() => setExpanded((e) => !e)}>
              {expanded ? "Show less" : "Read more"}
            </button>
          )}
        </div>

        <section class="videos-list-panel" aria-label="Episodes">
          <header class="videos-list-header">
            <h2>Episodes</h2>
            {!sourcesLoading && sourcesCount > 0 && <span>{sourcesCount} releases on nyaa.si</span>}
          </header>
          {error && <p class="error-banner">{error}</p>}
          {directMissing && (
            <p class="error-banner">No release of Episode {directEpisode} was found. Pick another episode below.</p>
          )}
          {!sourcesLoading && sourcesCount === 0 && !error && (
            <p class="empty-state">Nobody has uploaded this to nyaa.si yet, or it's listed under a different title.</p>
          )}

          <div class="videos-list">
            {sourcesLoading && Array.from({ length: 6 }, (_, i) => <VideoRowSkeleton key={i} />)}

            {!sourcesLoading &&
              groupedSources.map(([key, group]) => {
                const episodeNumber = group.label.kind === "episode" ? group.label.number : null;
                const thumbnail = episodeNumber != null ? thumbnails[episodeNumber] : undefined;
                const entry = progress[key];
                const watched = entry?.completed ?? false;
                const fraction = entry && !watched ? Math.min(1, entry.position / entry.duration) : 0;
                return (
                  <div class={`video-row${watched ? " watched" : ""}${key === continueTarget?.[0] ? " current" : ""}`} key={key}>
                    <button class="video-row-main" onClick={() => setPlayingKey(key)} aria-label={`Play ${key}`}>
                      <div class="video-thumbnail">
                        {thumbnail ? <img src={thumbnail} alt="" loading="lazy" /> : <span class="video-thumbnail-number">{episodeNumber ?? "All"}</span>}
                        <span class="video-thumbnail-play">
                          <PlayIcon size={22} />
                        </span>
                        {fraction > 0 && (
                          <span class="video-progress">
                            <span style={{ width: `${fraction * 100}%` }} />
                          </span>
                        )}
                      </div>
                      <div class="video-info">
                        <div class="video-title">{key}</div>
                        <div class="video-meta">
                          {group.releases.length} {group.releases.length === 1 ? "release" : "releases"}
                          {entry && !watched && ` · ${remainingLabel(entry)}`}
                          {group.label.kind === "batch" && " · pick the episode in the player"}
                        </div>
                      </div>
                    </button>
                    <button
                      class={`video-watched-toggle${watched ? " on" : ""}`}
                      onClick={() => setWatched(anime, key, episodeNumber, !watched)}
                      aria-pressed={watched}
                      title={watched ? "Mark as unwatched" : "Mark as watched"}
                    >
                      <CheckIcon size={16} />
                    </button>
                  </div>
                );
              })}
          </div>
        </section>
      </div>

      {player}
    </div>
  );
}
