import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";
import Hls from "hls.js";
import { invoke } from "@tauri-apps/api/core";
import { isTypingTarget } from "./keyboard";
import { isFullscreen, setFullscreen, toggleFullscreen, useFullscreen } from "./fullscreen";
import { isPip, setPip, togglePip, usePip } from "./pip";
import type { AnimeMedia, NyaaResult, PlayFile, StreamStats, SubtitleTrack } from "./types";
import { displayTitle, isMovie } from "./types";
import { getStreamStats, getSubtitleTracks, playMagnet, reportDecoderSupport, stopPlayback } from "./playback";
import { saveFrameThumbnail } from "./torrentThumbnail";
import { loadingProgress } from "./loadingProgress";
import { bestRelease, codecPlayable, getPreferredGroup, releaseCodec, releaseGroup, releaseResolution, setPreferredGroup, sortReleases } from "./releases";
import { parseEpisode } from "./episodeParser";
import { Buffering } from "./Buffering";
import { StatisticsMenu } from "./StatisticsMenu";
import { getSettings as getSettingsSnapshot, useSettings } from "./settings";
import { defaultSubtitleIndex, isStyledTrack, subtitleTrackLabel } from "./subtitles";
import { useAssRenderer } from "./assRenderer";
import { resumePosition, saveProgress } from "./watchProgress";
import { PlayerPlaylist, type PlaylistItem } from "./PlayerPlaylist";
import {
  BackIcon,
  EpisodesIcon,
  ExitFullscreenIcon,
  FullscreenIcon,
  PipIcon,
  SkipBackIcon,
  SkipForwardIcon,
  MuteIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  SourcesIcon,
  StatsIcon,
  SubtitlesIcon,
  VolumeIcon,
} from "./icons";

const STATS_POLL_MS = 1000;
// How long a keybind-triggered flash of the controls stays up before
// auto-hiding again.
const KEYBIND_FLASH_MS = 1200;
// Mouse idle this long anywhere but the top/bottom control areas fades
// the controls and hides the cursor. While the pointer rests over those
// areas the controls stay up indefinitely.
const CONTROLS_IDLE_MS = 2000;
// What counts as "over the controls": the bars themselves and the
// always-present invisible strips under them (the bars have
// pointer-events:none while hidden, so the strips are hit instead).
const CONTROLS_AREA_SELECTOR =
   ".player-controls, .player-topbar, .player-controls-hover-zone";
const SEEK_STEP_SECONDS = 5;
const SEEK_STEP_SECONDS_LARGE = 10;
// Keyboard seeking (arrows/J/L) only actually moves the video once this
// long has passed since the last key press (stremio-web's useKeyboardSeek
// does the same): each real hls.js seek flushes the buffer and fetches a
// new segment, so committing on every press stutters playback.
const KEYBOARD_SEEK_COMMIT_MS = 300;
const VOLUME_STEP = 5;
// The track list needs the container header, which may not have
// downloaded yet when the player mounts - retried until it has.
const SUBTITLE_PROBE_RETRY_MS = 3000;
const SUBTITLE_PROBE_MAX_ATTEMPTS = 40;
const SUBTITLE_DELAY_STEP = 0.1;
const PROGRESS_SAVE_MS = 5000;
// Countdown shown before auto-starting the next episode.
const AUTOPLAY_NEXT_SECONDS = 8;
const VOLUME_STORAGE_KEY = "nyaa-stream:volume";
const REMAINING_STORAGE_KEY = "nyaa-stream:show-remaining";

/** Thumbnail width for the last-frame capture (16:9 cards). */
const LAST_FRAME_WIDTH = 640;

/** Copies the frame on screen as the player closes and saves it as the
 * episode's thumbnail (see `saveFrameThumbnail`). The draw must run before
 * playback stops, while the <video> still holds the frame; JPEG encoding
 * then happens off the unmount path via toBlob. MSE media fetched with
 * CORS doesn't taint the canvas; any failure just skips the thumbnail. */
function captureLastFrame({ video, animeId, episode, hasPlayed }: { video: HTMLVideoElement | null; animeId: number; episode: number | null; hasPlayed: boolean }) {
   if (!video || episode == null || !hasPlayed || video.readyState < 2 || !video.videoWidth) {
      console.debug("[player] last-frame capture skipped", {
         animeId,
         episode,
         hasVideo: video != null,
         hasPlayed,
         readyState: video?.readyState,
         videoWidth: video?.videoWidth,
      });
      return;
   }
   try {
      const canvas = document.createElement("canvas");
      canvas.width = LAST_FRAME_WIDTH;
      canvas.height = Math.round((LAST_FRAME_WIDTH * video.videoHeight) / video.videoWidth);
      canvas.getContext("2d")?.drawImage(video, 0, 0, canvas.width, canvas.height);
      canvas.toBlob(
         (blob) => {
            if (!blob) {
               console.warn("[player] last-frame encode produced nothing", { animeId, episode });
               return;
            }
            console.info("[player] captured last frame", { animeId, episode, bytes: blob.size });
            void saveFrameThumbnail(animeId, episode, blob);
         },
         "image/jpeg",
         0.82,
      );
   } catch (err) {
      console.warn("[player] last-frame capture failed", { animeId, episode, err: String(err) });
   }
}

interface Props {
   anime: AnimeMedia;
   /** MediaPage group key ("Episode 5", "Batch", ...) - also the watch
    * progress key. */
   episodeKey: string;
   /** Episode number when the group is a numbered episode; used to find the
    * right file inside a batch torrent. */
   episode: number | null;
   releases: NyaaResult[];
   onClose: () => void;
   /** Present when there's a next episode to go to. */
   onNext: (() => void) | null;
   nextLabel: string | null;
   /** The anime's episode groups, for the side playlist. */
   playlist: PlaylistItem[];
   onSelectEpisode: (key: string) => void;
}

type Menu = "subtitles" | "sources" | "stats" | null;

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
   return (
      magnet.match(/xt=urn:btih:([a-zA-Z0-9]+)/)?.[1]?.toLowerCase() ?? null
   );
}

function baseName(path: string): string {
   return path.split(/[\\/]/).pop() ?? path;
}

/** The file inside the torrent to play: the one whose name parses to the
 * requested episode (batches), else the backend's default (largest video). */
function pickFile(
   files: PlayFile[],
   defaultIdx: number,
   episode: number | null,
): PlayFile | null {
   const videos = files.filter((f) => f.isVideo);
   if (videos.length > 1 && episode != null) {
      const matches = videos.filter((f) => {
         const label = parseEpisode(baseName(f.name));
         return label.kind === "episode" && label.number === episode;
      });
      if (matches.length > 0) {
         const pick = matches.reduce((a, b) => (b.length > a.length ? b : a));
         console.info("[player] matched episode file in batch", {
            episode,
            file: pick.name,
         });
         return pick;
      }
      console.warn(
         "[player] no file in torrent matched episode, using default",
         { episode },
      );
   }
   // A batch opened as a whole (no episode number): start from its first
   // file by name rather than the largest, which is an arbitrary episode.
   if (videos.length > 1 && episode == null) {
      return [...videos].sort((x, y) =>
         x.name.localeCompare(y.name, undefined, { numeric: true }),
      )[0];
   }
   return (
      files.find((f) => f.index === defaultIdx) ?? videos[0] ?? files[0] ?? null
   );
}

function fileLabel(file: PlayFile): string {
   const label = parseEpisode(baseName(file.name));
   const prefix = label.kind === "episode" ? `Ep ${label.number} · ` : "";
   return `${prefix}${baseName(file.name)}`;
}

