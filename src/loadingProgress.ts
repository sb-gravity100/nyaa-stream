import type { StreamStats } from "./types";
import type { MpvSbtlBuffer } from "./mpvVideo";

// Ported from stremio-web's useStatistics.ts (getLoadingProgress): a
// weighted "is this ready enough to start playing" score, not a raw
// percent-of-whole-file completion number - peers matter early (a healthy
// swarm), then downloaded bytes past a size-scaled threshold dominate, with
// download speed as a smaller tiebreaker. Capped at 99 so the bar never
// visually completes before playback actually starts.
const MB = 1024 * 1024;

export function loadingProgress(stats: StreamStats | null, mpvBuffer: MpvSbtlBuffer | null = null): number {
  if (stats === null && mpvBuffer === null) return 0;

  // Backends that track the player's buffer (sbtl) report real readiness:
  // verified playback time ahead of where the player reads, against the
  // threshold at which mpv starts (cache-pause-wait matches it).
  if (mpvBuffer) {
    if (mpvBuffer.state === "ready" || mpvBuffer.level === "full") return 99;
    return Math.min(99, (mpvBuffer.ahead_ms / Math.max(1, mpvBuffer.ready_ms)) * 99);
  }
  const buffer = stats?.buffer;
  if (buffer) {
    if (buffer.level === "ready" || buffer.level === "full") return 99;
    return Math.min(99, (buffer.aheadMs / Math.max(1, buffer.readyMs)) * 99);
  }
  if (stats === null) return 0;

  const peerScore = Math.min(1, stats.connectedPeers / 8) * 20;
  const minDownload = Math.min(8 * MB, Math.max(2 * MB, stats.totalBytes * 0.008));
  const downloadedScore = Math.min(1, stats.downloadedBytes / minDownload) * 70;
  const speedScore = Math.min(1, stats.downloadSpeedMbps) * 10;

  return Math.min(99, peerScore + downloadedScore + speedScore);
}
