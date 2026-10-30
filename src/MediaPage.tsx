import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { displayTitle, formatSeason, isMovie, type AnimeMedia, type CustomSource, type KitsuMetadata, type NyaaResult, type TorrentDetails } from "./types";
import { episodeLabelText, extractSeasonNumber, labelOf, type EpisodeLabel } from "./episodeParser";
import { PlayerView } from "./PlayerView";
import { progressForAnime, setWatched, subscribeProgress, type ProgressEntry } from "./watchProgress";
import { BackIcon, CheckIcon, PlayIcon, PlusIcon } from "./icons";
import { Buffering } from "./Buffering";
import type { WatchTarget } from "./router";
import { useSettings } from "./settings";
import { cachedTorrentThumbnail, notifyThumbnailSaved, subscribeFrameSaved } from "./torrentThumbnail";
import { releaseGroup } from "./releases";
import { useEnhancedImage } from "./enhanceImage";
import { copyToClipboard, openContextMenu } from "./contextMenuStore";
import { getEpisodeStill, type Artwork } from "./artwork";
import { useProviderCredentials } from "./providerCredentials";

type SourceGroup = [string, { label: EpisodeLabel; releases: NyaaResult[] }];
const CUSTOM_EXTRA_FILE = /\b(OVA|OAD|ONA|SP|Specials?|NCOP|NCED|Extras?|PV|Trailer)\d*\b/i;

function isCustomExtraFile(file: CustomSource["files"][number]): boolean {
  return CUSTOM_EXTRA_FILE.test(file.path) || file.label?.kind === "special" || (file.label?.kind === "batch" && file.label.extras === true);
}

function episodesForCustomFile(file: CustomSource["files"][number]): number[] {
  if (file.episode != null) return [file.episode];
  const range = file.episodeRange ?? (file.label?.kind === "batch" ? file.label.episodeRange : null);
  if (!range || range[1] < range[0]) return [];
  return Array.from({ length: Math.min(range[1] - range[0] + 1, 1000) }, (_, index) => range[0] + index);
}

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
  artwork: Artwork | null;
  groupedSources: SourceGroup[];
  /** The entry's season from the backend; falls back to the episode group's. */
  entrySeason?: number | null;
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
  customSources: CustomSource[];
  onSaveCustomSource: (input: string, title: string, kind: "episode" | "pack", season: number | null, start: number | null, end: number | null, sourceId?: number) => Promise<void>;
  onRemoveCustomSource: (id: number) => Promise<void>;
  onMoveCustomSource: (source: CustomSource) => void;
}

