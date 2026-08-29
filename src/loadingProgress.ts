import type { StreamStats } from "./types";

// Ported from stremio-web's useStatistics.ts (getLoadingProgress): a
// weighted "is this ready enough to start playing" score, not a raw
// percent-of-whole-file completion number - peers matter early (a healthy
// swarm), then downloaded bytes past a size-scaled threshold dominate, with
// download speed as a smaller tiebreaker. Capped at 99 so the bar never
// visually completes before playback actually starts.
const MB = 1024 * 1024;

export function loadingProgress(stats: StreamStats | null): number {
  if (stats === null) return 0;

  const peerScore = Math.min(1, stats.connectedPeers / 8) * 20;
  const minDownload = Math.min(8 * MB, Math.max(2 * MB, stats.totalBytes * 0.008));
  const downloadedScore = Math.min(1, stats.downloadedBytes / minDownload) * 70;
  const speedScore = Math.min(1, stats.downloadSpeedMbps) * 10;

  return Math.min(99, peerScore + downloadedScore + speedScore);
}
