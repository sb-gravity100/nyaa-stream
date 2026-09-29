import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { displayTitle, formatSeason, isMovie, type AnimeMedia, type KitsuMetadata, type NyaaResult } from "./types";
import type { EpisodeLabel } from "./episodeParser";
import { PlayerView } from "./PlayerView";
import { progressForAnime, setWatched, subscribeProgress, type ProgressEntry } from "./watchProgress";
import { BackIcon, CheckIcon, PlayIcon, PlusIcon } from "./icons";
import { Buffering } from "./Buffering";
import type { WatchTarget } from "./router";
import { useSettings } from "./settings";
import { cachedTorrentThumbnail, subscribeFrameSaved } from "./torrentThumbnail";
import { useEnhancedImage } from "./enhanceImage";

type SourceGroup = [string, { label: EpisodeLabel; releases: NyaaResult[] }];

/** The rows AniList/Kitsu know as real episodes: numbered episodes within
 * the episode count (AniList's, or Kitsu's when it's the fallback source).
 * Batch and Unknown rows are never listed; a movie's one group is episode
 * 1. A show still airing has no count yet - there every numbered episode
 * stays, since Kitsu's episode list trails new releases by a day or more
 * and would hide exactly the newest one. */
function listedSources(groups: SourceGroup[], anime: AnimeMedia): SourceGroup[] {
  const count = anime.episodes;
  return groups.filter(([, g]) => g.label.kind === "episode" && (count == null || (g.label.number >= 1 && g.label.number <= count)));
}

