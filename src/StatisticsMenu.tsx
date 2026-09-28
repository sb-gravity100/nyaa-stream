import { useEffect, useState } from "preact/hooks";
import type { MpvVideo } from "./mpvVideo";
import type { StreamStats } from "./types";
import { cachedTorrentThumbnail } from "./torrentThumbnail";

// Grew out of stremio-web's Player/StatisticsMenu (peers/speed/completed +
// info hash): now the full picture - torrent swarm, what mpv is decoding,
// and the episode's thumbnail.
interface Props {
  stats: StreamStats;
  infoHash: string | null;
  video: MpvVideo | null;
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

const EMPTY_SAMPLE = { width: 0, height: 0, dropped: 0, decoderDropped: 0, bufferAhead: 0, cacheSpeed: 0, videoCodec: "", audioCodec: "", hwdec: "" };

/** mpv's own playback figures, sampled every second. */
function usePlaybackSample(video: MpvVideo | null) {
  const [sample, setSample] = useState(EMPTY_SAMPLE);
  useEffect(() => {
    if (!video) return;
    let cancelled = false;
    const get = (name: string) => video.getProperty(name).catch(() => null);
    const read = async () => {
      const [dropped, decoderDropped, cache, videoCodec, audioCodec, hwdec] = await Promise.all([
        get("frame-drop-count"),
        get("decoder-frame-drop-count"),
        get("demuxer-cache-state"),
        get("video-codec"),
        get("audio-codec-name"),
        get("hwdec-current"),
      ]);
      if (cancelled) return;
      const cacheState = cache as { "cache-end"?: number; "raw-input-rate"?: number } | null;
      setSample({
        width: video.videoWidth,
        height: video.videoHeight,
        dropped: Number(dropped ?? 0),
        decoderDropped: Number(decoderDropped ?? 0),
        bufferAhead: Math.max(0, (cacheState?.["cache-end"] ?? 0) - video.currentTime),
        cacheSpeed: cacheState?.["raw-input-rate"] ?? 0,
        videoCodec: String(videoCodec ?? ""),
        audioCodec: String(audioCodec ?? ""),
        hwdec: String(hwdec ?? ""),
      });
    };
    void read();
    const timer = window.setInterval(read, REFRESH_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [video]);
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

export function StatisticsMenu({ stats, infoHash, video, animeId, episode, subtitleLabel }: Props) {
  const [copied, setCopied] = useState(false);
  const playback = usePlaybackSample(video);
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
        <div class="statistics-menu-heading">Playback</div>
        <Row label="Resolution" value={playback.width ? `${playback.width}×${playback.height}` : "-"} />
        <Row label="Video" value={playback.videoCodec ? `${playback.videoCodec}${playback.hwdec && playback.hwdec !== "no" ? ` · ${playback.hwdec}` : " · software"}` : "-"} />
        <Row label="Audio" value={playback.audioCodec || "-"} />
        <Row label="Buffered ahead" value={`${playback.bufferAhead.toFixed(1)} s`} />
        <Row label="Stream read" value={playback.cacheSpeed ? `${(playback.cacheSpeed / 1024 / 1024).toFixed(1)} MB/s` : "-"} />
        <Row label="Dropped frames" value={`${playback.dropped} output · ${playback.decoderDropped} decoder`} />
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
