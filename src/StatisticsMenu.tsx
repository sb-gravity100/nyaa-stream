import { useEffect, useState } from "preact/hooks";
import type Hls from "hls.js";
import type { StreamStats } from "./types";
import { cachedTorrentThumbnail } from "./torrentThumbnail";

// Grew out of stremio-web's Player/StatisticsMenu (peers/speed/completed +
// info hash): now the full picture - torrent swarm, the streaming server's
// HLS run, what the browser is actually playing, and the episode's
// thumbnail.
interface Props {
  stats: StreamStats;
  infoHash: string | null;
  video: HTMLVideoElement | null;
  hls: Hls | null;
  animeId: number;
  episode: number | null;
  /** Active subtitle track's label, null when off. */
  subtitleLabel: string | null;
}

const REFRESH_MS = 1000;

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatTime(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Browser-side playback figures, sampled every second. */
function usePlaybackSample(video: HTMLVideoElement | null, hls: Hls | null) {
  const [sample, setSample] = useState({ width: 0, height: 0, dropped: 0, frames: 0, bufferAhead: 0, bandwidth: 0 });
  useEffect(() => {
    if (!video) return;
    const read = () => {
      const quality = video.getVideoPlaybackQuality?.();
      let bufferAhead = 0;
      for (let i = 0; i < video.buffered.length; i++) {
        if (video.buffered.start(i) <= video.currentTime && video.currentTime <= video.buffered.end(i)) {
          bufferAhead = video.buffered.end(i) - video.currentTime;
        }
      }
      setSample({
        width: video.videoWidth,
        height: video.videoHeight,
        dropped: quality?.droppedVideoFrames ?? 0,
        frames: quality?.totalVideoFrames ?? 0,
        bufferAhead,
        bandwidth: hls?.bandwidthEstimate ?? 0,
      });
    };
    read();
    const timer = window.setInterval(read, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [video, hls]);
  return sample;
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div class="statistics-menu-row">
      <span class="statistics-menu-label">{label}</span>
      <span class="statistics-menu-value">{value}</span>
    </div>
  );
}

export function StatisticsMenu({ stats, infoHash, video, hls, animeId, episode, subtitleLabel }: Props) {
  const [copied, setCopied] = useState(false);
  const playback = usePlaybackSample(video, hls);
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  useEffect(() => {
    if (episode == null) return;
    cachedTorrentThumbnail(animeId, episode).then(setThumbnail);
  }, [animeId, episode]);

  async function copyHash() {
    if (!infoHash) return;
    try {
      await navigator.clipboard.writeText(infoHash);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable - nothing more to do.
    }
  }

  const readySeconds = stats.readyRanges.reduce((sum, [start, end]) => sum + (end - start), 0);
  const run = stats.run;

  return (
    <div class="statistics-menu">
      <div class="statistics-menu-title">Statistics</div>
      <div class="statistics-menu-stats">
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Peers</div>
          <div class="statistics-menu-value">{stats.connectedPeers}</div>
        </div>
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Speed</div>
          <div class="statistics-menu-value">{stats.downloadSpeedMbps.toFixed(2)} MB/s</div>
        </div>
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Completed</div>
          <div class="statistics-menu-value">{Math.min(stats.progressPercent, 100).toFixed(0)}%</div>
        </div>
      </div>

      <div class="statistics-menu-section">
        <div class="statistics-menu-heading">Torrent</div>
        <Row label="State" value={stats.state} />
        <Row label="File" value={stats.fileName || "-"} />
        <Row label="Downloaded" value={`${formatBytes(stats.downloadedBytes)} / ${formatBytes(stats.totalBytes)}`} />
        <Row label="Upload" value={`${stats.uploadSpeedMbps.toFixed(2)} MB/s · ${formatBytes(stats.uploadedBytes)} sent`} />
        <Row label="Peers" value={`${stats.unchokedPeers} sending · ${stats.queuedPeers} queued · ${stats.swarmSize} in swarm`} />
        <Row label="Sources" value={String(stats.sources)} />
      </div>

      <div class="statistics-menu-section">
        <div class="statistics-menu-heading">Streaming</div>
        <Row label="Video" value={stats.videoMode ?? "waiting for first segment"} />
        <Row label="Ready to seek" value={`${formatTime(readySeconds)} in ${stats.readyRanges.length} stretch${stats.readyRanges.length === 1 ? "" : "es"}`} />
        {run && (
          <>
            <Row label="Current run" value={`from ${formatTime(run.startSeconds)} · ${run.segmentsProduced} segments${run.running ? "" : " · finished"}`} />
            <Row label="Run speed" value={`${run.speedXRealtime.toFixed(1)}× realtime`} />
            <Row label="Subtitle tracks" value={`${run.subtitleTracks} extracted with video`} />
          </>
        )}
      </div>

      <div class="statistics-menu-section">
        <div class="statistics-menu-heading">Playback</div>
        <Row label="Resolution" value={playback.width ? `${playback.width}×${playback.height}` : "-"} />
        <Row label="Buffered ahead" value={`${playback.bufferAhead.toFixed(1)} s`} />
        <Row label="Dropped frames" value={`${playback.dropped} / ${playback.frames}`} />
        <Row label="Loader bandwidth" value={playback.bandwidth ? `${(playback.bandwidth / 8 / 1024 / 1024).toFixed(1)} MB/s` : "-"} />
        <Row label="Subtitles" value={subtitleLabel ?? "off"} />
      </div>

      {episode != null && (
        <div class="statistics-menu-section">
          <div class="statistics-menu-heading">Thumbnail</div>
          {thumbnail ? (
            <div class="statistics-menu-thumbnail">
              <img src={thumbnail} alt="" />
              <span class="statistics-menu-label">Saved frame - replaced by the last frame when you close the player</span>
            </div>
          ) : (
            <Row label="Source" value="Kitsu / AniList art (no saved frame yet)" />
          )}
        </div>
      )}

      {infoHash && (
        <button class="statistics-menu-hash" onClick={copyHash} title="Copy info hash">
          <span class="statistics-menu-hash-value">{infoHash}</span>
          <span class="statistics-menu-hash-label">{copied ? "Copied" : "Copy"}</span>
        </button>
      )}
    </div>
  );
}
