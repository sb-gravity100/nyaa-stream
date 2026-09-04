import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import Hls from "hls.js";
import type { NyaaResult, StreamStats } from "./types";
import { getStreamStats, playMagnet, stopPlayback } from "./playback";
import { loadingProgress } from "./loadingProgress";
import { bestRelease } from "./releases";
import { Buffering } from "./Buffering";
import { StatisticsMenu } from "./StatisticsMenu";

const STATS_POLL_MS = 1000;
// How long a keybind-triggered flash of the controls stays up before
// auto-hiding again - only applies to that flash, not to hovering: while
// the mouse is actually over the controls (or their hover zone), they stay
// up indefinitely regardless of this.
const KEYBIND_FLASH_MS = 1200;
// Small grace delay before hiding on mouse-leave, so moving from the
// invisible hover zone onto the now-visible control bar (or vice versa -
// two separate, exactly-overlapping elements, see the render below) can't
// flicker shut between the two elements' enter/leave events.
const HOVER_HIDE_GRACE_MS = 150;
const SEEK_STEP_SECONDS = 5;
const SEEK_STEP_SECONDS_LARGE = 10;
const VOLUME_STEP = 5;

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
// with a solid-black bottom control bar that only appears while the mouse
// is actually over it (not on any mouse movement over the video the way
// this used to work), plus PotPlayer/YouTube-style keybinds (space/K
// play-pause, arrows/J/L seek, up/down volume, M mute, F fullscreen, Esc
// close) that work regardless of whether the bar is currently shown.
// mpv/native-window
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
  const [torrentId, setTorrentId] = useState<string | null>(null);
  const [streamUrl, setStreamUrl] = useState<string | null>(null);
  const [stats, setStats] = useState<StreamStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  // Starts hidden, not shown-then-auto-hidden-after-idle like this used to
  // work - hover is now the only thing that reveals it (a keybind press
  // still flashes it briefly regardless, see flashControls).
  const [controlsVisible, setControlsVisible] = useState(false);
  const [paused, setPaused] = useState(true);
  const estimatedDurationSeconds = estimatedDurationMinutes != null ? estimatedDurationMinutes * 60 : null;
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [seekPreview, setSeekPreview] = useState<number | null>(null);
  const [volume, setVolumeState] = useState(100);
  const [muted, setMuted] = useState(false);
  const [statsMenuOpen, setStatsMenuOpen] = useState(false);
  const idleTimerRef = useRef<number | undefined>(undefined);
  const statsPollRef = useRef<number | undefined>(undefined);
  // Whether the pointer is currently over the controls or their hover zone
  // - read by the keybind flash's own hide timer so it doesn't yank the
  // bar away while the mouse is legitimately sitting on it.
  const hoveringControlsRef = useRef(false);
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
      const hls = new Hls({
        // A segment can legitimately take tens of seconds to become
        // available (torrent-engine's HlsJobs waits up to 45s server-side
        // when a seek lands on a not-yet-downloaded region) - hls.js's own
        // default fragment timeout is well under that, so it was giving
        // up and firing a *fatal* networkError ("fragLoadTimeOut") well
        // before our server would actually have delivered the segment -
        // verified live. These are generous enough to cover that server
        // budget with room for retries, not just raised arbitrarily.
        fragLoadingTimeOut: 60_000,
        fragLoadingMaxRetry: 4,
        manifestLoadingTimeOut: 20_000,
      });
      hlsRef.current = hls;
      hls.loadSource(hlsSrc);
      hls.attachMedia(video);
      // Caps hls.js's own recommended fatal-error recovery pattern
      // (below) so a fault that recovery genuinely can't fix (rather than
      // a slow segment that just needed one more retry) doesn't retry
      // silently forever with a permanently-stuck spinner and no
      // indication to the user that anything's wrong.
      let recoveryAttempts = 0;
      const MAX_RECOVERY_ATTEMPTS = 8;
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (!data.fatal) return;
        console.error("[player] hls.js fatal error", { type: data.type, details: data.details, recoveryAttempts });
        // A "fatal" network or media error usually just means hls.js's own
        // retry budget ran out, not that the stream is actually
        // unplayable - restarting the load (or, for a media/decode
        // error, recovering the <video> element) from here typically
        // succeeds rather than needing to give up and show an error.
        if (recoveryAttempts < MAX_RECOVERY_ATTEMPTS) {
          if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
            recoveryAttempts++;
            hls.startLoad();
            return;
          }
          if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
            recoveryAttempts++;
            hls.recoverMediaError();
            return;
          }
        }
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
        const s = await getStreamStats(torrentId as string);
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

  // Controls are purely hover-driven now (not "any mouse movement over the
  // video, then auto-hide after idle" - moving the mouse elsewhere on the
  // video no longer reveals them at all, only entering the bottom strip
  // does), plus a brief flash on keybind use so e.g. a seek/volume keybind
  // is visible without requiring the mouse to be there too.
  function showControls() {
    hoveringControlsRef.current = true;
    window.clearTimeout(idleTimerRef.current);
    setControlsVisible(true);
  }

  function scheduleHideControls() {
    hoveringControlsRef.current = false;
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = window.setTimeout(() => setControlsVisible(false), HOVER_HIDE_GRACE_MS);
  }

  function flashControls() {
    setControlsVisible(true);
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = window.setTimeout(() => {
      if (!hoveringControlsRef.current) setControlsVisible(false);
    }, KEYBIND_FLASH_MS);
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

  function changeVolumeBy(delta: number) {
    const video = videoRef.current;
    if (!video) return;
    const next = Math.min(100, Math.max(0, volume + delta));
    setVolumeState(next);
    video.volume = next / 100;
  }

  // Mute is the <video> element's own separate `.muted` flag, not zeroing
  // the volume slider - matches PotPlayer/every native player: unmuting
  // restores playback at whatever level the slider was already at, rather
  // than the slider itself needing to remember and restore a value.
  function toggleMute() {
    const video = videoRef.current;
    if (!video) return;
    video.muted = !video.muted;
  }

  function seekBy(deltaSeconds: number) {
    const video = videoRef.current;
    if (!video || !duration) return;
    video.currentTime = Math.min(Math.max(video.currentTime + deltaSeconds, 0), duration);
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      videoRef.current?.parentElement?.requestFullscreen();
    }
  }

  // Player-wide keybinds (PotPlayer/YouTube-style): active whenever the
  // player is mounted, i.e. for the whole time it's open. Ignored while
  // focus is on an actual form control (the source-select dropdown, the
  // seek/volume range inputs) so typing/interacting with those doesn't
  // double-fire a keybind too.
  useEffect(() => {
    function isFormControl(target: EventTarget | null): boolean {
      if (!(target instanceof HTMLElement)) return false;
      return target.tagName === "INPUT" || target.tagName === "SELECT" || target.tagName === "TEXTAREA";
    }

    function handleKeyDown(e: KeyboardEvent) {
      if (isFormControl(e.target)) return;
      switch (e.key.toLowerCase()) {
        case " ":
        case "k":
          togglePause();
          break;
        case "arrowleft":
          seekBy(-SEEK_STEP_SECONDS);
          break;
        case "arrowright":
          seekBy(SEEK_STEP_SECONDS);
          break;
        case "j":
          seekBy(-SEEK_STEP_SECONDS_LARGE);
          break;
        case "l":
          seekBy(SEEK_STEP_SECONDS_LARGE);
          break;
        case "arrowup":
          changeVolumeBy(VOLUME_STEP);
          break;
        case "arrowdown":
          changeVolumeBy(-VOLUME_STEP);
          break;
        case "m":
          toggleMute();
          break;
        case "f":
          toggleFullscreen();
          break;
        case "escape":
          // Escape's own browser-native behavior already exits fullscreen
          // first if that's active - only close the player on a second
          // press once there's nothing left for the browser to do with it.
          if (!document.fullscreenElement) onClose();
          return;
        default:
          return;
      }
      // Space/arrows would otherwise scroll the page or (for a focused
      // button) re-trigger a click.
      e.preventDefault();
      flashControls();
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [duration, volume, onClose]);

  const displayPosition = seekPreview ?? position;
  const buffering = !error && !ready;
  // stats.readySeconds (how far HLS segments have actually been produced),
  // not stats.progressPercent (raw torrent byte download) - those two can
  // diverge a lot, e.g. right after a seek forces a jump in download
  // priority a stretch of bytes can be fully downloaded well before ffmpeg
  // has actually processed it into a segment, or vice versa once a
  // stretch is available a fast -c:v copy pass can race ahead of download.
  // Using progressPercent here used to make regions look instantly
  // seekable when they weren't - verified live, see PLAN.md.
  const downloadedPercent = stats && duration ? Math.min(100, (stats.readySeconds / duration) * 100) : 0;
  const playedPercent = duration ? Math.min(100, (displayPosition / duration) * 100) : 0;

  return (
    <div class="player-view">
      {streamUrl && (
        <video
          ref={videoRef}
          class="player-video"
          autoPlay
          onLoadedMetadata={(e) => setDuration((e.target as HTMLVideoElement).duration || 0)}
          onTimeUpdate={(e) => setPosition((e.target as HTMLVideoElement).currentTime)}
          onPlay={() => setPaused(false)}
          onPause={() => setPaused(true)}
          onVolumeChange={(e) => setMuted((e.target as HTMLVideoElement).muted)}
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

      {/* Always-present, invisible strip the same size/position as the
          control bar below - the only thing that reveals the controls now
          (see showControls/scheduleHideControls's doc comment). Needs to
          exist separately from .player-controls itself because that has
          pointer-events:none while hidden and so can never receive the
          hover that would reveal it in the first place. */}
      <div class="player-controls-hover-zone" onMouseEnter={showControls} onMouseLeave={scheduleHideControls} />

      <div
        class={`player-controls${controlsVisible ? " visible" : ""}`}
        onMouseEnter={showControls}
        onMouseLeave={scheduleHideControls}
      >
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
            title="Seek (←/→ 5s, J/L 10s)"
            onInput={handleSeekInput}
            onChange={handleSeekCommit}
            disabled={!duration}
          />
        </div>
        <div class="player-controls-row">
          <button
            class="player-control-button"
            onClick={togglePause}
            aria-label={paused ? "Play" : "Pause"}
            title={`${paused ? "Play" : "Pause"} (Space/K)`}
          >
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
          <button
            class="player-control-button"
            onClick={toggleMute}
            aria-label={muted ? "Unmute" : "Mute"}
            title={`${muted ? "Unmute" : "Mute"} (M)`}
          >
            {muted || volume === 0 ? "🔇" : "🔊"}
          </button>
          <input
            class="player-volume"
            type="range"
            min={0}
            max={100}
            step={1}
            value={volume}
            onInput={handleVolumeInput}
            aria-label="Volume"
            title="Volume (↑/↓)"
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
          <button
            class="player-control-button"
            onClick={toggleFullscreen}
            aria-label="Fullscreen"
            title="Fullscreen (F)"
          >
            ⛶
          </button>
          <button class="player-control-button" onClick={onClose} aria-label="Close player" title="Close (Esc)">
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