function CustomSourcePicker({ sources, onSave, onRemove, onMove, defaultSeason, editSourceId, onEditComplete }: {
  sources: CustomSource[];
  onSave: Props["onSaveCustomSource"];
  onRemove: Props["onRemoveCustomSource"];
  onMove: Props["onMoveCustomSource"];
  defaultSeason: number;
  editSourceId: number | null;
  onEditComplete: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState<"episode" | "pack">("pack");
  const [season, setSeason] = useState(String(defaultSeason));
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editingSourceId, setEditingSourceId] = useState<number | null>(null);
  const [magnetPreview, setMagnetPreview] = useState<{ infoHash: string; name: string | null; trackers: number } | null>(null);
  const [files, setFiles] = useState<CustomSource["files"]>([]);
  function edit(source: CustomSource) {
    setEditingSourceId(source.id);
    setInput(source.magnet); setTitle(source.title); setKind(source.kind);
    setSeason(source.season?.toString() ?? String(defaultSeason)); setStart(source.episodeStart?.toString() ?? ""); setEnd(source.episodeEnd?.toString() ?? "");
    setOpen(true); setError(null); setMagnetPreview({ infoHash: source.infoHash, name: source.title, trackers: 0 }); setFiles(source.files);
  }
  useEffect(() => {
    if (editSourceId == null) return;
    const source = sources.find((candidate) => candidate.id === editSourceId);
    if (source) edit(source);
    onEditComplete();
  }, [editSourceId, sources]);
  async function save(event: Event) {
    event.preventDefault(); setBusy(true); setError(null);
    try {
      await onSave(input, title, kind, season ? Number(season) : null, start ? Number(start) : null, end ? Number(end) : null, editingSourceId ?? undefined);
      setEditingSourceId(null);
      setInput(""); setTitle(""); setSeason(""); setStart(""); setEnd("");
    } catch (err) { setError(String(err)); }
    finally { setBusy(false); }
  }
  async function inspect() {
    setError(null); setFiles([]);
    try {
      const parsed = await invoke<{ infoHash: string; name: string | null; trackers: number }>("parse_custom_magnet", { input });
      setMagnetPreview(parsed);
      const inspected = await invoke<CustomSource["files"]>("inspect_custom_magnet", { input });
      setFiles(inspected);
      const videos = inspected.filter((file) => file.isVideo && !isCustomExtraFile(file));
      if (videos.length === 0) throw new Error("No episode video files were found in this magnet.");
      const singleRange = videos.length === 1 ? episodesForCustomFile(videos[0]) : [];
      const isPack = videos.length >= 2 || singleRange.length > 1;
      const seasons = Array.from(new Set(videos.flatMap((file) => file.label && "seasons" in file.label ? file.label.seasons : [])));
      if (seasons.length === 1) setSeason(String(seasons[0]));
      if (!isPack) {
        setKind("episode");
        const onlyNumber = episodesForCustomFile(videos[0])[0];
        setStart(onlyNumber?.toString() ?? "");
        setEnd("");
      } else {
        setKind("pack");
        const episodes = videos.flatMap(episodesForCustomFile).sort((a, b) => a - b);
        if (episodes.length) { setStart(String(episodes[0])); setEnd(String(episodes[episodes.length - 1])); }
      }
    } catch (err) { setError(err instanceof Error ? err.message : String(err)); }
  }
  return <div class="custom-sources-picker">
    <button class="link-button" onClick={() => setOpen((value) => !value)}>{open ? "Close custom sources" : `Custom sources${sources.length ? ` · ${sources.length}` : ""}`}</button>
    {open && <div class="custom-source-panel">
      {sources.map((source) => <div class="custom-source-entry" key={source.id}>
        <span>{source.title} · {source.kind}{source.episodeStart != null ? ` · ${source.episodeStart}${source.episodeEnd && source.episodeEnd !== source.episodeStart ? `–${source.episodeEnd}` : ""}` : ""}</span>
        <button class="link-button" onClick={() => edit(source)}>Edit</button>
        <button class="link-button" onClick={() => onMove(source)}>Move…</button>
        <button class="link-button" onClick={() => void onRemove(source.id)}>Remove</button>
      </div>)}
      <form onSubmit={save} class="custom-source-form">
        <input value={input} onInput={(e) => { setInput(e.currentTarget.value); setMagnetPreview(null); setFiles([]); }} placeholder="Magnet link or 40 digit hash" required maxlength={4096} />
        <button type="button" class="button button-quiet" onClick={() => void inspect()}>Check magnet</button>
        <input value={title} onInput={(e) => setTitle(e.currentTarget.value)} placeholder="Name (optional)" maxlength={200} />
        <select value={kind} onChange={(e) => setKind(e.currentTarget.value as "episode" | "pack")}><option value="pack">Pack</option><option value="episode">Episode</option></select>
        <input type="number" min="1" value={season} onInput={(e) => setSeason(e.currentTarget.value)} placeholder="Season" />
        <input type="number" min="1" value={start} onInput={(e) => setStart(e.currentTarget.value)} placeholder="Episode / from" />
        <input type="number" min="1" value={end} onInput={(e) => setEnd(e.currentTarget.value)} placeholder="To (optional)" />
        <button class="button button-secondary" disabled={busy}>{busy ? "Saving…" : "Save source"}</button>
      </form>
      {magnetPreview && <p class="custom-source-preview">{magnetPreview.name || magnetPreview.infoHash} · {episodeLabelText(labelOf(undefined, magnetPreview.name || ""))} · {magnetPreview.trackers} trackers</p>}
      {files.length > 0 && <ul class="custom-source-files">{files.filter((file) => file.isVideo && !isCustomExtraFile(file)).map((file) => <li key={file.path}>{file.path}{file.episode != null ? ` · S${file.season ?? "?"} E${file.episode}` : file.episodeRange ? ` · E${file.episodeRange[0]}–${file.episodeRange[1]}` : " · Batch / unnumbered"}</li>)}</ul>}
      {error && <p class="error-banner" role="alert">{error}</p>}
    </div>}
  </div>;
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


/** Watch-progress key of an episode group: numbered episodes by number
 * ("Episode 5" - season-agnostic, like the player route and the cards that
 * link to it), others by their group key ("Batch", ...). Also the resume
 * buffer's and download cache's episode key. */
function progressKeyOf(key: string, group: { label: { kind: string; number?: number } }): string {
  return group.label.kind === "episode" && group.label.number != null ? `Episode ${group.label.number}` : key;
}
export function MediaPage({
  anime,
  kitsu,
  artwork,
  groupedSources,
  entrySeason = null,
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
  customSources,
  onSaveCustomSource,
  onRemoveCustomSource,
  onMoveCustomSource,
}: Props) {
  const seasonLabel = formatSeason(anime.season, anime.seasonYear);
  const movie = isMovie(anime);
  const settings = useSettings();
  const [editCustomSourceId, setEditCustomSourceId] = useState<number | null>(null);
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

  /** A group's watch progress: under its progress key (see `progressKeyOf`),
   * else under its group key (entries saved before keys were fixed). */
  const progressOf = (key: string, group: (typeof groupedSources)[number][1]) => progress[progressKeyOf(key, group)] ?? progress[key];

  // The first episode that isn't finished, after the most recently
  // watched one - what the primary button plays.
  const continueTarget = useMemo(() => {
    const episodes = visibleSources.filter(([, g]) => g.label.kind === "episode");
    if (episodes.length === 0) return visibleSources[0] ?? null;
    const latest = Object.values(progress).sort((a, b) => b.updatedAt - a.updatedAt)[0];
    if (!latest) return episodes[0];
    const latestIndex = episodes.findIndex(([key, g]) => progressKeyOf(key, g) === latest.episodeKey || key === latest.episodeKey);
    if (latestIndex < 0) return episodes[0];
    if (!latest.completed) return episodes[latestIndex];
    return episodes[latestIndex + 1] ?? episodes[latestIndex];
  }, [visibleSources, progress]);

  // Backdrop and episode thumbnails come from Kitsu (real wide banner and
  // better per-episode coverage than AniList - see src/kitsu.ts), falling
  // back to AniList's poster.
  const backdrop = artwork?.backdrop ?? kitsu?.background ?? anime.bannerImage ?? anime.coverImage.extraLarge ?? anime.coverImage.large;
  const enhancedBackdrop = useEnhancedImage(backdrop);
  const kitsuThumbnails = kitsu?.episodeThumbnails;
  const providerCredentials = useProviderCredentials();
  const [tmdbEpisodeStills, setTmdbEpisodeStills] = useState<Record<number, string>>({});
  const askedEpisodeStills = useRef(new Set<string>());
  const previousTmdbToken = useRef(providerCredentials.tmdbReadAccessToken);
  useEffect(() => {
    if (previousTmdbToken.current !== providerCredentials.tmdbReadAccessToken) {
      previousTmdbToken.current = providerCredentials.tmdbReadAccessToken;
      askedEpisodeStills.current.clear();
      setTmdbEpisodeStills({});
      console.info("[media] provider credentials changed; refreshing TMDB episode stills", { animeId: anime.id, hasTmdb: !!providerCredentials.tmdbReadAccessToken });
    }
    for (const [, group] of visibleSources) {
      if (group.label.kind !== "episode" || kitsuThumbnails?.[group.label.number]) continue;
      const episode = group.label.number;
      const key = `${anime.id}-${episode}`;
      if (askedEpisodeStills.current.has(key)) continue;
      askedEpisodeStills.current.add(key);
      console.debug("[media] request TMDB episode still", { animeId: anime.id, episode });
      void getEpisodeStill(anime.id, episode).then((still) => {
        if (still) setTmdbEpisodeStills((current) => current[episode] ? current : { ...current, [episode]: still.url });
      });
    }
  }, [anime.id, visibleSources, kitsuThumbnails, providerCredentials.tmdbReadAccessToken]);
  // Frames already on disk (the player's last frame, or a capture) fill in
  // episodes Kitsu has no picture for, instead of a bare number tile.
  const localThumbnails = useLocalThumbnails(
    anime.id,
    visibleSources.flatMap(([, group]) => (group.label.kind === "episode" && !kitsuThumbnails?.[group.label.number] ? [group.label.number] : [])),
  );
  const thumbnails = useMemo(() => ({ ...localThumbnails, ...tmdbEpisodeStills, ...kitsuThumbnails }), [localThumbnails, tmdbEpisodeStills, kitsuThumbnails]);
  const autoFrameChecks = useRef(new Set<string>());
  useEffect(() => {
    const known = new Set(["asw", "toonshub", "doomdos", "judas", "varyg", "anime time"]);
    for (const [, group] of visibleSources) {
      if (group.label.kind !== "episode" || thumbnails[group.label.number]) continue;
      const episode = group.label.number;
      const key = `${anime.id}-${episode}`;
      if (autoFrameChecks.current.has(key)) continue;
      const release = group.releases.find((candidate) => known.has((releaseGroup(candidate) ?? "").toLowerCase()));
      if (!release) continue;
      autoFrameChecks.current.add(key);
      void invoke<string | null>("build_view_page_thumbnail", {
        cacheKey: key,
        viewUrls: [release.view_url],
        groups: [releaseGroup(release) ?? ""],
        force: false,
      }).then((url) => { if (url) notifyThumbnailSaved(anime.id, episode, url); })
        .catch((err) => console.debug("[thumbnail-build] automatic screenshot lookup failed", { key, err: String(err) }));
    }
  }, [anime.id, visibleSources, thumbnails]);
  const [thumbnailBuild, setThumbnailBuild] = useState<{ current: number; total: number; episodes: number[]; failed: number; noSeed: number } | null>(null);
  const thumbnailCancelRef = useRef(false);
  const [thumbnailBuildDone, setThumbnailBuildDone] = useState<{ failed: number; noSeed: number } | null>(null);
  const missingThumbnailEpisodes = visibleSources.flatMap(([, group]) => group.label.kind === "episode" && !thumbnails[group.label.number] ? [group.label.number] : []);
  async function buildMissingThumbnails() {
    if (thumbnailBuild || missingThumbnailEpisodes.length === 0) return;
    const episodes = [...missingThumbnailEpisodes];
    thumbnailCancelRef.current = false;
    try { await invoke("reset_thumbnail_build"); }
    catch (err) { console.debug("[thumbnail-build] backend cancel state reset failed", { err: String(err) }); }
    setThumbnailBuildDone(null);
    setThumbnailBuild({ current: 0, total: episodes.length, episodes: [], failed: 0, noSeed: 0 });
    const groupName = (release: NyaaResult) => releaseGroup(release)?.toLowerCase() ?? "";
    const screenshotGroups = new Set(["asw", "toonshub", "doomdos", "judas", "varyg", "anime time"]);
    const noScreenshotGroups = new Set(["subsplease", "erai-raws", "ember", "dkb", "new-raws", "sankyuu", "minimtbb", "tsundere-raws", "yameii", "kaleido-subs"]);

    // First use release-page frames, which cost no torrent traffic.
    for (const [index, episode] of episodes.entries()) {
      if (thumbnailCancelRef.current) break;
      setThumbnailBuild({ current: index, total: episodes.length, episodes: [episode], failed: 0, noSeed: 0 });
      const row = visibleSources.find(([, group]) => group.label.kind === "episode" && group.label.number === episode);
      if (!row) continue;
      const releases = row[1].releases;
      const pages = [...releases].sort((a, b) => Number(screenshotGroups.has(groupName(b))) - Number(screenshotGroups.has(groupName(a))));
      for (const release of pages) {
        if (noScreenshotGroups.has(groupName(release))) continue;
        try {
          const url = await invoke<string | null>("build_view_page_thumbnail", { cacheKey: `${anime.id}-${episode}`, viewUrls: [release.view_url], groups: [releaseGroup(release) ?? ""], force: true });
          if (url) {
            notifyThumbnailSaved(anime.id, episode, url);
            break;
          }
        } catch (err) { console.warn("[thumbnail-build] release-page frame lookup failed", { episode, err: String(err) }); }
        if (thumbnailCancelRef.current) break;
      }
    }

    const sizeBytes = (value: string) => {
      const match = value.match(/([\d.]+)\s*(B|KIB|KB|MIB|MB|GIB|GB|TIB|TB)/i);
      if (!match) return Number.MAX_SAFE_INTEGER;
      const unit = match[2].toUpperCase();
      const power = unit.startsWith("T") ? 4 : unit.startsWith("G") ? 3 : unit.startsWith("M") ? 2 : unit.startsWith("K") ? 1 : 0;
      return Number(match[1]) * 1024 ** power;
    };
    const bytesPerEpisode = (release: NyaaResult, label: EpisodeLabel) => {
      const bytes = sizeBytes(release.size);
      if (bytes === Number.MAX_SAFE_INTEGER) return bytes;
      const episodes = label.kind === "batch" ? (label.episodeRange ? label.episodeRange[1] - label.episodeRange[0] + 1 : anime.episodes || 1) : 1;
      return bytes / episodes;
    };
    const minThumbnailSeeders = 3;
    type Candidate = { release: NyaaResult; fileIdx: number };
    type EpisodePlan = { episode: number; hadSeededSource: boolean; candidates: Candidate[] };
    const preliminary = new Map<number, NyaaResult[]>();
    const viewUrls = new Set<string>();
    let noSeed = 0;
    for (const episode of episodes) {
      if (thumbnailCancelRef.current || await cachedTorrentThumbnail(anime.id, episode)) continue;
      const row = visibleSources.find(([, group]) => group.label.kind === "episode" && group.label.number === episode);
      if (!row) continue;
      const candidates = row[1].releases.filter((release) => {
        if (release.seeders < 1) return false;
        const label = labelOf(release.label, release.title);
        if (label.kind === "episode") return label.number === episode;
        return label.kind === "batch" && label.episodeRange != null && episode >= label.episodeRange[0] && episode <= label.episodeRange[1];
      });
      if (candidates.length === 0) noSeed++;
      const ordered = [...candidates].sort((a, b) => {
        const labelA = labelOf(a.label, a.title);
        const labelB = labelOf(b.label, b.title);
        // Favor a viable swarm, then the least data for this episode. Do not
        // let an uploader or standalone-release preference outrank size.
        return Number(b.seeders >= minThumbnailSeeders) - Number(a.seeders >= minThumbnailSeeders)
          || bytesPerEpisode(a, labelA) - bytesPerEpisode(b, labelB)
          || b.seeders - a.seeders;
      }).slice(0, 3);
      preliminary.set(episode, ordered);
      for (const release of ordered) viewUrls.add(release.view_url);
    }

    let details: (TorrentDetails | null)[] = [];
    if (viewUrls.size > 0 && !thumbnailCancelRef.current) {
      try { details = await invoke<(TorrentDetails | null)[]>("get_torrent_details_batch", { viewUrls: [...viewUrls] }); }
      catch (err) { console.warn("[thumbnail-build] release file lists unavailable", { err: String(err) }); }
    }
    const detailsByUrl = new Map([...viewUrls].map((url, index) => [url, details[index] ?? null]));
    const plans: EpisodePlan[] = [];
    for (const [episode, ordered] of preliminary) {
      const candidates: Candidate[] = [];
      for (const release of ordered) {
        const fileList = detailsByUrl.get(release.view_url)?.files ?? [];
        const label = labelOf(release.label, release.title);
        const videoFiles = fileList.filter((file) => /\.(mkv|mp4|m4v|avi|webm|mov|ts|m2ts|wmv|flv)$/i.test(file.path));
        const matching = videoFiles.filter((file) => (file.episode === episode || (videoFiles.length === 1 && label.kind === "episode"))
          && (file.season == null || label.kind === "episode" && file.season === label.season || label.kind === "batch" && file.season === label.season));
        if (label.kind === "batch" && matching.length === 0) continue;
        const largestVideo = videoFiles.reduce((best, file) => !best || file.size > best.size ? file : best, undefined as (typeof videoFiles)[number] | undefined);
        candidates.push({ release, fileIdx: matching[0]?.index ?? largestVideo?.index ?? 0 });
      }
      plans.push({ episode, hadSeededSource: (preliminary.get(episode)?.length ?? 0) > 0, candidates });
    }

    const completed = new Set<number>();
    const maxCandidates = Math.max(0, ...plans.map((plan) => plan.candidates.length));
    for (let rank = 0; rank < maxCandidates && !thumbnailCancelRef.current; rank++) {
      const groups = new Map<string, { requests: { cacheKey: string; fileIdx: number; durationMinutes: number | null }[]; episodes: number[] }>();
      for (const plan of plans) {
        if (completed.has(plan.episode)) continue;
        const candidate = plan.candidates[rank];
        if (!candidate) continue;
        const group = groups.get(candidate.release.magnet) ?? { requests: [], episodes: [] };
        group.requests.push({ cacheKey: `${anime.id}-${plan.episode}`, fileIdx: candidate.fileIdx, durationMinutes: anime.duration });
        group.episodes.push(plan.episode);
        groups.set(candidate.release.magnet, group);
      }
      for (const [magnet, group] of groups) {
        if (thumbnailCancelRef.current) break;
        setThumbnailBuild({ current: completed.size, total: episodes.length, episodes: group.episodes, failed: 0, noSeed: 0 });
        let result: (string | null)[] | null = null;
        while (!thumbnailCancelRef.current && result == null) {
          try {
            result = await invoke<(string | null)[]>("capture_torrent_thumbnail_batch", { magnet, requests: group.requests, force: true });
          } catch (err) {
            if (String(err).includes("thumbnail-deferred")) {
              await new Promise((resolve) => window.setTimeout(resolve, 1000));
              continue;
            }
            console.warn("[thumbnail-build] batch capture failed", { episodes: group.episodes, err: String(err) });
            break;
          }
        }
        if (thumbnailCancelRef.current) break;
        group.episodes.forEach((episode, index) => {
          const url = result?.[index];
          if (url) { notifyThumbnailSaved(anime.id, episode, url); completed.add(episode); }
        });
        setThumbnailBuild({ current: completed.size, total: episodes.length, episodes: [], failed: 0, noSeed: 0 });
      }
    }
    let failed = 0;
    if (!thumbnailCancelRef.current) {
      for (const plan of plans) {
        if (!completed.has(plan.episode) && plan.hadSeededSource) failed++;
      }
    }
    setThumbnailBuild(null);
    setThumbnailBuildDone({ failed, noSeed });
    thumbnailCancelRef.current = false;
  }
  const paragraphs = anime.description ? descriptionParagraphs(anime.description) : [];
  const watchedCount = Object.values(progress).filter((p) => p.completed).length;
  const continueEntry = continueTarget ? progressOf(continueTarget[0], continueTarget[1]) : undefined;
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
          progress: progressOf(key, group) ?? null,
        };
      }),
    [visibleSources, thumbnails, progress],
  );

  /** Marks every numbered episode listed before `index` as watched. */
  function markPreviousWatched(index: number) {
    for (const [key, group] of visibleSources.slice(0, index)) {
      if (group.label.kind !== "episode" || progressOf(key, group)?.completed) continue;
      setWatched(anime, progressKeyOf(key, group), group.label.number, true);
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
      // Fixed for the player's lifetime (like its `key`): the group key can
      // change as release details arrive, which re-ran play_magnet and split
      // progress and the resume buffer across two keys.
      progressKey={watchLabel ?? playing[0]}
      episode={playing[1].label.kind === "episode" ? playing[1].label.number : null}
      season={entrySeason ?? (playing[1].label.kind === "episode" ? playing[1].label.season : null)}
      releases={playing[1].releases}
      onClose={onCloseWatch}
      onNext={next ? () => play(next[0], true) : null}
      nextLabel={next ? next[0] : null}
      playlist={playlist}
      onSelectEpisode={selectFromPlayer}
      onRemoveCustomSource={onRemoveCustomSource}
      onEditCustomSource={(id) => {
        setEditCustomSourceId(id);
        onCloseWatch();
      }}
      onMoveCustomSource={(id) => {
        const source = customSources.find((candidate) => candidate.id === id);
        if (source) onMoveCustomSource(source);
        onCloseWatch();
      }}
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
          {artwork?.logo
            ? <img class="media-logo" src={artwork.logo} alt={displayTitle(anime.title)} />
            : <h1 class="media-title">{displayTitle(anime.title)}</h1>}
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
            <CustomSourcePicker
              sources={customSources}
      onSave={onSaveCustomSource}
              onRemove={onRemoveCustomSource}
              onMove={onMoveCustomSource}
              defaultSeason={entrySeason ?? extractSeasonNumber(anime.title.english ?? anime.title.romaji ?? "")}
              editSourceId={editCustomSourceId}
              onEditComplete={() => setEditCustomSourceId(null)}
            />
            {!movie && missingThumbnailEpisodes.length > 0 && !watch && <button class="button button-secondary thumbnail-build-button" onClick={() => {
              if (thumbnailBuild) {
                thumbnailCancelRef.current = true;
                invoke("cancel_thumbnail_build").catch((err) => console.debug("[thumbnail-build] cancel request failed", { err: String(err) }));
              }
              else void buildMissingThumbnails();
            }}>
              {thumbnailBuild ? `Building ${thumbnailBuild.current}/${thumbnailBuild.total} · Cancel` : `Build thumbnails (${missingThumbnailEpisodes.length})`}
            </button>}
            {thumbnailBuildDone != null && (thumbnailBuildDone.failed > 0 || thumbnailBuildDone.noSeed > 0) && <span class="thumbnail-build-result">
              {[thumbnailBuildDone.failed > 0 ? `${thumbnailBuildDone.failed} failed` : "", thumbnailBuildDone.noSeed > 0 ? `${thumbnailBuildDone.noSeed} no seeded source` : ""].filter(Boolean).join(" · ")}
            </span>}
          </header>
          {error && <p class="error-banner">{error}</p>}
          {watchMissing && (
            <p class="error-banner">No release of {watchLabel} was found. Pick another episode below.</p>
          )}
          {!sourcesLoading && sourcesCount === 0 && customSources.length === 0 && !error && (
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
                const entry = progressOf(key, group);
                const watched = entry?.completed ?? false;
                const fraction = entry && !watched ? Math.min(1, entry.position / entry.duration) : 0;
                return (
                  <div
                    class={`video-row${watched ? " watched" : ""}${key === continueTarget?.[0] ? " current" : ""}`}
                    key={key}
                    style={{ "--i": index }}
                    onContextMenu={(event) => {
                      console.debug("[media] open episode context menu", { animeId: anime.id, episodeKey: key, watched });
                      openContextMenu(event, [
                        { label: "Play", action: () => play(key) },
                        { label: watched ? "Mark unwatched" : "Mark watched", action: () => setWatched(anime, key, episodeNumber, !watched) },
                        { label: "Copy episode title", action: () => copyToClipboard(key, "episode-title") },
                      ]);
                    }}
                  >

                    <button class="video-row-main" onClick={() => play(key)} aria-label={`Play ${key}`}>
                      <div class="video-thumbnail">
                        {thumbnail ? <img src={thumbnail} alt="" loading="lazy" /> : <span class="video-thumbnail-number">{episodeNumber ?? "All"}</span>}
                        {episodeNumber != null && thumbnailBuild?.episodes.includes(episodeNumber) && <span class="thumbnail-build-spinner" aria-label="Building thumbnail" />}
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
