import type { EpisodeLabel } from "./episodeParser";
export interface AnimeTitle {
  romaji: string | null;
  english: string | null;
  native: string | null;
}

export interface CoverImage {
  large: string | null;
  extraLarge: string | null;
}

export interface AnimeMedia {
  id: number;
  title: AnimeTitle;
  description: string | null;
  episodes: number | null;
  coverImage: CoverImage;
  /** AniList's wide banner; backdrop fallback when Kitsu has no cover. */
  bannerImage?: string | null;
  averageScore: number | null;
  format: string | null;
  season: string | null;
  seasonYear: number | null;
  /** Typical per-episode runtime in minutes, per AniList. */
  duration: number | null;
  /** AniList airing status; absent on older saved snapshots. */
  status?: string | null;
  /** AniList alternative names, extra release-search terms. */
  synonyms?: string[];
  nextAiringEpisode?: { airingAt: number; episode: number } | null;
}

export interface NyaaResult {
  title: string;
  magnet: string;
  torrent_url: string;
  view_url: string;
  size: string;
  seeders: number;
  leechers: number;
  published: string;
  /** Parsed by the backend (release-parse); absent in the browser preview. */
  label?: EpisodeLabel;
  /** AnimeTosho's AniDB episode id (eid): releases sharing it are the same
   * episode. Set when the dataset knows the release; absent otherwise. */
  episode_group?: number | null;
  /** True when the dataset's trusted AniDB id put the release on this show:
   * the season filters in `groupedSources` then keep it. */
  aid_trusted?: boolean | null;
  /** Legacy/alternate-path marker for an unmarked release shared with another franchise entry. */
  season_ambiguous?: boolean | null;
}

export interface CustomSource {
  id: number;
  animeId: number;
  infoHash: string;
  magnet: string;
  title: string;
  kind: "episode" | "pack";
  season: number | null;
  episodeStart: number | null;
  episodeEnd: number | null;
  files: { path: string; size: number; index: number; season: number | null; episode: number | null; episodeRange?: [number, number] | null; isVideo: boolean; label?: EpisodeLabel | null }[];
  createdAt: number;
  updatedAt: number;
}

/** One file of a torrent, from its nyaa.si view page. */
export interface TorrentFile {
  /** Folders and file name joined with `/`. */
  path: string;
  size: number;
  /** Position among all files, in listed order. */
  index: number;
  /** Video files only; null = no season marker anywhere (unknown). */
  season: number | null;
  episode: number | null;
}

export interface TorrentDetails {
  submitter: string;
  is_batch: boolean;
  file_count: number;
  /** Absent in the browser preview and in details cached before v0.4.3. */
  files?: TorrentFile[];
  /** Union of the video files' seasons, ascending. */
  seasons?: number[];
}

export interface AiringMedia {
  id: number;
  title: AnimeTitle;
  coverImage: CoverImage;
  duration: number | null;
}

export interface AiringEntry {
  episode: number;
  airingAt: number;
  media: AiringMedia;
}

export interface KitsuMetadata {
  background: string | null;
  episodeThumbnails: Record<number, string>;
}

export interface PlayFile {
  index: number;
  /** Path inside the torrent. */
  name: string;
  length: number;
  isVideo: boolean;
  /** Raw HTTP Range stream for libmpv builds without stream callbacks. */
  streamUrl: string;
  /** Direct sbtl callback URL for libmpv builds that support stream callbacks. */
  mpvUrl?: string;
  /** The file name's episode label, parsed by the backend. */
  label?: EpisodeLabel;
}

export interface PlaySession {
  /** Torrent info-hash (not a numeric session id - see torrent-engine's `TorrentId`). */
  torrentId: string;
  /** Rejects cleanup from an older player after a new episode has started. */
  playbackGeneration: number;
  files: PlayFile[];
  /** Largest video file - fallback when no file matches the episode. */
  defaultFileIdx: number;
}

export interface SubtitleTrack {
  /** mpv's track id. */
  index: number;
  language: string | null;
  title: string | null;
  /** Source codec (`ass`, `subrip`, ...) - served as ASS either way. */
  codec: string;
  /** Container's own default-track flag. */
  default: boolean;
}

/** Verified data ahead of the player's newest read (sbtl backend only). */
export interface BufferStatus {
  level: "stalled" | "low" | "ready" | "full";
  pos: number;
  aheadBytes: number;
  aheadMs: number;
  readyMs: number;
  etaReadyMs: number | null;
  rate: number;
  rateKnown: boolean;
  fillRate: number;
  stalledMs: number;
}

export interface StreamStats {
  state: "initializing" | "live" | "paused" | "error";
  progressPercent: number;
  downloadSpeedMbps: number;
  connectedPeers: number;
  finished: boolean;
  downloadedBytes: number;
  totalBytes: number;
  /** The file's verified bytes as `[start, end)` runs (file-relative) - the
   * mpv player's seek bar maps them to time for its "downloaded" layer. */
  downloadedByteRanges: [number, number][];
  torrentName: string;
  fileName: string;
  uploadSpeedMbps: number;
  uploadedBytes: number;
  unchokedPeers: number;
  queuedPeers: number;
  swarmSize: number;
  sources: number;
  /** null before the player reads. */
  buffer?: BufferStatus | null;
}

/** AniList format MOVIE: one film, not episodes. */
export function isMovie(anime: { format: string | null }): boolean {
  return anime.format === "MOVIE";
}

/** Group key (and watch-progress key) of a movie's single release group.
 * It's labelled as episode 1, so `#/anime/:id/episode/1`, resume and
 * Continue watching all resolve to it like any episode. */
export const MOVIE_KEY = "Movie";

export function displayTitle(title: AnimeTitle): string {
  return title.english ?? title.romaji ?? title.native ?? "Untitled";
}

export function formatSeason(season: string | null, year: number | null): string | null {
  if (!season || year == null) return null;
  const label = season.charAt(0) + season.slice(1).toLowerCase();
  return `${label} ${year}`;
}
