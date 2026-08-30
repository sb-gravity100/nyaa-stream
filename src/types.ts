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
  averageScore: number | null;
  format: string | null;
  season: string | null;
  seasonYear: number | null;
  /** Typical per-episode runtime in minutes, per AniList. */
  duration: number | null;
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

export interface PlaySession {
  torrentId: number;
  /** Base HLS playlist URL - append `?duration=<seconds>` before use. */
  hlsUrl: string;
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
}

export function displayTitle(title: AnimeTitle): string {
  return title.english ?? title.romaji ?? title.native ?? "Untitled";
}

export function formatSeason(season: string | null, year: number | null): string | null {
  if (!season || year == null) return null;
  const label = season.charAt(0) + season.slice(1).toLowerCase();
  return `${label} ${year}`;
}
