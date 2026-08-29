import { useEffect, useRef, useState } from "preact/hooks";
import type { NyaaResult, StreamStats } from "./types";
import { getStreamStats, playMagnet, stopPlayback } from "./playback";

const STATS_POLL_MS = 1000;
const CONTROLS_IDLE_MS = 2500;

interface Props {
  title: string;
  release: NyaaResult;
  onClose: () => void;
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

// A plain HTML5 <video> pointed at torrent-engine's local Range-capable
// stream URL, with a PotPlayer-style bottom control bar that fades in on
// mouse movement and auto-hides after idle rather than staying on screen.
// mpv/native-window embedding was tried first but hit a Tauri/WebView2
// transparency bug on Windows that broke click-through app-wide (see
// PLAN.md's Known gaps) - this sidesteps that entirely since the video is
// just page content.
export function PlayerView({ title, release, onClose }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [torrentId, setTorrentId] = useState<number | null>(null);
  const [streamUrl, setStreamUrl] = useState<string | null>(null);
  const [stats, setStats] = useState<StreamStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [paused, setPaused] = useState(true);
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [seekPreview, setSeekPreview] = useState<number | null>(null);
  const [volume, setVolumeState] = useState(100);
  const idleTimerRef = useRef<number | undefined>(undefined);
  const statsPollRef = useRef<number | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const session = await playMagnet(release.magnet, title);
        if (!cancelled) {
          setTorrentId(session.torrentId);
          setStreamUrl(session.streamUrl);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
    // release/title are fixed for the lifetime of one PlayerView instance
    // (the parent remounts it via `key` for a different episode).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
    if (video.paused) void video.play();
    else video.pause();
  }

  function handleSeekInput(e: Event) {
    setSeekPreview(Number((e.target as HTMLInputElement).value));
  }

  function handleSeekCommit(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    setSeekPreview(null);
    const video = videoRef.current;
    if (video) video.currentTime = value;
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

  return (
    <div class="player-view" onMouseMove={wake} onMouseLeave={() => setControlsVisible(false)}>
      {streamUrl && (
        <video
          ref={videoRef}
          class="player-video"
          src={streamUrl}
          autoPlay
          onLoadedMetadata={(e) => setDuration((e.target as HTMLVideoElement).duration || 0)}
          onTimeUpdate={(e) => setPosition((e.target as HTMLVideoElement).currentTime)}
          onPlay={() => setPaused(false)}
          onPause={() => setPaused(true)}
          onCanPlay={() => setReady(true)}
          onWaiting={() => setReady(false)}
          onPlaying={() => setReady(true)}
          onError={() => setError("Playback failed - the file format may not be supported by this browser engine.")}
        />
      )}

      {error && <div class="player-message player-error">{error}</div>}

      {buffering && (
        <div class="player-message">
          {stats ? (
            <>
              Buffering… {stats.progressPercent.toFixed(0)}% ∙ {stats.downloadSpeedMbps.toFixed(2)} MiB/s ∙{" "}
              {stats.connectedPeers} peers
            </>
          ) : (
            "Adding torrent…"
          )}
        </div>
      )}

      <div class={`player-controls${controlsVisible ? " visible" : ""}`}>
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
        <div class="player-controls-row">
          <button class="player-control-button" onClick={togglePause} aria-label={paused ? "Play" : "Pause"}>
            {paused ? "▶" : "❚❚"}
          </button>
          <div class="player-time">
            {formatTime(displayPosition)} / {formatTime(duration)}
          </div>
          <div class="player-title">{title}</div>
          <div class="player-spacer" />
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
