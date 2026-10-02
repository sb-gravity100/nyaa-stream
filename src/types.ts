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
}

export interface TorrentDetails {
  submitter: string;
  is_batch: boolean;
  file_count: number;
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
  /** Base HLS playlist URL - append `?duration=<seconds>` before use. */
  hlsUrl: string;
  /** Raw Range-capable byte stream - what the embedded mpv opens. */
  streamUrl: string;
}

export interface PlaySession {
  /** Torrent info-hash (not a numeric session id - see torrent-engine's `TorrentId`). */
  torrentId: string;
  files: PlayFile[];
  /** Largest video file - fallback when no file matches the episode. */
  defaultFileIdx: number;
}

export interface SubtitleTrack {
  /** Absolute demuxer stream index - what identifies this track to the
   * backend (see torrent-engine's `SubtitleTrack` doc comment). */
  index: number;
  language: string | null;
  title: string | null;
  /** Source codec (`ass`, `subrip`, ...) - served as ASS either way. */
  codec: string;
  /** Container's own default-track flag. */
  default: boolean;
  /** Merged ASS script URL, polled by `assRenderer.ts` as it grows. */
  url: string;
}

export interface SubtitleInfo {
  tracks: SubtitleTrack[];
  /** Embedded font attachment URLs for the libass renderer. */
  fonts: string[];
}

/** Verified data ahead of the player's newest read (tl backend only). */
export interface BufferStatus {
  level: "stalled" | "low" | "ready" | "full";
  pos: number;
  aheadBytes: number;
  aheadMs: number;
  readyMs: number;
  etaReadyMs: number | null;
  rate: number;
  rateKnown: boolean;
}

export interface StreamStats {
  state: "initializing" | "live" | "paused" | "error";
  progressPercent: number;
  downloadSpeedMbps: number;
  connectedPeers: number;
  finished: boolean;
  downloadedBytes: number;
  totalBytes: number;
  /** How far into the file (seconds) HLS segments are actually produced
   * and instantly seekable - not the same as progressPercent, which
   * tracks raw torrent byte download and can run ahead of or behind
   * this (see torrent-engine's StreamStats doc comment). */
  readySeconds: number;
  /** Every produced `[start, end)` stretch in seconds - seek restarts
   * leave several disjoint ones. */
  readyRanges: [number, number][];
  /** The file's verified bytes as `[start, end)` runs (file-relative) - the
   * mpv player's seek bar maps them to time for its "downloaded" layer. */
  downloadedByteRanges: [number, number][];
  /** "direct" (stream copy) or e.g. "HEVC -> H.264 (h264_nvenc)" when the
   * streaming server is transcoding; null before the first segment. */
  videoMode: string | null;
  torrentName: string;
  fileName: string;
  uploadSpeedMbps: number;
  uploadedBytes: number;
  unchokedPeers: number;
  queuedPeers: number;
  swarmSize: number;
  sources: number;
  /** The live HLS run (torrent-engine's HlsRunInfo), null before one starts. */
  run: {
    startSeconds: number;
    segmentsProduced: number;
    speedXRealtime: number;
    running: boolean;
    subtitleTracks: number;
  } | null;
  /** null with the libtorrent backend or before the player reads. */
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