interface Props {
  anime: AnimeMedia;
  kitsu: KitsuMetadata | null;
  groupedSources: SourceGroup[];
  sourcesLoading: boolean;
  sourcesCount: number;
  /** The route's player target (`#/anime/:id/episode/:n` or `/play/:key`),
   * null on the plain anime page. */
  watch: WatchTarget | null;
  /** Opens the player on a group - `replace` for moving between episodes
   * inside the player, so back leaves the player instead of stepping
   * through every episode watched. */
  onWatch: (watch: WatchTarget, replace?: boolean) => void;
  /** Leaves the player: back to wherever it was opened from. */
  onCloseWatch: () => void;
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

type NextAiring = { airingAt: number; episode: number } | null;

// AniList details fetched once per anime per session, for snapshots (library,
// Continue watching) saved before `nextAiringEpisode` was requested.
const nextAiringCache = new Map<number, NextAiring>();

/** The next episode still to air, from the anime itself when it carries the
 * field, else one AniList lookup. */
function useNextAiring(anime: AnimeMedia): NextAiring {
  const [next, setNext] = useState<NextAiring>(() =>
    anime.nextAiringEpisode !== undefined ? anime.nextAiringEpisode : (nextAiringCache.get(anime.id) ?? null),
  );
  useEffect(() => {
    if (anime.nextAiringEpisode !== undefined) {
      setNext(anime.nextAiringEpisode);
      return;
    }
    if (anime.status === "FINISHED" || isMovie(anime) || nextAiringCache.has(anime.id)) return;
    let cancelled = false;
    invoke<AnimeMedia>("get_anime_details", { id: anime.id })
      .then((fresh) => {
        nextAiringCache.set(anime.id, fresh.nextAiringEpisode ?? null);
        if (!cancelled) setNext(fresh.nextAiringEpisode ?? null);
      })
      .catch((err) => console.debug("[media] next airing lookup failed", { id: anime.id, err: String(err) }));
    return () => {
      cancelled = true;
    };
  }, [anime.id]);
  return next;
}

function airingCountdown(airingAt: number): string {
  const seconds = airingAt - Date.now() / 1000;
  if (seconds <= 0) return "airing now";
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `in ${days}d ${hours}h`;
  if (hours > 0) return `in ${hours}h ${minutes}m`;
  return `in ${Math.max(1, minutes)}m`;
}

function remainingLabel(entry: ProgressEntry): string {
  const minutes = Math.max(1, Math.round((entry.duration - entry.position) / 60));
  return `${minutes} min left`;
}

/** Episode number -> thumbnail already in the local disk cache, for the
 * `episodes` asked about. Updates when the player saves a new last frame. */
function useLocalThumbnails(animeId: number, episodes: number[]): Record<number, string> {
  const [found, setFound] = useState<Record<number, string>>({});
  const asked = useMemo(() => new Set<number>(), [animeId]);
  const currentAnime = useRef(animeId);
  currentAnime.current = animeId;
  const wanted = episodes.join(",");

  useEffect(() => {
    setFound({});
  }, [animeId]);

  useEffect(() => {
    for (const episode of episodes) {
      if (asked.has(episode)) continue;
      asked.add(episode);
      void cachedTorrentThumbnail(animeId, episode).then((url) => {
        if (url && currentAnime.current === animeId) setFound((current) => ({ ...current, [episode]: url }));
      });
    }
  }, [animeId, wanted]);

  useEffect(
    () =>
      subscribeFrameSaved((key, url) => {
        const [id, episode] = key.split("-").map(Number);
        if (id === animeId && Number.isFinite(episode)) setFound((current) => ({ ...current, [episode]: url }));
      }),
    [animeId],
  );
  return found;
}

export function MediaPage({
  anime,
  kitsu,
  groupedSources,
  sourcesLoading,
  sourcesCount,
  watch,
  onWatch,
  onCloseWatch,
  error,
  onBack,
  inLibrary,
  onAddToLibrary,
  onRemoveFromLibrary,
}: Props) {
  const seasonLabel = formatSeason(anime.season, anime.seasonYear);
  const movie = isMovie(anime);
  const settings = useSettings();
  // What the page and the player's episode list show; the route still
  // resolves against every group, so a direct link to a hidden row plays.
  const visibleSources = useMemo(
    () => (settings.hideUnlistedSources ? listedSources(groupedSources, anime) : groupedSources),
    [settings.hideUnlistedSources, groupedSources, anime.episodes],
  );
  const hiddenCount = groupedSources.length - visibleSources.length;
  const progress = useAnimeProgress(anime.id);
  const nextAiring = useNextAiring(anime);
  // Re-renders the countdown once a minute.
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!nextAiring) return;
    const timer = window.setInterval(() => setTick((t) => t + 1), 60_000);
    return () => window.clearInterval(timer);
  }, [nextAiring]);
  const [expanded, setExpanded] = useState(false);
  // The group the route's player target resolves to, once releases load.
  // Numbered episodes match by number (season-agnostic, like the cards
  // that link to them).
  const playingKey = useMemo(() => {
    if (!watch) return null;
    if ("key" in watch) return groupedSources.some(([key]) => key === watch.key) ? watch.key : null;
    return groupedSources.find(([, g]) => g.label.kind === "episode" && g.label.number === watch.episode)?.[0] ?? null;
  }, [watch, groupedSources]);
  const watchLabel = watch ? ("key" in watch ? watch.key : `Episode ${watch.episode}`) : null;
  // A player route whose group has no releases: show this page with a note.
  const watchMissing = watch != null && playingKey == null && !sourcesLoading;
  useEffect(() => {
    if (watchMissing) console.warn("[media] player route has no releases", { watch });
  }, [watchMissing]);

  function play(key: string, replace = false) {
    const group = groupedSources.find(([k]) => k === key)?.[1];
    onWatch(group?.label.kind === "episode" ? { episode: group.label.number } : { key }, replace);
  }

  // Esc leaves the loading shell the same way it closes the player.
  useEffect(() => {
    if (!watch || playingKey != null || watchMissing) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onCloseWatch();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [watch, playingKey, watchMissing, onCloseWatch]);

  const playingIndex = playingKey == null ? -1 : groupedSources.findIndex(([key]) => key === playingKey);
  const playing = playingIndex >= 0 ? groupedSources[playingIndex] : null;

  // Next = the following numbered episode of the same season, if its
  // releases are listed.
  const next = useMemo(() => {
    if (!playing || playing[1].label.kind !== "episode") return null;
    const { season, number } = playing[1].label;
    return (
      visibleSources.find(([, g]) => g.label.kind === "episode" && g.label.season === season && g.label.number === number + 1) ?? null
    );
  }, [playing, visibleSources]);

  // The first episode that isn't finished, after the most recently
  // watched one - what the primary button plays.
  const continueTarget = useMemo(() => {
    const episodes = visibleSources.filter(([, g]) => g.label.kind === "episode");
    if (episodes.length === 0) return visibleSources[0] ?? null;
    const latest = Object.values(progress).sort((a, b) => b.updatedAt - a.updatedAt)[0];
    if (!latest) return episodes[0];
    const latestIndex = episodes.findIndex(([key]) => key === latest.episodeKey);
    if (latestIndex < 0) return episodes[0];
    if (!latest.completed) return episodes[latestIndex];
    return episodes[latestIndex + 1] ?? episodes[latestIndex];
  }, [visibleSources, progress]);

  // Backdrop and episode thumbnails come from Kitsu (real wide banner and
  // better per-episode coverage than AniList - see src/kitsu.ts), falling
  // back to AniList's poster.
  const backdrop = kitsu?.background ?? anime.bannerImage ?? anime.coverImage.extraLarge ?? anime.coverImage.large;
  const enhancedBackdrop = useEnhancedImage(backdrop);
  const kitsuThumbnails = kitsu?.episodeThumbnails;
  // Frames already on disk (the player's last frame, or a capture) fill in
  // episodes Kitsu has no picture for, instead of a bare number tile.
  const localThumbnails = useLocalThumbnails(
    anime.id,
    visibleSources.flatMap(([, group]) => (group.label.kind === "episode" && !kitsuThumbnails?.[group.label.number] ? [group.label.number] : [])),
  );
  const thumbnails = useMemo(() => ({ ...localThumbnails, ...kitsuThumbnails }), [localThumbnails, kitsuThumbnails]);
  const paragraphs = anime.description ? descriptionParagraphs(anime.description) : [];
  const watchedCount = Object.values(progress).filter((p) => p.completed).length;
  const continueEntry = continueTarget ? progress[continueTarget[0]] : undefined;
  const continueLabel = continueTarget
    ? continueEntry && !continueEntry.completed
      ? `Resume ${continueTarget[0]}`
      : `Play ${continueTarget[0]}`
    : null;

  // Memoized so the player's episode list only re-renders when an entry
  // actually changes, not on every MediaPage render.
  const playlist = useMemo(
    () =>
      visibleSources.map(([key, group]) => {
        const number = group.label.kind === "episode" ? group.label.number : null;
        return {
          key,
          episode: number,
          thumbnail: number != null ? (thumbnails[number] ?? null) : null,
          releaseCount: group.releases.length,
          progress: progress[key] ?? null,
        };
      }),
    [visibleSources, thumbnails, progress],
  );

  /** Marks every numbered episode listed before `index` as watched. */
  function markPreviousWatched(index: number) {
    for (const [key, group] of visibleSources.slice(0, index)) {
      if (group.label.kind !== "episode" || progress[key]?.completed) continue;
      setWatched(anime, key, group.label.number, true);
    }
  }

  // Stable for the memoized player playlist.
  const selectFromPlayer = useCallback((key: string) => play(key, true), [groupedSources, onWatch]);

  const player = playing && (
    <PlayerView
      // The route target, not the group key: a group can be relabeled as
      // release details arrive (batch detection, absolute numbering), which
      // remounted the player and restarted the torrent.
      key={watchLabel ?? playing[0]}
      anime={anime}
      episodeKey={playing[0]}
      episode={playing[1].label.kind === "episode" ? playing[1].label.number : null}
      releases={playing[1].releases}
      onClose={onCloseWatch}
      onNext={next ? () => play(next[0], true) : null}
      nextLabel={next ? next[0] : null}
      playlist={playlist}
      onSelectEpisode={selectFromPlayer}
    />
  );

  if (watch && !watchMissing) {
    // Player shell while nyaa.si releases load - same full-viewport frame
    // the player uses, so the hand-off to PlayerView doesn't flash.
    return (
      player || (
        <div class="player-view controls-shown">
          <div class="player-topbar">
            <button class="player-icon-button" onClick={onCloseWatch} aria-label="Back" title="Back (Esc)">
              <BackIcon size={22} />
            </button>
            <div class="player-heading">
              <div class="player-heading-title">{displayTitle(anime.title)}</div>
              <div class="player-heading-sub">{watchLabel}</div>
            </div>
          </div>
          {error ? (
            <div class="player-message player-error" role="alert">
              <p>{error}</p>
              <button class="button button-quiet" onClick={onCloseWatch}>
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
        <>
          <div class="ambient" aria-hidden="true">
            <img src={backdrop} alt="" />
          </div>
          <div class="media-background-layer" aria-hidden="true">
            <img class="media-background-image" src={enhancedBackdrop} alt="" />
          </div>
        </>
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
            {movie && anime.duration != null && (
              <div>
                <dt>Runtime</dt>
                <dd>{anime.duration} min</dd>
              </div>
            )}
            {!movie && anime.episodes != null && (
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
            {nextAiring && (
              <div>
                <dt>Next</dt>
                <dd>
                  Ep {nextAiring.episode} {airingCountdown(nextAiring.airingAt)}
                </dd>
              </div>
            )}
          </dl>
          <div class="media-actions">
            {continueTarget && !sourcesLoading && (
              <button class="button button-primary" onClick={() => play(continueTarget[0])}>
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

        <section class="videos-list-panel" aria-label={movie ? "Movie" : "Episodes"}>
          <header class="videos-list-header">
            <h2>
              {movie ? "Movie" : "Episodes"}
              {!sourcesLoading && visibleSources.length > 0 && <span class="home-section-count">{visibleSources.length}</span>}
            </h2>
            {!sourcesLoading && sourcesCount > 0 && (
              <span>
                {sourcesCount} releases on nyaa.si
                {hiddenCount > 0 && ` · ${hiddenCount} unlisted ${hiddenCount === 1 ? "row" : "rows"} hidden`}
              </span>
            )}
          </header>
          {error && <p class="error-banner">{error}</p>}
          {watchMissing && (
            <p class="error-banner">No release of {watchLabel} was found. Pick another episode below.</p>
          )}
          {!sourcesLoading && sourcesCount === 0 && !error && (
            <p class="empty-state">Nobody has uploaded this to nyaa.si yet, or it's listed under a different title.</p>
          )}
          {!sourcesLoading && visibleSources.length === 0 && hiddenCount > 0 && (
            <p class="empty-state">
              Only unlisted releases (batches or unrecognized titles) were found. Turn off “Hide unlisted sources” in Settings to see them.
            </p>
          )}

          <div class="videos-list">
            {sourcesLoading && Array.from({ length: 6 }, (_, i) => <VideoRowSkeleton key={i} />)}

            {!sourcesLoading &&
              visibleSources.map(([key, group], index) => {
                const episodeNumber = group.label.kind === "episode" ? group.label.number : null;
                // A movie's row shows its key art, not a "1".
                const thumbnail = movie ? (backdrop ?? undefined) : episodeNumber != null ? thumbnails[episodeNumber] : undefined;
                const entry = progress[key];
                const watched = entry?.completed ?? false;
                const fraction = entry && !watched ? Math.min(1, entry.position / entry.duration) : 0;
                return (
                  <div
                    class={`video-row${watched ? " watched" : ""}${key === continueTarget?.[0] ? " current" : ""}`}
                    key={key}
                    style={{ "--i": index }}
                  >

                    <button class="video-row-main" onClick={() => play(key)} aria-label={`Play ${key}`}>
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
                          {group.releases.every((r) => r.seeders === 0) && <span class="video-dead"> · no seeders</span>}
                        </div>
                      </div>
                    </button>
                    {index > 0 && !watched && (
                      <button
                        class="video-watched-toggle video-mark-previous"
                        onClick={() => markPreviousWatched(index)}
                        title="Mark all previous episodes as watched"
                        aria-label="Mark all previous episodes as watched"
                      >
                        ✓✓
                      </button>
                    )}
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
