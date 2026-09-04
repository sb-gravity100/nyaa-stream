import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import Hls from "hls.js";
import type { NyaaResult, StreamStats, SubtitleTrack } from "./types";
import { getStreamStats, getSubtitleTracks, playMagnet, stopPlayback } from "./playback";
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
// Keyboard seeking (arrows/J/L) only actually moves the video once this
// long has passed since the last key press, rather than on every single
// press - stremio-web's own useKeyboardSeek hook does the same. Holding or
// rapidly tapping a seek key used to fire a real hls.js seek (buffer
// flush + new segment fetch) on every press, which stutters/"jitters"
// visible playback well before the user's actually done seeking; this
// keeps the seek bar/time display moving smoothly off local state
// (seekPreview) in the meantime and only commits once input settles.
const KEYBOARD_SEEK_COMMIT_MS = 300;
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

function subtitleTrackLabel(track: SubtitleTrack, position: number): string {
  return track.title ?? track.language ?? `Track ${position + 1}`;
}

// Most fansub releases that carry more than one subtitle track use the
// extra one(s) for signs/songs commentary rather than a second language -
// defaulting to the first plain "eng"/"en" track (rather than just the
// first track outright) avoids landing on one of those by chance. Off
// (null) otherwise: nothing in the track list says which one - if any -
// is the "main" dialogue track for a non-English default.
function defaultSubtitleIndex(tracks: SubtitleTrack[]): number | null {
  const english = tracks.find((t) => t.language?.toLowerCase().startsWith("en"));
  return english?.index ?? null;
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
  const [subtitleTracks, setSubtitleTracks] = useState<SubtitleTrack[]>([]);
  // null = subtitles off - also the initial state before tracks have even
  // been fetched, so no <track> is marked "showing" prematurely.
  const [activeSubtitleIndex, setActiveSubtitleIndex] = useState<number | null>(null);
  const idleTimerRef = useRef<number | undefined>(undefined);
  const statsPollRef = useRef<number | undefined>(undefined);
  // Pending arrow/J-L keyboard-seek target, not yet applied to the video -
  // see seekBy's doc comment.
  const keyboardSeekTargetRef = useRef<number | null>(null);
  const keyboardSeekTimerRef = useRef<number | undefined>(undefined);
  const statsMenuRef = useRef<HTMLDivElement>(null);
  const statsButtonRef = useRef<HTMLButtonElement>(null);
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
    setSubtitleTracks([]);
    setActiveSubtitleIndex(null);
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
      window.clearTimeout(keyboardSeekTimerRef.current);
      stopPlayback();
    };
  }, []);

  // Fetched once the torrent's added rather than only once its container
  // header has fully downloaded - the backend just returns an empty list
  // (or a quietly-ignored error, see get_subtitle_tracks's doc comment)
  // until ffprobe can actually read it, so there's nothing to gate this on
  // client-side.
  useEffect(() => {
    if (torrentId == null) return;
    let cancelled = false;
    (async () => {
      const tracks = await getSubtitleTracks(torrentId).catch(() => [] as SubtitleTrack[]);
      if (cancelled) return;
      setSubtitleTracks(tracks);
      setActiveSubtitleIndex(defaultSubtitleIndex(tracks));
    })();
    return () => {
      cancelled = true;
    };
  }, [torrentId]);

  // <track> elements only apply their `default` attribute once, on load -
  // switching the active track later has to be done by hand through the
  // native TextTrackList API instead of re-rendering the `default` prop.
  // `video.textTracks` order matches the DOM order of the <track> elements
  // rendered below, which is the same order as `subtitleTracks` itself, so
  // position (not label - two untitled same-language tracks would collide
  // on that) is what ties a TextTrack back to its SubtitleTrack.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    for (let i = 0; i < video.textTracks.length; i++) {
      const track = subtitleTracks[i];
      video.textTracks[i].mode = track && track.index === activeSubtitleIndex ? "showing" : "disabled";
    }
  }, [subtitleTracks, activeSubtitleIndex]);

  useEffect(() => {
    if (!statsMenuOpen) return;
    function handlePointerDown(event: PointerEvent) {
      const target = event.target as Node;
      if (statsMenuRef.current?.contains(target)) return;
      if (statsButtonRef.current?.contains(target)) return;
      setStatsMenuOpen(false);
    }
    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [statsMenuOpen]);

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

  // Dragging the slider by hand overrides any still-pending keyboard-seek
  // commit (see seekBy) rather than letting that timer fire later and jump
  // the video back to its own stale target.
  function cancelPendingKeyboardSeek() {
    keyboardSeekTargetRef.current = null;
    window.clearTimeout(keyboardSeekTimerRef.current);
  }

  function handleSeekInput(e: Event) {
    cancelPendingKeyboardSeek();
    setSeekPreview(Number((e.target as HTMLInputElement).value));
  }

  function handleSeekCommit(e: Event) {
    cancelPendingKeyboardSeek();
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

  // See KEYBOARD_SEEK_COMMIT_MS's doc comment - accumulates onto any
  // already-pending (not yet committed) target rather than the video's own
  // currentTime, so repeated presses within the debounce window stack
  // correctly instead of each one re-reading a currentTime that hasn't
  // moved yet.
  function seekBy(deltaSeconds: number) {
    const video = videoRef.current;
    if (!video || !duration) return;
    const base = keyboardSeekTargetRef.current ?? video.currentTime;
    const target = Math.min(Math.max(base + deltaSeconds, 0), duration);
    keyboardSeekTargetRef.current = target;
    setSeekPreview(target);
    window.clearTimeout(keyboardSeekTimerRef.current);
    keyboardSeekTimerRef.current = window.setTimeout(() => {
      keyboardSeekTargetRef.current = null;
      setSeekPreview(null);
      video.currentTime = target;
    }, KEYBOARD_SEEK_COMMIT_MS);
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
        >
          {subtitleTracks.map((track, i) => (
            <track
              key={track.index}
              kind="subtitles"
              src={track.url}
              srcLang={track.language ?? undefined}
              label={subtitleTrackLabel(track, i)}
            />
          ))}
        </video>
      )}

      {error && <div class="player-message player-error">{error}</div>}

      {buffering && !error && <Buffering progress={loadingProgress(stats)} />}

      {statsMenuOpen && stats && (
        <div ref={statsMenuRef}>
          <StatisticsMenu
            peers={stats.connectedPeers}
            speedMbps={stats.downloadSpeedMbps}
            completedPercent={stats.progressPercent}
            infoHash={infoHash}
          />
        </div>
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
          {subtitleTracks.length > 0 && (
            <select
              class="player-source-select"
              value={activeSubtitleIndex ?? "off"}
              onChange={(e) => {
                const value = (e.target as HTMLSelectElement).value;
                setActiveSubtitleIndex(value === "off" ? null : Number(value));
              }}
              aria-label="Subtitles"
              title="Subtitles"
            >
              <option value="off">Subtitles: Off</option>
              {subtitleTracks.map((track, i) => (
                <option key={track.index} value={track.index}>
                  {subtitleTrackLabel(track, i)}
                </option>
              ))}
            </select>
          )}
          {stats && (
            <button
              ref={statsButtonRef}
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
