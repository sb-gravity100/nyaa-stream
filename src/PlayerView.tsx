import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { NyaaResult, StreamStats } from "./types";
import { getStreamStats, playMagnet, stopPlayback } from "./playback";
import { loadingProgress } from "./loadingProgress";
import { bestRelease } from "./releases";
import { Buffering } from "./Buffering";
import { StatisticsMenu } from "./StatisticsMenu";

const STATS_POLL_MS = 1000;
const CONTROLS_IDLE_MS = 2500;

interface Props {
  title: string;
  releases: NyaaResult[];
  onClose: () => void;
}

// One line per source in the picker: seeders/leechers first since that's
// the main thing worth comparing sources on, then size and the release
// title itself (carries quality/group/batch info the parser already
// extracted elsewhere, but raw here since this is the "see everything"
// picker torrentThumbnail.ts's/MediaPage.tsx's auto-pick doesn't offer).
function releaseLabel(release: NyaaResult): string {
  return `${release.seeders}↑ ${release.leechers}↓ · ${release.size} · ${release.title}`;
}

function formatTime(seconds: number): string {
  const clamped = Number.isFinite(seconds) && seconds > 0 ? seconds : 0;
  const total = Math.floor(clamped);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? m.toString().padStart(2, "0") : m.toString();
  const ss = s.toString().padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

function infoHashFromMagnet(magnet: string): string | null {
  return magnet.match(/xt=urn:btih:([a-zA-Z0-9]+)/)?.[1]?.toLowerCase() ?? null;
}

// A plain HTML5 <video> pointed at torrent-engine's local Range-capable
// stream URL, with a PotPlayer-style bottom control bar that fades in on
// mouse movement and auto-hides after idle rather than staying on screen.
// mpv/native-window embedding was tried first but hit a Tauri/WebView2
// transparency bug on Windows that broke click-through app-wide (see
// PLAN.md's Known gaps) - this sidesteps that entirely since the video is
// just page content.
export function PlayerView({ title, releases, onClose }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [selectedRelease, setSelectedRelease] = useState<NyaaResult>(() => bestRelease(releases));
  const [torrentId, setTorrentId] = useState<number | null>(null);
  const [streamUrl, setStreamUrl] = useState<string | null>(null);
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  const [stats, setStats] = useState<StreamStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [paused, setPaused] = useState(true);
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [seekPreview, setSeekPreview] = useState<number | null>(null);
  const [volume, setVolumeState] = useState(100);
  const [statsMenuOpen, setStatsMenuOpen] = useState(false);
  const idleTimerRef = useRef<number | undefined>(undefined);
  const statsPollRef = useRef<number | undefined>(undefined);
  const infoHash = useMemo(() => infoHashFromMagnet(selectedRelease.magnet), [selectedRelease.magnet]);
  const sortedReleases = useMemo(() => [...releases].sort((a, b) => b.seeders - a.seeders), [releases]);

  // Re-runs whenever the user picks a different source from the dropdown,
  // not just on mount - play_magnet's own defensive cleanup on the backend
  // tears down the previous torrent, so switching is just "play again".
  useEffect(() => {
    let cancelled = false;
    setStreamUrl(null);
    setVideoSrc(null);
    setTorrentId(null);
    setStats(null);
    setError(null);
    setReady(false);
    setDuration(0);
    setPosition(0);
    (async () => {
      try {
        const session = await playMagnet(selectedRelease.magnet, title);
        if (!cancelled) {
          setTorrentId(session.torrentId);
          setStreamUrl(session.streamUrl);
          setVideoSrc(session.streamUrl);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedRelease, title]);

  useEffect(() => {
    if (torrentId == null) return;
    async function poll() {
      try {
        const s = await getStreamStats(torrentId as number);
        setStats(s);
        if (s.finished) window.clearInterval(statsPollRef.current);
      } catch {
        window.clearInterval(statsPollRef.current);
      }
    }
    poll();
    statsPollRef.current = window.setInterval(poll, STATS_POLL_MS);
    return () => window.clearInterval(statsPollRef.current);
  }, [torrentId]);

  useEffect(() => {
    return () => {
      window.clearInterval(statsPollRef.current);
      window.clearTimeout(idleTimerRef.current);
      stopPlayback();
    };
  }, []);

  function wake() {
    setControlsVisible(true);
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = window.setTimeout(() => setControlsVisible(false), CONTROLS_IDLE_MS);
  }

  function togglePause() {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) {
      // play() returns a promise that rejects with AbortError if something
      // (e.g. the player closing mid-buffer) pauses the video before it
      // resolves - expected during teardown, not a real playback failure.
      video.play().catch((err) => {
        if (err instanceof DOMException && err.name === "AbortError") return;
        setError(String(err));
      });
    } else {
      video.pause();
    }
  }

  function handleSeekInput(e: Event) {
    setSeekPreview(Number((e.target as HTMLInputElement).value));
  }

  function handleSeekCommit(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    setSeekPreview(null);
    if (!streamUrl) return;
    // A live-piped fragmented MP4 can't be seeked within once bytes have
    // been sent - setting video.currentTime does nothing useful here.
    // Instead, restart the remux at the requested offset (ffmpeg's -ss +
    // -copyts on the backend keeps the new stream's timestamps lined up
    // with the real duration, so the seek bar doesn't reset to 0).
    setPosition(value);
    setReady(false);
    setVideoSrc(`${streamUrl}?start=${value}`);
  }

  function handleVolumeInput(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    setVolumeState(value);
    const video = videoRef.current;
    if (video) video.volume = value / 100;
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      videoRef.current?.parentElement?.requestFullscreen();
    }
  }

  const displayPosition = seekPreview ?? position;
  const buffering = !error && !ready;
  // Approximation: torrent download progress is tracked as an overall
  // fraction of the file's bytes, not per-region, so this assumes a
  // roughly even bitrate to translate "% of file downloaded" into "% of
  // the timeline downloaded" for the seek bar highlight.
  const downloadedPercent = stats ? Math.min(100, stats.progressPercent) : 0;
  const playedPercent = duration ? Math.min(100, (displayPosition / duration) * 100) : 0;

  return (
    <div class="player-view" onMouseMove={wake} onMouseLeave={() => setControlsVisible(false)}>
      {videoSrc && (
        <video
          key={videoSrc}
          ref={videoRef}
          class="player-video"
          src={videoSrc}
          autoPlay
          onLoadedMetadata={(e) => setDuration((e.target as HTMLVideoElement).duration || 0)}
          onTimeUpdate={(e) => setPosition((e.target as HTMLVideoElement).currentTime)}
          onPlay={() => setPaused(false)}
          onPause={() => setPaused(true)}
          onCanPlay={() => setReady(true)}
          onWaiting={() => setReady(false)}
          onPlaying={() => setReady(true)}
          onError={(e) => {
            const video = e.target as HTMLVideoElement;
            const mediaError = video.error;
            // MediaError.code: 1=ABORTED, 2=NETWORK, 3=DECODE, 4=SRC_NOT_SUPPORTED.
            // Logged (not just shown) so it reaches the backend log via
            // devLogger.ts instead of only ever being visible on-screen.
            console.error("[player] video error", {
              code: mediaError?.code,
              message: mediaError?.message,
              src: video.currentSrc,
            });
            setError("Playback failed - the file format may not be supported by this browser engine.");
          }}
        />
      )}

      {error && <div class="player-message player-error">{error}</div>}

      {buffering && !error && <Buffering progress={loadingProgress(stats)} />}

      {statsMenuOpen && stats && (
        <StatisticsMenu
          peers={stats.connectedPeers}
          speedMbps={stats.downloadSpeedMbps}
          completedPercent={stats.progressPercent}
          infoHash={infoHash}
        />
      )}

      <div class={`player-controls${controlsVisible ? " visible" : ""}`}>
        <div class="player-seek-wrap">
          <div class="player-seek-track" />
          <div class="player-seek-downloaded" style={{ width: `${downloadedPercent}%` }} />
          <div class="player-seek-played" style={{ width: `${playedPercent}%` }} />
          <input
            class="player-seek"
            type="range"
            min={0}
            max={duration || 1}
            step={0.1}
            value={displayPosition}
            onInput={handleSeekInput}
            onChange={handleSeekCommit}
            disabled={!duration}
          />
        </div>
        <div class="player-controls-row">
          <button class="player-control-button" onClick={togglePause} aria-label={paused ? "Play" : "Pause"}>
            {paused ? "▶" : "❚❚"}
          </button>
          <div class="player-time">
            {formatTime(displayPosition)} / {formatTime(duration)}
          </div>
          <div class="player-title">{title}</div>
          <div class="player-spacer" />
          {releases.length > 1 && (
            <select
              class="player-source-select"
              value={selectedRelease.magnet}
              onChange={(e) => {
                const magnet = (e.target as HTMLSelectElement).value;
                const next = releases.find((r) => r.magnet === magnet);
                if (next) setSelectedRelease(next);
              }}
              aria-label="Source"
            >
              {sortedReleases.map((r) => (
                <option key={r.magnet} value={r.magnet}>
                  {releaseLabel(r)}
                </option>
              ))}
            </select>
          )}
          <input
            class="player-volume"
            type="range"
            min={0}
            max={100}
            step={1}
            value={volume}
            onInput={handleVolumeInput}
            aria-label="Volume"
          />
          {stats && (
            <button
              class={`player-control-button${statsMenuOpen ? " active" : ""}`}
              onClick={() => setStatsMenuOpen((open) => !open)}
              aria-label="Statistics"
            >
              📊
            </button>
          )}
          <button class="player-control-button" onClick={toggleFullscreen} aria-label="Fullscreen">
            ⛶
          </button>
          <button class="player-control-button" onClick={onClose} aria-label="Close player">
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
