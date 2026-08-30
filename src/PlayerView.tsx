import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import Hls from "hls.js";
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
  /** AniList's typical per-episode runtime, in minutes - sent to the
   * backend as the HLS playlist's declared duration, since it can't
   * reliably determine this itself for a still-downloading torrent (see
   * PLAN.md's Known gaps). */
  estimatedDurationMinutes: number | null;
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

// An HLS stream (via hls.js) pointed at torrent-engine's playlist endpoint,
// with a PotPlayer-style bottom control bar that fades in on mouse movement
// and auto-hides after idle rather than staying on screen. mpv/native-window
// embedding was tried first but hit a Tauri/WebView2 transparency bug on
// Windows that broke click-through app-wide; a single ffmpeg remux per play
// (restarted on every seek) was tried next but leaked processes under rapid
// seeking - HLS replaces both: hls.js requests whichever short segment
// covers a seek target directly, no restarting anything (see PLAN.md's
// Known gaps for the full history).
export function PlayerView({ title, releases, estimatedDurationMinutes, onClose }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  const [selectedRelease, setSelectedRelease] = useState<NyaaResult>(() => bestRelease(releases));
  const [torrentId, setTorrentId] = useState<number | null>(null);
  const [streamUrl, setStreamUrl] = useState<string | null>(null);
  const [stats, setStats] = useState<StreamStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [paused, setPaused] = useState(true);
  const estimatedDurationSeconds = estimatedDurationMinutes != null ? estimatedDurationMinutes * 60 : null;
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
          setStreamUrl(session.hlsUrl);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedRelease, title]);

  // Attaches hls.js to the video element once a playlist URL is available,
  // and tears it down on source switch/unmount. The declared duration is
  // baked into the playlist request itself (see torrent-engine's
  // hls_playlist_handler) - if a better estimate arrives late (e.g.
  // get_anime_details resolving after mount, common from a Library/Latest-
  // Episodes card that starts with a partial AnimeMedia), this re-loads the
  // source with the corrected value rather than being stuck with the first
  // guess for the rest of the session.
  useEffect(() => {
    const video = videoRef.current;
    if (!video || !streamUrl) return;
    const hlsSrc = estimatedDurationSeconds != null ? `${streamUrl}?duration=${estimatedDurationSeconds}` : streamUrl;

    if (Hls.isSupported()) {
      const hls = new Hls();
      hlsRef.current = hls;
      hls.loadSource(hlsSrc);
      hls.attachMedia(video);
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (!data.fatal) return;
        console.error("[player] hls.js fatal error", { type: data.type, details: data.details });
        setError("Playback failed - the file format may not be supported by this browser engine.");
      });
      return () => {
        hls.destroy();
        hlsRef.current = null;
      };
    }

    if (video.canPlayType("application/vnd.apple.mpegurl")) {
      video.src = hlsSrc;
      return undefined;
    }

    setError("This browser engine can't play HLS streams.");
    return undefined;
  }, [streamUrl, estimatedDurationSeconds]);

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
    // hls.js intercepts this and fetches whichever segment covers `value` -
    // no restarting anything, unlike the single-ffmpeg-process approach
    // this replaced.
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
  // Approximation: torrent download progress is tracked as an overall
  // fraction of the file's bytes, not per-region, so this assumes a
  // roughly even bitrate to translate "% of file downloaded" into "% of
  // the timeline downloaded" for the seek bar highlight.
  const downloadedPercent = stats ? Math.min(100, stats.progressPercent) : 0;
  const playedPercent = duration ? Math.min(100, (displayPosition / duration) * 100) : 0;

  return (
    <div class="player-view" onMouseMove={wake} onMouseLeave={() => setControlsVisible(false)}>
      {streamUrl && (
        <video
          ref={videoRef}
          class="player-video"
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