function loadVolume(): number {
   try {
      const raw = Number(localStorage.getItem(VOLUME_STORAGE_KEY));
      return Number.isFinite(raw) && raw > 0 && raw <= 100 ? raw : 100;
   } catch {
      return 100;
   }
}

// Fallback player for when mpv isn't installed (see PlayerView.tsx, the
// mpv-backed default): full-viewport HLS via hls.js against
// torrent-engine's playlist, transcoding what the WebView can't decode and
// rendering subtitles with JASSUB. The control bar only appears while the
// pointer is over the bottom strip (or briefly after a keybind), plus
// PotPlayer/YouTube-style keybinds.
export function HlsPlayerView({
   anime,
   episodeKey,
   episode,
   releases,
   onClose,
   onNext,
   nextLabel,
   playlist,
   onSelectEpisode,
}: Props) {
   const settings = useSettings();
   const videoRef = useRef<HTMLVideoElement | null>(null);
   const rootRef = useRef<HTMLDivElement>(null);
   // Media position to resume from when hls.js is re-attached to the same
   // file (see the attach effect).
   const reattachRef = useRef<{ url: string; time: number } | null>(null);
   const [videoEl, setVideoEl] = useState<HTMLVideoElement | null>(null);
   const preferredGroup = settings.rememberFansubGroup
      ? getPreferredGroup(anime.id)
      : null;
   const releasePrefs = {
      preferredGroup,
      preferredFansubber: settings.preferredFansubber,
      preferredResolution: settings.preferredResolution,
   };
   const [selectedRelease, setSelectedRelease] = useState<NyaaResult>(() =>
      bestRelease(releases, releasePrefs),
   );
   const [torrentId, setTorrentId] = useState<string | null>(null);
   const [files, setFiles] = useState<PlayFile[]>([]);
   const [selectedFile, setSelectedFile] = useState<PlayFile | null>(null);
   const [stats, setStats] = useState<StreamStats | null>(null);
   const [error, setError] = useState<string | null>(null);
   const [status, setStatus] = useState("Connecting to peers…");
   const [ready, setReady] = useState(false);
   // Whether the current file has shown its first frame - the <video>
   // fades in on it instead of popping over the loading state.
   const [hasPlayed, setHasPlayed] = useState(false);
   const [controlsVisible, setControlsVisible] = useState(false);
   const [cursorHidden, setCursorHidden] = useState(false);
   const [paused, setPaused] = useState(true);
   const estimatedDurationSeconds =
      anime.duration != null ? anime.duration * 60 : null;
   const [duration, setDuration] = useState(0);
   // Media time as the <video> element reports it.
   const [position, setPosition] = useState(0);
   const [seekPreview, setSeekPreview] = useState<number | null>(null);
   // Seek-bar hover tooltip, positioned straight on the DOM: routing each
   // mousemove through React state re-rendered the whole player per event,
   // so the tooltip trailed behind the cursor.
   const seekTooltipRef = useRef<HTMLDivElement>(null);
   const [volume, setVolumeState] = useState(loadVolume);
   const [muted, setMuted] = useState(false);
   const [menu, setMenu] = useState<Menu>(null);
   const [playlistOpen, setPlaylistOpen] = useState(false);
   // Stable so the memoized PlayerPlaylist skips the ~4Hz re-renders
   // that timeupdate drives through this component.
   const selectFromPlaylist = useCallback(
      (key: string) => {
         console.info("[player] episode picked from list", { key });
         setPlaylistOpen(false);
         onSelectEpisode(key);
      },
      [onSelectEpisode],
   );
   const closePlaylist = useCallback(() => setPlaylistOpen(false), []);
   const fullscreen = useFullscreen();
   const pip = usePip();
   // Leaving the player never leaves the window stuck as a mini window.
   useEffect(() => () => void setPip(false), []);
   // What the <video> element itself has buffered, in episode time - drawn
   // on the seek bar above the transcode's ready ranges.
   const [bufferedRanges, setBufferedRanges] = useState<[number, number][]>([]);
   const [showRemaining, setShowRemaining] = useState(() => {
      try {
         return localStorage.getItem(REMAINING_STORAGE_KEY) === "1";
      } catch {
         return false;
      }
   });
   const [subtitleTracks, setSubtitleTracks] = useState<SubtitleTrack[]>([]);
   const [subtitleFonts, setSubtitleFonts] = useState<string[]>([]);
   // null = subtitles off.
   const [activeSubtitleIndex, setActiveSubtitleIndex] = useState<
      number | null
   >(null);
   // Source-file seconds at media time 0 (hls.js initPTS) - see the
   // INIT_PTS_FOUND handler. Media time + this = real episode time, which
   // is what's displayed, saved as progress, and what subtitles use.
   const [timeOffset, setTimeOffset] = useState(0);
   const [subtitleDelay, setSubtitleDelay] = useState(0);
   const [toast, setToast] = useState<string | null>(null);
   const [nextCountdown, setNextCountdown] = useState<number | null>(null);
   const idleTimerRef = useRef<number | undefined>(undefined);
   const toastTimerRef = useRef<number | undefined>(undefined);
   const keyboardSeekTargetRef = useRef<number | null>(null);
   const keyboardSeekTimerRef = useRef<number | undefined>(undefined);
   const hoveringControlsRef = useRef(false);
   const menuRef = useRef<HTMLDivElement>(null);
   // For the idle timer, which outlives the render that scheduled it.
   const menuOpenRef = useRef(false);
   menuOpenRef.current = menu != null;
   const timeOffsetRef = useRef(0);
   timeOffsetRef.current = timeOffset;
   // Resume target in source time, captured once per episode.
   const resumeAtRef = useRef<number | null>(
      settings.resumePlayback ? resumePosition(anime.id, episodeKey) : null,
   );
   const selectedReleaseRef = useRef(selectedRelease);
   selectedReleaseRef.current = selectedRelease;
   const infoHash = useMemo(
      () => infoHashFromMagnet(selectedRelease.magnet),
      [selectedRelease.magnet],
   );
   const sortedReleases = useMemo(
      () => sortReleases(releases, releasePrefs),
      [releases],
   );
   const videoFiles = files.filter((f) => f.isVideo);

   function showToast(message: string) {
      setToast(message);
      window.clearTimeout(toastTimerRef.current);
      toastTimerRef.current = window.setTimeout(() => setToast(null), 1400);
   }

   // Re-runs whenever the user picks a different source: play_magnet tears
   // down the previous torrent, so switching is just "play again".
   useEffect(() => {
      let cancelled = false;
      setTorrentId(null);
      setFiles([]);
      setSelectedFile(null);
      setStats(null);
      setError(null);
      setReady(false);
      setStatus("Connecting to peers…");
      (async () => {
         try {
            await reportDecoderSupport().catch((err) =>
               console.warn("[player] decoder support report failed", {
                  err: String(err),
               }),
            );
            const session = await playMagnet(
               selectedRelease.magnet,
               `${displayTitle(anime.title)} ${episodeKey}`,
            );
            if (cancelled) return;
            // A movie is the torrent's largest video (the backend's
            // default) - never an extra that happens to parse as "01".
            const file = isMovie(anime)
               ? (session.files.find((f) => f.index === session.defaultFileIdx) ?? null)
               : pickFile(session.files, session.defaultFileIdx, episode);
            if (!file) {
               setError("This torrent has no playable video file.");
               return;
            }
            setTorrentId(session.torrentId);
            setFiles(session.files);
            setSelectedFile(file);
            setStatus("Preparing stream…");
         } catch (err) {
            if (!cancelled)
               setError(
                  `Couldn't start this source: ${err instanceof Error ? err.message : String(err)}`,
               );
         }
      })();
      return () => {
         cancelled = true;
      };
   }, [selectedRelease, anime.id, episodeKey, episode]);

   // Per-file state resets (source switch or batch file switch).
   useEffect(() => {
      setDuration(0);
      setPosition(0);
      setTimeOffset(0);
      setHasPlayed(false);
      setSubtitleTracks([]);
      setSubtitleFonts([]);
      setActiveSubtitleIndex(null);
      setSubtitleDelay(0);
      setNextCountdown(null);
   }, [selectedFile?.hlsUrl]);

   // Latest values for the unmount capture below.
   const lastFrameRef = useRef({ video: null as HTMLVideoElement | null, animeId: anime.id, episode, hasPlayed: false });
   lastFrameRef.current = { video: videoEl, animeId: anime.id, episode, hasPlayed };

   // Must be declared before the hls.js effect: Preact runs unmount
   // cleanups in declaration order, and hls.destroy() empties the <video>
   // (readyState 0), which made every capture silently bail.
   useEffect(() => () => captureLastFrame(lastFrameRef.current), []);

   // Attaches hls.js once a file is chosen. The declared duration is baked
   // into the playlist request (see torrent-engine's hls_playlist_handler).
   useEffect(() => {
      const video = videoEl;
      if (!video || !selectedFile) return;
      // Re-attaching the same file (only the duration estimate changed -
      // e.g. anime details arriving after playback began) continues from the
      // current position. It used to restart from 0, which silently undid a
      // seek made before the details landed (verified live).
      const reattach =
         reattachRef.current?.url === selectedFile.hlsUrl
            ? reattachRef.current
            : null;
      reattachRef.current = null;
      const startAt = reattach ? reattach.time : resumeAtRef.current;
      // `start` makes the backend begin its run for the fMP4 init segment at
      // the resume point rather than at 0 (see hls_playlist_handler).
      const params = new URLSearchParams();
      if (estimatedDurationSeconds != null)
         params.set("duration", String(estimatedDurationSeconds));
      if (startAt != null && startAt > 0) params.set("start", String(startAt));
      const query = params.toString();
      const hlsSrc = query
         ? `${selectedFile.hlsUrl}?${query}`
         : selectedFile.hlsUrl;
      console.info("[player] attaching hls", {
         file: selectedFile.name,
         startAt,
         reattach: reattach != null,
      });

      if (!Hls.isSupported()) {
         setError("This browser engine can't play HLS streams.");
         return undefined;
      }
      const hls = new Hls({
         // A segment can legitimately take tens of seconds to become
         // available (torrent-engine waits up to 45s server-side on a
         // not-yet-downloaded region) - hls.js's defaults gave up well before
         // that and fired fatal fragLoadTimeOut errors.
         fragLoadingTimeOut: 60_000,
         fragLoadingMaxRetry: 4,
         manifestLoadingTimeOut: 20_000,
         // Buffer ahead generously but stay well under what the backend
         // would treat as a seek (see torrent-engine's job_will_reach_soon).
         maxBufferLength: 60,
         maxMaxBufferLength: 90,
         backBufferLength: 90,
         // Nudge over the small gaps a keyframe-aligned seek restart can
         // leave between transcode runs instead of stalling on them.
         maxBufferHole: 1,
         nudgeMaxRetry: 10,
         // Resume: start loading at the saved point instead of 0. This is
         // media time; the offset correction below is at most one keyframe
         // interval, well inside what "resume" needs.
         startPosition: startAt ?? -1,
      });
      hls.loadSource(hlsSrc);
      hls.attachMedia(video);
      // hls.js anchors media time 0 to whichever fragment it loads first.
      // With -copyts that's source time 0 for a normal start, but a resume
      // anchors on a keyframe-aligned fragment - initPTS is exactly that
      // anchor, and without it subtitles and the time display would be off
      // by a constant (verified: 2s on a 5s-GOP test file).
      hls.on(Hls.Events.INIT_PTS_FOUND, (_event, data) => {
         if (data.id !== "main") return;
         // Segments were MPEG-TS until the fMP4 switch, where initPTS was a
         // raw 33-bit timestamp: a first DTS just below zero (B-frames) wrapped
         // to ~2^33 - verified live: an offset of 95443s made progress save as
         // "watched" and libass draw subtitles 26 hours ahead. fMP4 timestamps
         // don't wrap; the unwrap below is kept as a harmless guard.
         let offset = data.initPTS / data.timescale;
         const wrap = 2 ** 33 / 90_000;
         if (offset > wrap / 2) offset -= wrap;
         console.info("[player] hls initPTS found", { offset });
         setTimeOffset(offset);
      });
      let recoveryAttempts = 0;
      const MAX_RECOVERY_ATTEMPTS = 8;
      hls.on(Hls.Events.ERROR, (_event, data) => {
         if (!data.fatal) return;
         console.error("[player] hls.js fatal error", {
            type: data.type,
            details: data.details,
            recoveryAttempts,
         });
         // hls.js's own recommended recovery: a "fatal" error usually just
         // means its retry budget ran out on a slow segment.
         // Unsupported codec (typically HEVC without the system extension):
         // retrying can never succeed, so say so plainly right away.
         if (
            data.details === Hls.ErrorDetails.BUFFER_ADD_CODEC_ERROR ||
            data.details === Hls.ErrorDetails.BUFFER_INCOMPATIBLE_CODECS_ERROR
         ) {
            const codec = releaseCodec(selectedReleaseRef.current);
            setError(
               `This system can't decode ${codec ? codec.toUpperCase() : "this source's"} video. Pick an H.264 (x264/AVC) source instead.`,
            );
            return;
         }
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
         setError(
            "Playback stopped: this source isn't downloading or can't be decoded. Try another source.",
         );
      });
      const attachedUrl = selectedFile.hlsUrl;
      return () => {
         // Remember where playback was in case this is a same-file re-attach.
         if (video.currentTime > 0)
            reattachRef.current = { url: attachedUrl, time: video.currentTime };
         hls.destroy();
      };
   }, [videoEl, selectedFile?.hlsUrl, estimatedDurationSeconds]);

   useEffect(() => {
      if (torrentId == null || !selectedFile) return;
      const fileIdx = selectedFile.index;
      let timer: number | undefined;
      async function poll() {
         try {
            const s = await getStreamStats(torrentId as string, fileIdx);
            setStats(s);
            if (s.finished) window.clearInterval(timer);
         } catch {
            window.clearInterval(timer);
         }
      }
      void poll();
      timer = window.setInterval(poll, STATS_POLL_MS);
      return () => window.clearInterval(timer);
   }, [torrentId, selectedFile?.index]);

   useEffect(() => {
      return () => {
         window.clearTimeout(idleTimerRef.current);
         window.clearTimeout(keyboardSeekTimerRef.current);
         window.clearTimeout(toastTimerRef.current);
         stopPlayback();
      };
   }, []);

   // Retried until the backend's probe succeeds: right after play_magnet
   // the container header usually hasn't downloaded yet.
   useEffect(() => {
      if (torrentId == null || !selectedFile) return;
      const fileIdx = selectedFile.index;
      let cancelled = false;
      let timer: number | undefined;
      let attempts = 0;
      async function attempt() {
         attempts++;
         try {
            const info = await getSubtitleTracks(torrentId as string, fileIdx);
            if (cancelled) return;
            console.info("[player] subtitle tracks", {
               count: info.tracks.length,
               fonts: info.fonts.length,
            });
            setSubtitleTracks(info.tracks);
            setSubtitleFonts(info.fonts);
            const current = getSettingsSnapshot();
            setActiveSubtitleIndex(
               defaultSubtitleIndex(
                  info.tracks,
                  current.subtitleLanguage,
                  current.subtitlesEnabled,
               ),
            );
         } catch (err) {
            if (cancelled) return;
            if (attempts < SUBTITLE_PROBE_MAX_ATTEMPTS) {
               console.debug("[player] subtitle probe not ready, retrying", {
                  attempts,
               });
               timer = window.setTimeout(attempt, SUBTITLE_PROBE_RETRY_MS);
            } else {
               console.warn("[player] giving up on subtitle probe", {
                  err: String(err),
               });
            }
         }
      }
      void attempt();
      return () => {
         cancelled = true;
         window.clearTimeout(timer);
      };
   }, [torrentId, selectedFile?.index]);

   const activeSubtitle =
      subtitleTracks.find((t) => t.index === activeSubtitleIndex) ?? null;

   // How much of the video's bottom edge the control dock covers while it's
   // up - handed to the renderer, which lifts only the bottom-aligned lines
   // that would sit under it. Measured from layout offsets (not bounding
   // rects) so the dock's own slide-in transform doesn't skew it.
   const DOCK_CLEARANCE_PX = 14;
   const [subtitleInset, setSubtitleInset] = useState(0);
   useEffect(() => {
      if (!(controlsVisible || menu)) {
         setSubtitleInset(0);
         return;
      }
      const root = rootRef.current;
      const dock = root?.querySelector<HTMLElement>(".player-controls");
      const video = videoRef.current;
      if (!root || !dock || !video || !video.videoWidth) return;
      // The visible picture inside the <video> box (letterboxing excluded).
      const box = video.getBoundingClientRect();
      const scale = Math.min(
         box.width / video.videoWidth,
         box.height / video.videoHeight,
      );
      const pictureBottom =
         box.top -
         root.getBoundingClientRect().top +
         (box.height + video.videoHeight * scale) / 2;
      setSubtitleInset(
         // From the seek bar, not the dock's top edge: the dock starts
         // with a tall transparent gradient that doesn't cover anything.
         Math.max(0, pictureBottom - (dock.offsetTop + (dock.querySelector<HTMLElement>(".player-seek-wrap")?.offsetTop ?? 0)) + DOCK_CLEARANCE_PX),
      );
   }, [controlsVisible, menu, duration]);
   useAssRenderer({
      video: videoEl,
      url: activeSubtitle?.url ?? null,
      fonts: subtitleFonts,
      styled: activeSubtitle ? isStyledTrack(activeSubtitle) : false,
      style: settings.subtitleStyle,
      bottomInsetPx: subtitleInset,
      timeOffset,
      delay: subtitleDelay,
   });

   // Episode time (source-file seconds) - what the user sees and what's
   // saved, independent of where hls.js anchored media time 0.
   const episodeTime = position + timeOffset;
   // The playlist's declared length is itself an estimate (AniList
   // runtime), so it's used as the episode length as-is.
   const episodeDuration = duration;

   function persistProgress() {
      const video = videoRef.current;
      // readyState check instead of the `ready` state: this runs from an
      // interval whose closure would otherwise see a stale value.
      if (!video || !video.duration || video.readyState < 1) return;
      saveProgress(
         anime,
         episodeKey,
         episode,
         video.currentTime + timeOffsetRef.current,
         video.duration,
      );
   }

   useEffect(() => {
      if (paused) {
         persistProgress();
         return;
      }
      const timer = window.setInterval(persistProgress, PROGRESS_SAVE_MS);
      return () => {
         window.clearInterval(timer);
         persistProgress();
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, [paused, duration]);

   // Remembers the fansub group once its release actually plays, not when
   // merely picked. Done from the `playing` event rather than an effect on
   // `ready`: that effect fired on a source switch while `ready` was still
   // true from the previous release, saving a group that never played
   // (verified live).
   function rememberGroupOnPlay() {
      if (!getSettingsSnapshot().rememberFansubGroup) return;
      const group = releaseGroup(selectedReleaseRef.current);
      if (group && group !== getPreferredGroup(anime.id))
         setPreferredGroup(anime.id, group);
   }

   // Next-episode countdown.
   useEffect(() => {
      if (nextCountdown == null) return;
      if (nextCountdown <= 0) {
         onNext?.();
         return;
      }
      const timer = window.setTimeout(
         () => setNextCountdown((n) => (n == null ? null : n - 1)),
         1000,
      );
      return () => window.clearTimeout(timer);
   }, [nextCountdown]);

   function handleEnded() {
      persistProgress();
      if (onNext && settings.autoplayNext)
         setNextCountdown(AUTOPLAY_NEXT_SECONDS);
   }

   useEffect(() => {
      const root = rootRef.current;
      if (!root) return;

      // Re-checked on every move rather than via enter/leave events, which
      // miss the pointer when the bars appear or vanish underneath it.
      function handlePointerMove(event: PointerEvent) {
         if (event.pointerType !== "mouse") return;
         const target = event.target;
         const over =
            target instanceof Element &&
            target.closest(CONTROLS_AREA_SELECTOR) != null;
         if (over !== hoveringControlsRef.current)
            console.debug("[player] pointer over controls area", { over });
         hoveringControlsRef.current = over;
         showControls();
      }

      function handlePointerLeave() {
         console.debug("[player] pointer left player");
         hoveringControlsRef.current = false;
         window.clearTimeout(idleTimerRef.current);
         hideControlsIfIdle();
      }

      root.addEventListener("pointermove", handlePointerMove);
      root.addEventListener("pointerleave", handlePointerLeave);
      return () => {
         root.removeEventListener("pointermove", handlePointerMove);
         root.removeEventListener("pointerleave", handlePointerLeave);
      };
   }, []);

   useEffect(() => {
      if (!menu) return;
      function handlePointerDown(event: PointerEvent) {
         const target = event.target as HTMLElement;
         if (menuRef.current?.contains(target)) return;
         if (target.closest("[data-menu-toggle]")) return;
         setMenu(null);
      }
      document.addEventListener("pointerdown", handlePointerDown);
      return () =>
         document.removeEventListener("pointerdown", handlePointerDown);
   }, [menu]);

   useEffect(() => {
      if (!playlistOpen) return;
      function handlePointerDown(event: PointerEvent) {
         const target = event.target as HTMLElement;
         if (
            target.closest(".player-playlist") ||
            target.closest("[data-playlist-toggle]")
         )
            return;
         setPlaylistOpen(false);
      }
      document.addEventListener("pointerdown", handlePointerDown);
      return () =>
         document.removeEventListener("pointerdown", handlePointerDown);
   }, [playlistOpen]);


   /** Show controls + cursor and restart the idle countdown. Uses only
    * refs and setters, so stale closures (the pointer effect) are safe. */
   function showControls() {
      setControlsVisible(true);
      setCursorHidden(false);
      window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(
         hideControlsIfIdle,
         CONTROLS_IDLE_MS,
      );
   }

   function hideControlsIfIdle() {
      // Resting over the top/bottom bars, or with a menu open, keeps them
      // up; the next pointer move restarts the countdown.
      if (hoveringControlsRef.current || menuOpenRef.current) return;
      console.debug("[player] mouse idle, hiding controls");
      setControlsVisible(false);
      setCursorHidden(true);
   }

   function togglePlaylist() {
      setMenu(null);
      setPlaylistOpen((current) => {
         console.debug("[player] toggle episode list", { open: !current });
         return !current;
      });
   }

   function skip(deltaSeconds: number) {
      const video = videoRef.current;
      if (!video || !episodeDuration) return;
      cancelPendingKeyboardSeek();
      setNextCountdown(null);
      seekToEpisodeTime(
         Math.min(
            Math.max(
               video.currentTime + timeOffsetRef.current + deltaSeconds,
               0,
            ),
            episodeDuration,
         ),
      );
   }

   function toggleRemaining() {
      setShowRemaining((v) => {
         try {
            localStorage.setItem(REMAINING_STORAGE_KEY, v ? "0" : "1");
         } catch {
            // Per-viewer convenience only.
         }
         return !v;
      });
   }

   function updateBuffered() {
      const video = videoRef.current;
      if (!video) return;
      const ranges: [number, number][] = [];
      for (let i = 0; i < video.buffered.length; i++) {
         ranges.push([
            video.buffered.start(i) + timeOffsetRef.current,
            video.buffered.end(i) + timeOffsetRef.current,
         ]);
      }
      setBufferedRanges(ranges);
   }

   function flashControls() {
      setControlsVisible(true);
      window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(
         hideControlsIfIdle,
         KEYBIND_FLASH_MS,
      );
   }


   function togglePause() {
      const video = videoRef.current;
      if (!video) return;
      if (video.paused) {
         // play() rejects with AbortError if something pauses the video
         // before it resolves (e.g. closing mid-buffer) - expected.
         video.play().catch((err) => {
            if (err instanceof DOMException && err.name === "AbortError")
               return;
            setError(String(err));
         });
      } else {
         video.pause();
      }
   }

   function cancelPendingKeyboardSeek() {
      keyboardSeekTargetRef.current = null;
      window.clearTimeout(keyboardSeekTimerRef.current);
   }

   /** Seek to an episode (source) time. */
   function seekToEpisodeTime(target: number) {
      const video = videoRef.current;
      if (!video) return;
      video.currentTime = Math.max(0, target - timeOffsetRef.current);
   }

   function handleSeekInput(e: Event) {
      cancelPendingKeyboardSeek();
      setSeekPreview(Number((e.target as HTMLInputElement).value));
   }

   function handleSeekCommit(e: Event) {
      cancelPendingKeyboardSeek();
      const value = Number((e.target as HTMLInputElement).value);
      setSeekPreview(null);
      setNextCountdown(null);
      seekToEpisodeTime(value);
   }

   function handleSeekHover(e: MouseEvent) {
      if (!episodeDuration) return;
      const tooltip = seekTooltipRef.current;
      if (!tooltip) return;
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const x = Math.min(rect.width, Math.max(0, e.clientX - rect.left));
      tooltip.textContent = formatTime((x / rect.width) * episodeDuration);
      tooltip.style.transform = `translateX(${x}px) translateX(-50%)`;
      tooltip.classList.add("shown");
   }

   function hideSeekTooltip() {
      seekTooltipRef.current?.classList.remove("shown");
   }

   function applyVolume(next: number) {
      setVolumeState(next);
      const video = videoRef.current;
      if (video) video.volume = next / 100;
      try {
         localStorage.setItem(VOLUME_STORAGE_KEY, String(next));
      } catch {
         // Per-viewer convenience only.
      }
   }

   function toggleMute() {
      const video = videoRef.current;
      if (!video) return;
      video.muted = !video.muted;
   }

   // Accumulates onto any pending (not yet committed) target so repeated
   // presses within the debounce window stack correctly.
   function seekBy(deltaSeconds: number) {
      const video = videoRef.current;
      if (!video || !episodeDuration) return;
      const base =
         keyboardSeekTargetRef.current ??
         video.currentTime + timeOffsetRef.current;
      const target = Math.min(
         Math.max(base + deltaSeconds, 0),
         episodeDuration,
      );
      keyboardSeekTargetRef.current = target;
      setSeekPreview(target);
      window.clearTimeout(keyboardSeekTimerRef.current);
      keyboardSeekTimerRef.current = window.setTimeout(() => {
         keyboardSeekTargetRef.current = null;
         setSeekPreview(null);
         seekToEpisodeTime(target);
      }, KEYBOARD_SEEK_COMMIT_MS);
   }


   /** Copies the current frame (at the video's native resolution, with
    * the rendered subtitles when the libass canvas can be read) to the
    * clipboard as PNG - Ctrl+C. */
   async function copyFrame() {
      const video = videoRef.current;
      if (!video || video.videoWidth === 0) return;
      // Immediate feedback: the readback + native clipboard write (which
      // also PNG-encodes the frame) takes around a second for 1080p.
      showToast("Copying frame…");
      const canvas = document.createElement("canvas");
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
      const subtitleCanvas =
         rootRef.current?.querySelector<HTMLCanvasElement>("canvas.JASSUB");
      let withSubtitles = false;
      if (subtitleCanvas && activeSubtitleIndex != null) {
         try {
            // The libass canvas is sized to the displayed video box; map it
            // onto the frame.
            const videoBox = video.getBoundingClientRect();
            const subBox = subtitleCanvas.getBoundingClientRect();
            const scale = canvas.width / videoBox.width;
            ctx.drawImage(
               subtitleCanvas,
               (subBox.left - videoBox.left) * scale,
               (subBox.top - videoBox.top) * scale,
               subBox.width * scale,
               subBox.height * scale,
            );
            withSubtitles = true;
         } catch (err) {
            console.debug(
               "[player] subtitle layer not capturable, copying bare frame",
               { err: String(err) },
            );
         }
      }
      try {
         // Raw RGBA over binary IPC to the native clipboard (see
         // copy_frame_to_clipboard for why not navigator.clipboard).
         const pixels = ctx.getImageData(
            0,
            0,
            canvas.width,
            canvas.height,
         ).data;
         await invoke(
            "copy_frame_to_clipboard",
            new Uint8Array(pixels.buffer),
            {
               headers: {
                  "x-width": String(canvas.width),
                  "x-height": String(canvas.height),
               },
            },
         );
         console.info("[player] frame copied", {
            width: canvas.width,
            height: canvas.height,
            withSubtitles,
         });
         showToast("Frame copied");
      } catch (err) {
         console.error("[player] frame copy failed", { err: String(err) });
         showToast("Couldn't copy the frame");
      }
   }

   function cycleSubtitles() {
      const order: (number | null)[] = [
         null,
         ...subtitleTracks.map((t) => t.index),
      ];
      const next =
         order[(order.indexOf(activeSubtitleIndex) + 1) % order.length];
      setActiveSubtitleIndex(next);
      const track = subtitleTracks.find((t) => t.index === next);
      showToast(
         track
            ? `Subtitles: ${subtitleTrackLabel(track, subtitleTracks.indexOf(track))}`
            : "Subtitles off",
      );
   }

   function changeSubtitleDelay(delta: number) {
      setSubtitleDelay((d) => {
         const next = Math.round((d + delta) * 10) / 10;
         showToast(`Subtitle delay ${next > 0 ? "+" : ""}${next.toFixed(1)}s`);
         return next;
      });
   }

   // Player-wide keybinds, ignored only while typing in a text field.
   useEffect(() => {
      function handleKeyDown(e: KeyboardEvent) {
         if (isTypingTarget(e.target)) return;
         // Ctrl+C: copy the current frame - unless text is selected, where the
         // user means an ordinary copy.
         if (
            (e.ctrlKey || e.metaKey) &&
            !e.altKey &&
            e.key.toLowerCase() === "c" &&
            !window.getSelection()?.toString()
         ) {
            e.preventDefault();
            void copyFrame();
            return;
         }
         if (e.ctrlKey || e.metaKey || e.altKey) return;
         switch (e.key.toLowerCase()) {
            case " ":
            case "k":
               togglePause();
               break;
            // Arrow skips leave the controls as they are.
            case "arrowleft":
               e.preventDefault();
               seekBy(-SEEK_STEP_SECONDS);
               return;
            case "arrowright":
               e.preventDefault();
               seekBy(SEEK_STEP_SECONDS);
               return;
            case "j":
               seekBy(-SEEK_STEP_SECONDS_LARGE);
               break;
            case "l":
               seekBy(SEEK_STEP_SECONDS_LARGE);
               break;
            case "arrowup":
               applyVolume(Math.min(100, volume + VOLUME_STEP));
               showToast(`Volume ${Math.min(100, volume + VOLUME_STEP)}%`);
               break;
            case "arrowdown":
               applyVolume(Math.max(0, volume - VOLUME_STEP));
               showToast(`Volume ${Math.max(0, volume - VOLUME_STEP)}%`);
               break;
            case "m":
               toggleMute();
               break;
            case "c":
               cycleSubtitles();
               break;
            case "z":
               changeSubtitleDelay(-SUBTITLE_DELAY_STEP);
               break;
            case "x":
               changeSubtitleDelay(SUBTITLE_DELAY_STEP);
               break;
            case "n":
               if (onNext) onNext();
               break;
            case "p":
               togglePip();
               break;
            case "escape":
               if (menu) {
                  setMenu(null);
                  return;
               }
               if (playlistOpen) {
                  setPlaylistOpen(false);
                  return;
               }
               // Escape leaves fullscreen first, then closes the player.
               if (isPip()) void setPip(false);
               else if (isFullscreen()) setFullscreen(false);
               else onClose();
               return;
            default:
               return;
         }
         e.preventDefault();
         flashControls();
      }

      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, [
      episodeDuration,
      volume,
      onClose,
      onNext,
      menu,
      playlistOpen,
      subtitleTracks,
      activeSubtitleIndex,
   ]);

   const displayTime = seekPreview ?? episodeTime;
   const buffering = !error && !ready;

   // Seek bar fill/thumb and the clock are written straight to the DOM from
   // a requestAnimationFrame loop instead of from React state: `timeupdate`
   // only fires ~4x a second, which made the bar visibly step, and each of
   // those re-rendered the whole player. The loop reads the <video> clock
   // every frame and touches only three nodes.
   const seekPreviewRef = useRef<number | null>(null);
   seekPreviewRef.current = seekPreview;
   const durationRef = useRef(0);
   durationRef.current = episodeDuration;
   const showRemainingRef = useRef(showRemaining);
   showRemainingRef.current = showRemaining;
   const playedFillRef = useRef<HTMLDivElement>(null);
   const seekThumbRef = useRef<HTMLDivElement>(null);
   const clockRef = useRef<HTMLSpanElement>(null);
   useEffect(() => {
      let frame = 0;
      let lastText = "";
      const tick = () => {
         const video = videoRef.current;
         const duration = durationRef.current;
         const time =
            seekPreviewRef.current ??
            (video ? video.currentTime + timeOffsetRef.current : 0);
         const percent =
            duration > 0
               ? Math.min(100, Math.max(0, (time / duration) * 100))
               : 0;
         playedFillRef.current?.style.setProperty(
            "transform",
            `scaleX(${percent / 100})`,
         );
         seekThumbRef.current?.style.setProperty("left", `${percent}%`);
         const text = showRemainingRef.current
            ? `-${formatTime(Math.max(0, duration - time))}`
            : formatTime(time);
         if (text !== lastText && clockRef.current) {
            clockRef.current.textContent = text;
            lastText = text;
         }
         frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);
      return () => cancelAnimationFrame(frame);
   }, []);

   // Short stalls (a segment arriving a beat late) shouldn't flash the
   // buffering mark - only show it once a wait has lasted a moment.
   const BUFFERING_SHOW_DELAY_MS = 350;
   const [showBuffering, setShowBuffering] = useState(true);
   useEffect(() => {
      if (!buffering) {
         setShowBuffering(false);
         return;
      }
      if (!hasPlayed) {
         setShowBuffering(true);
         return;
      }
      const timer = window.setTimeout(
         () => setShowBuffering(true),
         BUFFERING_SHOW_DELAY_MS,
      );
      return () => window.clearTimeout(timer);
   }, [buffering, hasPlayed]);

   // Ambient color: the video's average color, sampled a few times a
   // second into a 16x9 canvas, tints the control dock and top bar so the
   // chrome picks up the scene instead of sitting on it as flat grey. The
   // <video> source is an MSE blob (same-origin), so the canvas isn't
   // tainted. Eased toward each new sample so scene cuts don't flicker.
   const AMBIENT_SAMPLE_MS = 400;
   useEffect(() => {
      if (!videoEl) return;
      const canvas = document.createElement("canvas");
      canvas.width = 16;
      canvas.height = 9;
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      if (!ctx) return;
      let current: [number, number, number] | null = null;
      const timer = window.setInterval(() => {
         if (videoEl.readyState < 2 || videoEl.videoWidth === 0) return;
         try {
            ctx.drawImage(videoEl, 0, 0, 16, 9);
            const data = ctx.getImageData(0, 0, 16, 9).data;
            let r = 0;
            let g = 0;
            let b = 0;
            for (let i = 0; i < data.length; i += 4) {
               r += data[i];
               g += data[i + 1];
               b += data[i + 2];
            }
            const n = data.length / 4;
            const target: [number, number, number] = [r / n, g / n, b / n];
            current = current
               ? (current.map((c, i) => c + (target[i] - c) * 0.35) as [
                    number,
                    number,
                    number,
                 ])
               : target;
            rootRef.current?.style.setProperty(
               "--ambient",
               `${Math.round(current[0])}, ${Math.round(current[1])}, ${Math.round(current[2])}`,
            );
         } catch (err) {
            console.debug("[player] ambient sample failed", {
               err: String(err),
            });
            window.clearInterval(timer);
         }
      }, AMBIENT_SAMPLE_MS);
      return () => window.clearInterval(timer);
   }, [videoEl]);

   // Keeps a closing menu mounted for its exit animation.
   const MENU_EXIT_MS = 180;
   const [renderedMenu, setRenderedMenu] = useState<Menu>(null);
   useEffect(() => {
      if (menu) {
         setRenderedMenu(menu);
         return;
      }
      const timer = window.setTimeout(
         () => setRenderedMenu(null),
         MENU_EXIT_MS,
      );
      return () => window.clearTimeout(timer);
   }, [menu]);
   // Every stretch the transcode has produced (instantly seekable), not raw
   // torrent download progress - see torrent-engine's StreamStats.
   const readyRanges = episodeDuration && stats ? stats.readyRanges : [];
   const title = displayTitle(anime.title);
   const group = releaseGroup(selectedRelease);
   const resolution = releaseResolution(selectedRelease);

   return (
      <div
         ref={rootRef}
         class={`player-view hls-player${controlsVisible || menu ? " controls-shown" : ""}${playlistOpen ? " playlist-open" : ""}`}
         style={{ cursor: cursorHidden ? "none" : "auto" }}
      >
         {selectedFile && (
            <video
               ref={(el) => {
                  if (videoRef.current === el) return;
                  videoRef.current = el;
                  if (el) el.volume = volume / 100;
                  setVideoEl(el);
               }}
               class={`player-video${hasPlayed ? " shown" : ""}`}
               autoPlay
               // No click-to-pause or double-click fullscreen (same as
               // the mpv player): Space/K, F, or the buttons.
               onLoadedMetadata={(e) =>
                  setDuration((e.target as HTMLVideoElement).duration || 0)
               }
               onDurationChange={(e) =>
                  setDuration((e.target as HTMLVideoElement).duration || 0)
               }
               onTimeUpdate={(e) =>
                  setPosition((e.target as HTMLVideoElement).currentTime)
               }
               onProgress={updateBuffered}
               onSeeked={updateBuffered}
               onPlay={() => setPaused(false)}
               onPause={() => setPaused(true)}
               onEnded={handleEnded}
               onVolumeChange={(e) =>
                  setMuted((e.target as HTMLVideoElement).muted)
               }
               onCanPlay={() => setReady(true)}
               onWaiting={() => setReady(false)}
               onPlaying={() => {
                  setReady(true);
                  setHasPlayed(true);
                  rememberGroupOnPlay();
               }}
               onError={(e) => {
                  const video = e.target as HTMLVideoElement;
                  const mediaError = video.error;
                  // MediaError.code: 1=ABORTED, 2=NETWORK, 3=DECODE, 4=SRC_NOT_SUPPORTED.
                  console.error("[player] video error", {
                     code: mediaError?.code,
                     message: mediaError?.message,
                  });
                  setError(
                     "This file can't be decoded by the player. Try another source.",
                  );
               }}
            />
         )}

         <div class="player-topbar">
            <button
               class="player-icon-button"
               onClick={onClose}
               aria-label="Close player"
               title="Back (Esc)"
            >
               <BackIcon size={22} />
            </button>
            <div class="player-heading">
               <div class="player-heading-title">{title}</div>
               <div class="player-heading-sub">
                  {episodeKey}
                  {group && <span class="player-chip">{group}</span>}
                  {resolution && <span class="player-chip">{resolution}p</span>}
                  {stats?.videoMode && stats.videoMode !== "direct" && (
                     <span
                        class="player-chip player-chip-accent"
                        title={stats.videoMode}
                     >
                        Converting to H.264
                     </span>
                  )}
               </div>
            </div>
         </div>

         {error && (
            <div class="player-message player-error" role="alert">
               <p>{error}</p>
               {sortedReleases.length > 1 && (
                  <button
                     class="button button-quiet"
                     onClick={() => setMenu("sources")}
                  >
                     Choose another source
                  </button>
               )}
            </div>
         )}

         {showBuffering && buffering && !error && (
            <div class="player-loading">
               <Buffering progress={loadingProgress(stats)} />
               <div class="player-loading-status">
                  {selectedFile
                     ? resumeAtRef.current != null && !duration
                        ? `Resuming at ${formatTime(resumeAtRef.current)}`
                        : status
                     : status}
                  {stats && stats.connectedPeers > 0 && (
                     <span>
                        {stats.connectedPeers} peers ·{" "}
                        {stats.downloadSpeedMbps.toFixed(1)} MB/s
                     </span>
                  )}
               </div>
            </div>
         )}


         {toast && (
            <div class="player-toast" key={toast}>
               {toast}
            </div>
         )}


         {playlist.length > 1 && (
            <PlayerPlaylist
               title={title}
               items={playlist}
               currentKey={episodeKey}
               open={playlistOpen}
               onSelect={selectFromPlaylist}
               onClose={closePlaylist}
            />
         )}

         {nextCountdown != null && onNext && (
            <div class="player-next-card">
               <div
                  class="player-next-progress"
                  style={{ animationDuration: `${AUTOPLAY_NEXT_SECONDS}s` }}
               />
               <div class="player-next-label">Up next</div>
               <div class="player-next-title">{nextLabel}</div>
               <div class="player-next-actions">
                  <button class="button button-primary" onClick={onNext}>
                     Play now ({nextCountdown})
                  </button>
                  <button
                     class="button button-quiet"
                     onClick={() => setNextCountdown(null)}
                  >
                     Cancel
                  </button>
               </div>
            </div>
         )}

         {renderedMenu && (
            <div class={`player-menu${menu ? " open" : ""}`} ref={menuRef}>
               {renderedMenu === "subtitles" && (
                  <>
                     <div class="player-menu-title">Subtitles</div>
                     <button
                        class={`player-menu-item${activeSubtitleIndex == null ? " selected" : ""}`}
                        onClick={() => setActiveSubtitleIndex(null)}
                     >
                        Off
                     </button>
                     {subtitleTracks.map((track, i) => (
                        <button
                           key={track.index}
                           class={`player-menu-item${track.index === activeSubtitleIndex ? " selected" : ""}`}
                           onClick={() => setActiveSubtitleIndex(track.index)}
                        >
                           {subtitleTrackLabel(track, i)}
                           <span class="player-menu-meta">
                              {isStyledTrack(track)
                                 ? "Styled"
                                 : track.codec.toUpperCase()}
                           </span>
                        </button>
                     ))}
                     {subtitleTracks.length === 0 && (
                        <div class="player-menu-empty">
                           Looking for subtitle tracks…
                        </div>
                     )}
                     <div class="player-menu-row">
                        <span>Delay</span>
                        <button
                           class="player-step"
                           onClick={() =>
                              changeSubtitleDelay(-SUBTITLE_DELAY_STEP)
                           }
                           aria-label="Earlier"
                        >
                           −
                        </button>
                        <span class="player-menu-value">
                           {subtitleDelay.toFixed(1)}s
                        </span>
                        <button
                           class="player-step"
                           onClick={() =>
                              changeSubtitleDelay(SUBTITLE_DELAY_STEP)
                           }
                           aria-label="Later"
                        >
                           +
                        </button>
                     </div>
                  </>
               )}
               {renderedMenu === "sources" && (
                  <>
                     {videoFiles.length > 1 && (
                        <>
                           <div class="player-menu-title">Episode file</div>
                           <div class="player-menu-scroll">
                              {videoFiles.map((file) => (
                                 <button
                                    key={file.index}
                                    class={`player-menu-item${file.index === selectedFile?.index ? " selected" : ""}`}
                                    onClick={() => {
                                       resumeAtRef.current = null;
                                       setSelectedFile(file);
                                       setMenu(null);
                                    }}
                                 >
                                    {fileLabel(file)}
                                 </button>
                              ))}
                           </div>
                        </>
                     )}
                     <div class="player-menu-title">Source</div>
                     <div class="player-menu-scroll">
                        {sortedReleases.map((r) => {
                           const g = releaseGroup(r);
                           return (
                              <button
                                 key={r.magnet}
                                 class={`player-menu-item player-source-item${r.magnet === selectedRelease.magnet ? " selected" : ""}`}
                                 onClick={() => {
                                    setSelectedRelease(r);
                                    setMenu(null);
                                 }}
                              >
                                 <span class="player-source-title">
                                    {r.title}
                                 </span>
                                 <span class="player-menu-meta">
                                    <span class="seeders">
                                       {r.seeders} seeders
                                    </span>{" "}
                                    · {r.size}
                                    {!codecPlayable(releaseCodec(r)) && (
                                       <span class="codec-note">
                                          {" "}
                                          · {releaseCodec(r)?.toUpperCase()},
                                          converted to H.264
                                       </span>
                                    )}
                                    {g && g === preferredGroup
                                       ? " · your usual group"
                                       : ""}
                                 </span>
                              </button>
                           );
                        })}
                     </div>
                  </>
               )}
               {renderedMenu === "stats" && stats && (
                  <StatisticsMenu
                     stats={stats}
                     infoHash={infoHash}
                     video={null}
                     animeId={anime.id}
                     episode={episode}
                     subtitleLabel={activeSubtitle ? subtitleTrackLabel(activeSubtitle, subtitleTracks.indexOf(activeSubtitle)) : null}
                  />
               )}
            </div>
         )}

         {/* Always-present, invisible strips with the control bars'
          footprint: they're what the pointer hits while the bars are hidden
          (pointer-events:none), so resting there keeps the controls up -
          see CONTROLS_AREA_SELECTOR. */}
         <div class="player-controls-hover-zone" />
         <div class="player-controls-hover-zone player-top-hover-zone" />

         <div
            class={`player-controls${controlsVisible || menu ? " visible" : ""}`}
         >
            <div
               class="player-seek-wrap"
               onMouseMove={handleSeekHover}
               onMouseLeave={hideSeekTooltip}
            >
               <div class="player-seek-track" />
               {readyRanges.map(([start, end]) => (
                  <div
                     key={start}
                     class="player-seek-ready"
                     style={{
                        left: `${Math.min(100, (start / episodeDuration) * 100)}%`,
                        width: `${Math.max(0, Math.min(100, ((end - start) / episodeDuration) * 100))}%`,
                     }}
                  />
               ))}
               {episodeDuration > 0 &&
                  bufferedRanges.map(([start, end]) => (
                     <div
                        key={`b${start}`}
                        class="player-seek-buffered"
                        style={{
                           left: `${Math.max(0, Math.min(100, (start / episodeDuration) * 100))}%`,
                           width: `${Math.max(0, Math.min(100, ((end - start) / episodeDuration) * 100))}%`,
                        }}
                     />
                  ))}
               <div class="player-seek-played" ref={playedFillRef} />
               <div class="player-seek-thumb" ref={seekThumbRef} />
               <div class="player-seek-tooltip" ref={seekTooltipRef} />
               <input
                  class="player-seek"
                  type="range"
                  min={0}
                  max={episodeDuration || 1}
                  step={0.1}
                  value={displayTime}
                  aria-label="Seek"
                  onInput={handleSeekInput}
                  onChange={handleSeekCommit}
                  disabled={!episodeDuration}
               />
            </div>
            <div class="player-controls-row">
               <button
                  class="player-icon-button"
                  onClick={togglePause}
                  aria-label={paused ? "Play" : "Pause"}
                  title={`${paused ? "Play" : "Pause"} (Space)`}
               >
                  {paused ? <PlayIcon size={22} /> : <PauseIcon size={22} />}
               </button>
               <button
                  class="player-icon-button"
                  onClick={() => skip(-SEEK_STEP_SECONDS_LARGE)}
                  aria-label="Back 10 seconds"
                  title="Back 10s (J)"
                  disabled={!episodeDuration}
               >
                  <SkipBackIcon />
               </button>
               <button
                  class="player-icon-button"
                  onClick={() => skip(SEEK_STEP_SECONDS_LARGE)}
                  aria-label="Forward 10 seconds"
                  title="Forward 10s (L)"
                  disabled={!episodeDuration}
               >
                  <SkipForwardIcon />
               </button>
               {onNext && (
                  <button
                     class="player-icon-button"
                     onClick={onNext}
                     aria-label="Next episode"
                     title={`${nextLabel ?? "Next episode"} (N)`}
                  >
                     <NextIcon size={20} />
                  </button>
               )}
               <div class="player-volume-group">
                  <button
                     class="player-icon-button"
                     onClick={toggleMute}
                     aria-label={muted ? "Unmute" : "Mute"}
                     title="Mute (M)"
                  >
                     {muted || volume === 0 ? <MuteIcon /> : <VolumeIcon />}
                  </button>
                  <input
                     class="player-volume"
                     type="range"
                     min={0}
                     max={100}
                     step={1}
                     value={muted ? 0 : volume}
                     style={{ "--fill": `${muted ? 0 : volume}%` }}
                     onInput={(e) => {
                        if (muted && videoRef.current)
                           videoRef.current.muted = false;
                        applyVolume(
                           Number((e.target as HTMLInputElement).value),
                        );
                     }}
                     aria-label="Volume"
                  />
               </div>
               <button
                  class="player-time"
                  onClick={toggleRemaining}
                  title="Show remaining time"
                  aria-label="Toggle remaining time"
               >
                  <span class="player-clock" ref={clockRef} />{" "}
                  <span>/ {formatTime(episodeDuration)}</span>
               </button>
               <div class="player-spacer" />
               <button
                  data-menu-toggle
                  class={`player-icon-button${menu === "subtitles" ? " active" : ""}${activeSubtitleIndex != null ? " on" : ""}`}
                  onClick={() => {
                     setPlaylistOpen(false);
                     setMenu((m) => (m === "subtitles" ? null : "subtitles"));
                  }}
                  aria-label="Subtitles"
                  title="Subtitles (C to cycle, Z/X delay)"
               >
                  <SubtitlesIcon />
               </button>
               <button
                  data-menu-toggle
                  class={`player-icon-button${menu === "sources" ? " active" : ""}`}
                  onClick={() => {
                     setPlaylistOpen(false);
                     setMenu((m) => (m === "sources" ? null : "sources"));
                  }}
                  aria-label="Sources"
                  title="Sources and files"
               >
                  <SourcesIcon />
               </button>
               {stats && (
                  <button
                     data-menu-toggle
                     class={`player-icon-button${menu === "stats" ? " active" : ""}`}
                     onClick={() =>
                        setMenu((m) => (m === "stats" ? null : "stats"))
                     }
                     aria-label="Statistics"
                     title="Statistics"
                  >
                     <StatsIcon />
                  </button>
               )}
               {playlist.length > 1 && (
                  <button
                     data-playlist-toggle
                     class={`player-icon-button${playlistOpen ? " active" : ""}`}
                     onClick={togglePlaylist}
                     aria-label="Episodes"
                     aria-expanded={playlistOpen}
                     title="Episodes"
                  >
                     <EpisodesIcon />
                  </button>
               )}
               <button
                  class={`player-icon-button${pip ? " active" : ""}`}
                  onClick={togglePip}
                  aria-label={pip ? "Exit picture-in-picture" : "Picture-in-picture"}
                  title={`${pip ? "Exit picture-in-picture" : "Picture-in-picture"} (P)`}
               >
                  <PipIcon />
               </button>
               <button
                  class="player-icon-button"
                  onClick={toggleFullscreen}
                  aria-label={fullscreen ? "Exit fullscreen" : "Fullscreen"}
                  title={`${fullscreen ? "Exit fullscreen" : "Fullscreen"} (F)`}
               >
                  {fullscreen ? <ExitFullscreenIcon /> : <FullscreenIcon />}
               </button>
            </div>
         </div>
      </div>
   );
}
