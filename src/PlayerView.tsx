import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";
import { isFullscreen, setFullscreen, toggleFullscreen, useFullscreen } from "./fullscreen";
import { isPip, setPip, togglePip, usePip } from "./pip";
import { MpvVideo, type Chapter } from "./mpvVideo";
import { isTypingTarget } from "./keyboard";
import { ExportDialog, type ExportRequest } from "./ExportDialog";
import type { AnimeMedia, NyaaResult, PlayFile, StreamStats, SubtitleTrack } from "./types";
import { displayTitle, isMovie } from "./types";
import { getStreamStats, playMagnet, stopPlayback, streamFileLoaded, type ResumeRequest } from "./playback";
import { saveFrameThumbnail } from "./torrentThumbnail";
import { loadingProgress } from "./loadingProgress";
import { bestRelease, isBatchRelease, getAnimeResolution, getPreferredGroup, releaseBadges, releaseGroup, releaseResolution, seederHealth, setAnimeResolution, setPreferredGroup, sortReleases } from "./releases";
import type { PreferredResolution } from "./settings";
import { parseEpisode } from "./episodeParser";
import { Buffering } from "./Buffering";
import { StatisticsMenu } from "./StatisticsMenu";
import { getSettings as getSettingsSnapshot, useSettings } from "./settings";
import { applyMpvSubtitleStyle, mpvSubtitleSettings, defaultSubtitleIndex, isStyledTrack, subtitleTrackLabel } from "./subtitles";
import { COMPLETED_FRACTION, MIN_RESUME_SECONDS, getProgress, resumePosition, saveProgress } from "./watchProgress";
import { PlayerPlaylist, type PlaylistItem } from "./PlayerPlaylist";
import {
  BackIcon,
  EpisodesIcon,
  ExitFullscreenIcon,
  FullscreenIcon,
  PipIcon,
  MuteIcon,
  NextIcon,
  SkipBackIcon,
  SkipForwardIcon,
  PauseIcon,
  PlayIcon,
  SourcesIcon,
  StatsIcon,
  SubtitlesIcon,
  VolumeIcon,
} from "./icons";

const STATS_POLL_MS = 1000;
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
// does the same): each real seek makes mpv drop its demuxer position and
// may wait on new torrent pieces, so committing on every press stutters.
const KEYBOARD_SEEK_COMMIT_MS = 300;
const VOLUME_STEP = 5;
const SUBTITLE_DELAY_STEP = 0.1;
const AUDIO_DELAY_STEP = 0.1;
/** Playback speed presets stepped through by `[` / `]`. */
const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
/** Jump when Shift is tapped in a file with no opening chapter marked - a
 * standard anime OP length. */
const INTRO_SKIP_SECONDS = 90;
const INTRO_CHAPTER = /^(op|opening|intro)\b|\bopening\b/i;
const PROGRESS_SAVE_MS = 5000;
// Countdown shown before auto-starting the next episode.
const AUTOPLAY_NEXT_SECONDS = 8;
const VOLUME_STORAGE_KEY = "nyaa-stream:volume";
const REMAINING_STORAGE_KEY = "nyaa-stream:show-remaining";

/** Thumbnail width for the last-frame capture (16:9 cards). */
const LAST_FRAME_WIDTH = 640;

/** Saves the frame on screen as the episode's thumbnail as the player
 * closes (see `saveFrameThumbnail`), then unloads mpv - the grab has to
 * happen while mpv still holds the frame. Any failure just skips the
 * thumbnail. */
async function captureLastFrameAndDetach({ video, animeId, episode, hasPlayed }: { video: MpvVideo | null; animeId: number; episode: number | null; hasPlayed: boolean }) {
   if (!video) return;
   await video.freeze();
   if (episode == null || !hasPlayed || video.readyState < 2) {
      console.debug("[player] last-frame capture skipped", { animeId, episode, hasPlayed, readyState: video.readyState });
   } else {
      try {
         const jpeg = await invoke<ArrayBuffer>("mpv_frame", { width: LAST_FRAME_WIDTH });
         console.info("[player] captured last frame", { animeId, episode, bytes: jpeg.byteLength });
         void saveFrameThumbnail(animeId, episode, new Blob([jpeg], { type: "image/jpeg" }));
      } catch (err) {
         console.warn("[player] last-frame capture failed", { animeId, episode, err: String(err) });
      }
   }
   await video.detach();
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

/** Maps the torrent's downloaded byte runs onto the timeline for the seek
 * bar's dim "downloaded" layer. Linear bytes -> seconds (no container
 * index yet - PLAN.md "Seek-bar buffer indicators"); runs less than a
 * second apart are joined and sub-second specks dropped so the bar stays
 * readable. */
function downloadedTimeRanges(byteRanges: [number, number][], totalBytes: number, duration: number): [number, number][] {
   if (!totalBytes || !duration || byteRanges.length === 0) return [];
   const toTime = (bytes: number) => Math.min(duration, (bytes / totalBytes) * duration);
   const merged: [number, number][] = [];
   for (const [startByte, endByte] of byteRanges) {
      const start = toTime(startByte);
      const end = toTime(endByte);
      const last = merged[merged.length - 1];
      if (last && start - last[1] < 1) last[1] = Math.max(last[1], end);
      else merged.push([start, end]);
   }
   return merged.filter(([start, end]) => end - start >= 0.5);
}

function infoHashFromMagnet(magnet: string): string | null {
   return (
      magnet.match(/xt=urn:btih:([a-zA-Z0-9]+)/)?.[1]?.toLowerCase() ?? null
   );
}

function baseName(path: string): string {
   return path.split(/[\\/]/).pop() ?? path;
}

/** The batch release each anime last played from, so the next episode
 * continues in the same torrent instead of jumping to another source. */
const lastBatchMagnet = new Map<number, string>();

/** The source an unfinished episode was last watched from, if any. */
function resumeSourceMagnet(animeId: number, episodeKey: string): string | undefined {
   if (!getSettingsSnapshot().resumePlayback) return undefined;
   const entry = getProgress(animeId, episodeKey);
   return entry && !entry.completed ? entry.source?.magnet : undefined;
}

/** The video file of the episode after `current` in the same torrent
 * (batches), if its name parses to that number. */
function nextEpisodeFile(files: PlayFile[], current: PlayFile): PlayFile | null {
   const videos = files.filter((f) => f.isVideo);
   if (videos.length < 2) return null;
   const label = parseEpisode(baseName(current.name));
   if (label.kind !== "episode") return null;
   const next = videos.filter((f) => {
      const l = parseEpisode(baseName(f.name));
      return l.kind === "episode" && l.number === label.number + 1;
   });
   return next.length > 0 ? next.reduce((a, b) => (b.length > a.length ? b : a)) : null;
}

/** The file inside the torrent to play: the one whose name parses to the
 * requested episode (batches), else the backend's default (largest video).
 * `matched` is true when a batch's file was identified as that episode. */
function pickFile(
   files: PlayFile[],
   defaultIdx: number,
   episode: number | null,
): { file: PlayFile; matched: boolean } | null {
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
         return { file: pick, matched: true };
      }
      console.warn(
         "[player] no file in torrent matched episode, using default",
         { episode },
      );
   }
   // A batch opened as a whole (no episode number): start from its first
   // file by name rather than the largest, which is an arbitrary episode.
   if (videos.length > 1 && episode == null) {
      const first = [...videos].sort((x, y) =>
         x.name.localeCompare(y.name, undefined, { numeric: true }),
      )[0];
      return { file: first, matched: false };
   }
   const fallback = files.find((f) => f.index === defaultIdx) ?? videos[0] ?? files[0] ?? null;
   return fallback ? { file: fallback, matched: videos.length <= 1 } : null;
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

// Full-viewport player: the embedded mpv (see `MpvVideo`/src-tauri's
// player.rs) draws the video under this transparent overlay, which owns the
// controls. The control bar only appears while the pointer is over the
// bottom strip (or briefly after a keybind), plus PotPlayer/YouTube-style
// keybinds.
function MpvPlayerView({
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
   const videoRef = useRef<MpvVideo | null>(null);
   const selectedFileRef = useRef<PlayFile | null>(null);
   const rootRef = useRef<HTMLDivElement>(null);
   const [videoEl, setVideoEl] = useState<MpvVideo | null>(null);
   const preferredGroup = settings.rememberFansubGroup
      ? getPreferredGroup(anime.id)
      : null;
   // This show's own quality choice (source menu) beats the global setting.
   const [animeResolution, setAnimeResolutionState] = useState(() => getAnimeResolution(anime.id));
   const releasePrefs = {
      preferredGroup,
      preferredFansubber: settings.preferredFansubber,
      preferredResolution: animeResolution ?? settings.preferredResolution,
   };
   const [selectedRelease, setSelectedRelease] = useState<NyaaResult>(
      () =>
         // Keep playing from the same batch across episodes: its next
         // episode is already being preloaded (see the preload effect).
         // A resume prefers the release it was watched from (its resume
         // buffer matches those bytes), when still listed.
         releases.find((r) => r.magnet === resumeSourceMagnet(anime.id, episodeKey)) ??
         releases.find((r) => r.magnet === lastBatchMagnet.get(anime.id)) ??
         bestRelease(releases, releasePrefs),
   );
   useEffect(() => {
      if (isBatchRelease(selectedRelease)) lastBatchMagnet.set(anime.id, selectedRelease.magnet);
      else lastBatchMagnet.delete(anime.id);
   }, [selectedRelease.magnet]);
   const [torrentId, setTorrentId] = useState<string | null>(null);
   const [files, setFiles] = useState<PlayFile[]>([]);
   const [selectedFile, setSelectedFile] = useState<PlayFile | null>(null);
   selectedFileRef.current = selectedFile;
   const [stats, setStats] = useState<StreamStats | null>(null);
   const [error, setError] = useState<string | null>(null);
   const [status, setStatus] = useState("Connecting to peers…");
   const [ready, setReady] = useState(false);
   // Whether the current file has shown its first frame.
   const [hasPlayed, setHasPlayed] = useState(false);
   const [controlsVisible, setControlsVisible] = useState(false);
   // For handlers registered once at mount (window key listeners).
   const controlsVisibleRef = useRef(false);
   controlsVisibleRef.current = controlsVisible;
   const [cursorHidden, setCursorHidden] = useState(false);
   const [paused, setPaused] = useState(true);
   // Target of a seek still waiting on data - the loading overlay says so.
   const [seekingTo, setSeekingTo] = useState<number | null>(null);
   const [duration, setDuration] = useState(0);
   // Media time as mpv reports it (real episode time).
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
   // What mpv has demuxed ahead (instantly seekable), in episode time.
   const [bufferedRanges, setBufferedRanges] = useState<[number, number][]>([]);
   const [showRemaining, setShowRemaining] = useState(() => {
      try {
         return localStorage.getItem(REMAINING_STORAGE_KEY) === "1";
      } catch {
         return false;
      }
   });
   const [subtitleTracks, setSubtitleTracks] = useState<SubtitleTrack[]>([]);
   // Bumped whenever mpv (re)loads the active subtitle track's ASS header.
   const [subtitleHeaderSeq, setSubtitleHeaderSeq] = useState(0);
   // null = subtitles off.
   const [activeSubtitleIndex, setActiveSubtitleIndex] = useState<
      number | null
   >(null);
   const [subtitleDelay, setSubtitleDelay] = useState(0);
   const [audioTracks, setAudioTracks] = useState<SubtitleTrack[]>([]);
   const [activeAudioId, setActiveAudioId] = useState<number | null>(null);
   const [audioDelay, setAudioDelay] = useState(0);
   const [chapters, setChapters] = useState<Chapter[]>([]);
   const [speed, setSpeed] = useState(1);
   // A-B loop points in episode time (null = unset); both set = looping.
   const [loopA, setLoopA] = useState<number | null>(null);
   const [loopB, setLoopB] = useState<number | null>(null);
   // Whether the playing file was identified as the requested episode (always true for a single-file torrent).
   const [fileMatched, setFileMatched] = useState(true);
   const [exporting, setExporting] = useState(false);
   const [exportOpen, setExportOpen] = useState(false);
   const [exportSaved, setExportSaved] = useState<string | null>(null);
   const [exportError, setExportError] = useState<string | null>(null);
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
      [releases, animeResolution],
   );
   const videoFiles = files.filter((f) => f.isVideo);

   // Source switch = flush + seek back (PLAN.md "Fast playback start"):
   // capture the playhead (once playback started; before that the pending
   // resume point stays), drop the old file from mpv at once, then let the
   // play_magnet effect start the new source at that time with the
   // "resume" watch hint.
   async function switchSource(release: NyaaResult) {
      setMenu(null);
      if (release.magnet === selectedRelease.magnet) return;
      const video = videoRef.current;
      if (video && hasPlayed) resumeAtRef.current = video.currentTime;
      console.info("[player] switching source", { title: release.title, resumeAt: resumeAtRef.current, hasPlayed });
      if (video) {
         await video.stop().catch((err) => console.warn("[player] mpv stop before source switch failed", { err: String(err) }));
      }
      setSelectedRelease(release);
   }

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
            const session = await playMagnet(
               selectedRelease.magnet,
               `${displayTitle(anime.title)} ${episodeKey}`,
               resumeAtRef.current != null ? "resume" : "first",
               `${anime.id}:${episodeKey}`,
            );
            if (cancelled) return;
            // A movie is the torrent's largest video (the backend's
            // default) - never an extra that happens to parse as "01".
            const picked = isMovie(anime)
               ? ((f) => (f ? { file: f, matched: true } : null))(session.files.find((f) => f.index === session.defaultFileIdx))
               : pickFile(session.files, session.defaultFileIdx, episode);
            const file = picked?.file ?? null;
            if (!picked || !file) {
               setError("This torrent has no playable video file.");
               return;
            }
            setTorrentId(session.torrentId);
            setFiles(session.files);
            setFileMatched(picked.matched);
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

   // Batch source: once this episode is playing, fetch the next episode's
   // file from the same torrent at the lowest priority (spare bandwidth
   // only), so pressing next starts from data that is already there.
   useEffect(() => {
      if (!hasPlayed || !torrentId || !selectedFile || !fileMatched) return;
      const next = nextEpisodeFile(files, selectedFile);
      if (!next) return;
      console.info("[player] preloading next episode file", { file: next.name });
      invoke("preload_next_file", { torrentId, fileIdx: next.index }).catch((err) =>
         console.debug("[player] preload unavailable", { err: String(err) }),
      );
   }, [hasPlayed, torrentId, selectedFile?.index, fileMatched]);

   // Per-file state resets (source switch or batch file switch).
   useEffect(() => {
      setDuration(0);
      setPosition(0);
      setHasPlayed(false);
      setBufferedRanges([]);
      setSubtitleTracks([]);
      setActiveSubtitleIndex(null);
      subtitlesPickedRef.current = false;
      setSubtitleDelay(0);
      setAudioTracks([]);
      setActiveAudioId(null);
      setAudioDelay(0);
      setChapters([]);
      setLoopA(null);
      setLoopB(null);
      setNextCountdown(null);
   }, [selectedFile?.streamUrl]);

   // Latest values for the unmount capture below.
   const lastFrameRef = useRef({ video: null as MpvVideo | null, animeId: anime.id, episode, hasPlayed: false });
   lastFrameRef.current = { video: videoEl, animeId: anime.id, episode, hasPlayed };
   useEffect(() => () => void captureLastFrameAndDetach(lastFrameRef.current), []);

   // Only the player overlay may cover mpv while it's mounted (App.css).
   useEffect(() => {
      document.documentElement.classList.add("mpv-active");
      return () => document.documentElement.classList.remove("mpv-active");
   }, []);

   // One mpv facade per player mount; its events drive the same state the
   // <video> element's used to.
   useEffect(() => {
      const video = new MpvVideo();
      videoRef.current = video;
      const on = (type: string, handler: () => void) => video.addEventListener(type, handler);
      on("durationchange", () => setDuration(video.duration));
      on("loadedmetadata", () => setDuration(video.duration));
      on("timeupdate", () => setPosition(video.currentTime));
      on("progress", updateBuffered);
      on("seeked", updateBuffered);
      on("play", () => setPaused(false));
      on("pause", () => setPaused(true));
      on("ended", () => handleEndedRef.current());
      on("volumechange", () => setMuted(video.muted));
      on("canplay", () => setReady(true));
      on("waiting", () => {
         setReady(false);
         setSeekingTo(video.seekingTo);
      });
      on("playing", () => {
         setReady(true);
         setSeekingTo(null);
         setHasPlayed(true);
         rememberGroupOnPlay();
      });
      on("tracks", () => {
         setSubtitleTracks(video.subtitleTracks);
         setAudioTracks(video.audioTracks);
         setActiveAudioId(video.activeAudioId);
      });
      on("chapters", () => setChapters(video.chapters));
      on("subheader", () => setSubtitleHeaderSeq((n) => n + 1));
      on("ratechange", () => setSpeed(video.playbackRate));
      on("error", () =>
         setError(
            `This file can't be played: ${video.error?.message ?? "unknown error"}. Try another source.`,
         ),
      );
      let cancelled = false;
      video
         .attach()
         .then(() => {
            if (cancelled) return;
            video.volume = volume / 100;
            setVideoEl(video);
         })
         .catch((err) => {
            console.error("[player] mpv failed to start", { err: String(err) });
            setError(`The player couldn't start: ${String(err)}. Is libmpv-2.dll available?`);
         });
      return () => {
         cancelled = true;
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, []);

   // Opens the chosen file in mpv, starting at the resume point.
   useEffect(() => {
      const video = videoEl;
      if (!video || !selectedFile) return;
      const startAt = resumeAtRef.current;
      console.info("[player] loading file", { file: selectedFile.name, startAt });
      video.load(selectedFile.streamUrl, startAt).catch((err) => {
         console.error("[player] loadfile failed", { err: String(err) });
         setError(`Couldn't open this file: ${String(err)}`);
      });
   }, [videoEl, selectedFile?.streamUrl]);

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

   // mpv has opened the file: the reads so far are what a resume buffer
   // must hold to open it again (PLAN.md "Continue-watching resume buffer").
   useEffect(() => {
      if (!videoEl || torrentId == null || !selectedFile) return;
      const fileIdx = selectedFile.index;
      const onLoaded = () => void streamFileLoaded(torrentId, fileIdx);
      videoEl.addEventListener("loadedmetadata", onLoaded);
      return () => videoEl.removeEventListener("loadedmetadata", onLoaded);
   }, [videoEl, torrentId, selectedFile?.index]);

   useEffect(() => {
      return () => {
         window.clearTimeout(idleTimerRef.current);
         window.clearTimeout(keyboardSeekTimerRef.current);
         window.clearTimeout(toastTimerRef.current);
         stopPlayback(resumeRequest());
      };
   }, []);

   // mpv's track list arrives with the file (and may grow as it loads):
   // pick the default track once per file, then follow the user's choice.
   const subtitlesPickedRef = useRef(false);
   useEffect(() => {
      if (subtitlesPickedRef.current || subtitleTracks.length === 0) return;
      subtitlesPickedRef.current = true;
      const current = getSettingsSnapshot();
      const pick = defaultSubtitleIndex(subtitleTracks, current.subtitleLanguage, current.subtitlesEnabled);
      console.info("[player] subtitle tracks", { count: subtitleTracks.length, pick });
      setActiveSubtitleIndex(pick);
   }, [subtitleTracks]);

   const activeSubtitle =
      subtitleTracks.find((t) => t.index === activeSubtitleIndex) ?? null;

   useEffect(() => {
      if (!videoEl || !subtitlesPickedRef.current) return;
      videoEl.setSubtitle(activeSubtitleIndex);
   }, [videoEl, activeSubtitleIndex, subtitleTracks.length > 0]);

   useEffect(() => {
      void videoEl?.setProperty("sub-delay", subtitleDelay);
   }, [videoEl, subtitleDelay]);

   useEffect(() => {
      void videoEl?.setProperty("audio-delay", audioDelay);
   }, [videoEl, audioDelay]);

   useEffect(() => {
      if (!videoEl) return;
      const hwdec = settings.hardwareDecoding ? "auto-safe" : "no";
      console.info("[player] hardware decoding", { hwdec });
      void videoEl.setProperty("hwdec", hwdec);
   }, [videoEl, settings.hardwareDecoding]);

   // The user's default style, applied by mpv (see applyMpvSubtitleStyle).
   // ASS restyling needs the track's header, which only exists once mpv has
   // loaded the track - on first play that is after this effect first runs,
   // so it re-runs when the header arrives (`subtitleHeaderSeq`).
   useEffect(() => {
      if (!videoEl) return;
      void applyMpvSubtitleStyle(videoEl, settings.subtitleStyle, activeSubtitle ? isStyledTrack(activeSubtitle) : false);
   }, [videoEl, settings.subtitleStyle, activeSubtitle?.index, subtitleHeaderSeq]);

   // mpv reports real file time and the file's own exact duration.
   const episodeTime = position;
   const episodeDuration = duration;
   const downloadedRanges = useMemo(
      () => downloadedTimeRanges(stats?.downloadedByteRanges ?? [], stats?.totalBytes ?? 0, episodeDuration),
      [stats?.downloadedByteRanges, stats?.totalBytes, episodeDuration],
   );

   /** Resume buffer request for stop_playback when the episode is still in
    * progress (would show in Continue watching) - PLAN.md "Continue-watching
    * resume buffer". Reads refs: runs from the unmount cleanup. */
   function resumeRequest(): ResumeRequest | undefined {
      const video = videoRef.current;
      const fileIdx = selectedFileRef.current?.index;
      if (!video || fileIdx == null || !video.duration) return undefined;
      const position = video.currentTime;
      if (position < MIN_RESUME_SECONDS || position / video.duration >= COMPLETED_FRACTION) return undefined;
      return { animeId: anime.id, episodeKey, fileIdx, position, magnet: selectedReleaseRef.current.magnet };
   }

   function persistProgress() {
      const video = videoRef.current;
      // readyState check instead of the `ready` state: this runs from an
      // interval whose closure would otherwise see a stale value.
      if (!video || !video.duration || video.readyState < 1) return;
      const file = selectedFileRef.current;
      saveProgress(
         anime,
         episodeKey,
         episode,
         video.currentTime,
         video.duration,
         file ? { magnet: selectedReleaseRef.current.magnet, fileIdx: file.index, fileName: file.name } : undefined,
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
   // For the mpv listener registered once at mount.
   const handleEndedRef = useRef(handleEnded);
   handleEndedRef.current = handleEnded;

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
               video.currentTime + deltaSeconds,
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
         ranges.push([video.buffered.start(i), video.buffered.end(i)]);
      }
      setBufferedRanges(ranges);
   }

   /** Keyboard gestures (seek, skip, subtitles...) never bring the controls
    * up - their toasts/flashes are the feedback while the mouse is idle. If
    * the pointer already has the controls open, the gesture just counts as
    * activity and keeps them up a little longer. */
   function flashControls() {
      if (!controlsVisibleRef.current) return;
      window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(
         hideControlsIfIdle,
         CONTROLS_IDLE_MS,
      );
   }

   function togglePause() {
      const video = videoRef.current;
      if (!video) return;
      if (video.paused) {
         video.play().catch((err) =>
            console.warn("[player] play failed", { err: String(err) }),
         );
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
      video.currentTime = Math.max(0, target);
   }

   // Pointer-driven seek bar: press previews, drag moves the preview,
   // release commits exactly once. It replaced a controlled
   // <input type=range>, whose re-rendered value fed stale positions back
   // in (a paused seek showed the old time until clicked again), fired
   // commits in bursts mid-drag, and kept focus (disabling Space). The
   // preview lives in a ref the rAF loop reads, so dragging doesn't
   // re-render the player.
   const seekDragRef = useRef<{ pointerId: number } | null>(null);
   const dragPreviewRef = useRef<number | null>(null);

   function seekValueAt(e: PointerEvent): number {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const fraction = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
      return fraction * episodeDuration;
   }

   function handleSeekPointerDown(e: PointerEvent) {
      if (!episodeDuration || e.button !== 0) return;
      e.preventDefault();
      cancelPendingKeyboardSeek();
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      seekDragRef.current = { pointerId: e.pointerId };
      dragPreviewRef.current = seekValueAt(e);
   }

   function handleSeekPointerMove(e: PointerEvent) {
      handleSeekHover(e);
      if (seekDragRef.current?.pointerId === e.pointerId) dragPreviewRef.current = seekValueAt(e);
   }

   function handleSeekPointerUp(e: PointerEvent) {
      if (seekDragRef.current?.pointerId !== e.pointerId) return;
      seekDragRef.current = null;
      const value = seekValueAt(e);
      dragPreviewRef.current = null;
      console.debug("[player] seek bar commit", { value, from: videoRef.current?.currentTime });
      setNextCountdown(null);
      setPosition(value);
      seekToEpisodeTime(value);
   }

   function handleSeekPointerCancel() {
      seekDragRef.current = null;
      dragPreviewRef.current = null;
   }

   function handleSeekHover(e: MouseEvent) {
      if (!episodeDuration) return;
      const tooltip = seekTooltipRef.current;
      if (!tooltip) return;
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const x = Math.min(rect.width, Math.max(0, e.clientX - rect.left));
      const at = (x / rect.width) * episodeDuration;
      const chapter = [...chapters].reverse().find((c) => c.time <= at);
      tooltip.textContent = chapter?.title ? `${formatTime(at)} · ${chapter.title}` : formatTime(at);
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
         keyboardSeekTargetRef.current ?? video.currentTime;
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



   /** Copies the current frame, rendered subtitles included, at the
    * video's native resolution to the clipboard - Ctrl+C. */
   async function copyFrame() {
      if (!videoRef.current || videoRef.current.readyState < 2) return;
      // Immediate feedback: the screenshot + PNG decode + native clipboard
      // write takes around a second for 1080p.
      showToast("Copying frame…");
      try {
         await invoke("mpv_copy_frame");
         console.info("[player] frame copied");
         showToast("Frame copied");
      } catch (err) {
         console.error("[player] frame copy failed", { err: String(err) });
         showToast("Couldn't copy the frame");
      }
   }

   /** Saves the current frame (subtitles included, native resolution) as a
    * PNG in the screenshot folder - X. */
   async function saveFrame() {
      if (!videoRef.current || videoRef.current.readyState < 2) return;
      showToast("Saving frame…");
      const stamp = formatTime(videoRef.current.currentTime).replace(/:/g, ".");
      try {
         const path = await invoke<string>("mpv_save_frame", {
            folder: settings.screenshotFolder || null,
            name: `${displayTitle(anime.title)} ${episodeKey} ${stamp}`,
         });
         console.info("[player] frame saved", { path });
         showToast(`Frame saved: ${baseName(path)}`);
      } catch (err) {
         console.error("[player] frame save failed", { err: String(err) });
         showToast("Couldn't save the frame");
      }
   }

   /** The subtitle track that was showing before subtitles were switched
    * off - what a C tap brings back. */
   const lastSubtitleRef = useRef<number | null>(null);
   if (activeSubtitleIndex != null) lastSubtitleRef.current = activeSubtitleIndex;

   function announceSubtitle(index: number | null) {
      const track = subtitleTracks.find((t) => t.index === index);
      showToast(track ? `Subtitles: ${subtitleTrackLabel(track, subtitleTracks.indexOf(track))}` : "Subtitles off");
   }

   /** C tap: subtitles off, or back to the track that was showing. */
   function toggleSubtitles() {
      if (subtitleTracks.length === 0) return;
      const next = activeSubtitleIndex != null ? null : (subtitleTracks.find((t) => t.index === lastSubtitleRef.current)?.index ?? subtitleTracks[0].index);
      setActiveSubtitleIndex(next);
      announceSubtitle(next);
   }

   /** C held + wheel: steps through off and every subtitle track. */
   function stepSubtitle(direction: 1 | -1) {
      const order: (number | null)[] = [null, ...subtitleTracks.map((t) => t.index)];
      if (order.length < 2) return;
      const next = order[(order.indexOf(activeSubtitleIndex) + direction + order.length) % order.length];
      setActiveSubtitleIndex(next);
      announceSubtitle(next);
   }

   function changeSubtitleDelay(delta: number) {
      setSubtitleDelay((d) => {
         const next = Math.round((d + delta) * 10) / 10;
         showToast(`Subtitle delay ${next > 0 ? "+" : ""}${next.toFixed(1)}s`);
         return next;
      });
   }

   /** A-B loop key: sets A, then B (playback loops A-B in mpv), then clears. */
   function stepAbLoop() {
      const video = videoRef.current;
      if (!video) return;
      const now = video.currentTime;
      if (loopA == null) {
         setLoopA(now);
         void video.setProperty("ab-loop-a", now);
         showToast(`Loop A: ${formatTime(now)}`);
      } else if (loopB == null) {
         if (now <= loopA + 0.5) {
            showToast("Loop B must come after A");
            return;
         }
         setLoopB(now);
         void video.setProperty("ab-loop-b", now);
         showToast(`Looping ${formatTime(loopA)} – ${formatTime(now)} · E exports`);
      } else {
         setLoopA(null);
         setLoopB(null);
         void video.setProperty("ab-loop-a", "no");
         void video.setProperty("ab-loop-b", "no");
         showToast("Loop cleared");
      }
   }

   /** Opens the export dialog for the looped section. */
   function exportLoop() {
      if (loopA == null || loopB == null || !torrentId || !selectedFile || exporting) return;
      setExportSaved(null);
      setExportError(null);
      setExportOpen(true);
   }

   /** Cuts the section to an MP4 (normalized H.264/AAC, see `export_clip`).
    * Needs the torrent to stay open until it finishes. */
   async function runExport(request: ExportRequest) {
      if (!torrentId || !selectedFile || exporting) return;
      setExporting(true);
      setExportSaved(null);
      setExportError(null);
      const burnSubs = request.includeSubs && activeSubtitle != null && videoEl != null;
      console.info("[player] exporting clip", { start: request.start, end: request.end, folder: request.folder || "(default)", subs: burnSubs });
      try {
         const subtitleSettings = burnSubs ? await mpvSubtitleSettings(videoEl, settings.subtitleStyle, isStyledTrack(activeSubtitle)) : null;
         const out = await invoke<string>("export_clip", {
            torrentId,
            fileIdx: selectedFile.index,
            startSeconds: request.start,
            endSeconds: request.end,
            audioStream: request.audioPosition,
            name: request.name,
            folder: request.folder || null,
            includeSubs: burnSubs,
            audioId: audioTracks[request.audioPosition]?.index ?? null,
            subtitleId: burnSubs ? activeSubtitle.index : null,
            subtitleSettings,
         });
         console.info("[player] clip exported", { out });
         setExportSaved(out);
         showToast(`Clip saved: ${baseName(out)}`);
      } catch (err) {
         console.error("[player] clip export failed", { err: String(err) });
         setExportError(`Couldn't export the clip: ${String(err)}`);
      } finally {
         setExporting(false);
      }
   }

   function changeAudioDelay(delta: number) {
      setAudioDelay((d) => {
         const next = Math.round((d + delta) * 10) / 10;
         showToast(`Audio delay ${next > 0 ? "+" : ""}${next.toFixed(1)}s`);
         return next;
      });
   }

   function pickAudio(id: number) {
      setActiveAudioId(id);
      videoRef.current?.setAudio(id);
   }

   /** Steps the playback speed through `SPEEDS`; 0 returns to 1x. */
   function changeSpeed(direction: 1 | -1 | 0) {
      const video = videoRef.current;
      if (!video) return;
      let next = 1;
      if (direction !== 0) {
         const i = SPEEDS.findIndex((s) => Math.abs(s - video.playbackRate) < 0.01);
         const from = i === -1 ? SPEEDS.indexOf(1) : i;
         next = SPEEDS[Math.min(SPEEDS.length - 1, Math.max(0, from + direction))];
      }
      video.playbackRate = next;
      setSpeed(next);
      showToast(`Speed ${next}×`);
   }

   /** The opening chapter playing at `time`, if the file marks one. */
   function openingAt(time: number): { end: number } | null {
      for (let i = 0; i < chapters.length; i++) {
         if (!INTRO_CHAPTER.test(chapters[i].title.trim())) continue;
         const end = chapters[i + 1] ? chapters[i + 1].time : episodeDuration;
         if (time >= chapters[i].time && time < end - 0.5) return { end };
      }
      return null;
   }

   /** The opening (intro chapter) playing at `time` or still to come, as
    * its end - so Shift in a prologue before the opening skips past the
    * whole opening, not just to where it starts. Null when the file marks
    * no opening ahead. */
   function openingFrom(time: number): { end: number } | null {
      for (let i = 0; i < chapters.length; i++) {
         if (!INTRO_CHAPTER.test(chapters[i].title.trim())) continue;
         const end = chapters[i + 1] ? chapters[i + 1].time : episodeDuration;
         if (end - time > 0.5) return { end };
      }
      return null;
   }

   /** Shift: jumps to the end of the opening chapter (see `openingFrom`);
    * only in a file with no opening marked, a fixed 90s. */
   function skipIntro() {
      const video = videoRef.current;
      if (!video || !episodeDuration) return;
      cancelPendingKeyboardSeek();
      const now = video.currentTime;
      const opening = openingFrom(now);
      const target = opening ? opening.end : Math.min(now + INTRO_SKIP_SECONDS, episodeDuration);
      console.info("[player] skip intro", { from: now, to: target, marked: opening != null });
      showToast(opening ? "Skipped opening" : `Skipped ${INTRO_SKIP_SECONDS}s`);
      seekToEpisodeTime(target);
   }

   // Hardware media keys while the player is open (see media_keys.rs).
   const togglePauseRef = useRef(togglePause);
   togglePauseRef.current = togglePause;
   const onNextRef = useRef(onNext);
   onNextRef.current = onNext;
   useEffect(() => {
      let unlisten: (() => void) | undefined;
      let cancelled = false;
      invoke("set_media_keys", { enabled: true }).catch((err) => console.warn("[player] media keys unavailable", { err: String(err) }));
      listen<string>("media-key", (e) => {
         console.debug("[player] media key", { key: e.payload });
         if (e.payload === "play-pause") togglePauseRef.current();
         else if (e.payload === "next") onNextRef.current?.();
         else if (e.payload === "previous" && videoRef.current) seekToEpisodeTimeRef.current(0);
      }).then((fn) => (cancelled ? fn() : (unlisten = fn)));
      return () => {
         cancelled = true;
         unlisten?.();
         invoke("set_media_keys", { enabled: false }).catch(() => {});
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, []);
   const seekToEpisodeTimeRef = useRef(seekToEpisodeTime);
   seekToEpisodeTimeRef.current = seekToEpisodeTime;

   // Windows taskbar progress: the episode's position, yellow while paused.
   const lastTaskbarRef = useRef(0);
   useEffect(() => {
      const now = Date.now();
      if (!duration || (now - lastTaskbarRef.current < 1000 && !paused)) return;
      lastTaskbarRef.current = now;
      void getCurrentWindow()
         .setProgressBar({ status: paused ? ProgressBarStatus.Paused : ProgressBarStatus.Normal, progress: Math.round((position / duration) * 100) })
         .catch(() => {});
   }, [position, duration, paused]);
   useEffect(
      () => () => {
         void getCurrentWindow()
            .setProgressBar({ status: ProgressBarStatus.None })
            .catch(() => {});
      },
      [],
   );

   // C alone toggles subtitles (on release, so holding it for the wheel
   // doesn't also toggle); C held + wheel steps through the tracks.
   const subtitleKeysRef = useRef({ toggle: toggleSubtitles, step: stepSubtitle });
   subtitleKeysRef.current = { toggle: toggleSubtitles, step: stepSubtitle };
   useEffect(() => {
      let held = false;
      let wheelUsed = false;
      let lastStep = 0;
      function down(e: KeyboardEvent) {
         if (e.key.toLowerCase() !== "c" || e.ctrlKey || e.altKey || e.metaKey || isTypingTarget(e.target)) return;
         if (!e.repeat) {
            held = true;
            wheelUsed = false;
         }
      }
      function up(e: KeyboardEvent) {
         if (e.key.toLowerCase() !== "c" || !held) return;
         held = false;
         if (!wheelUsed) subtitleKeysRef.current.toggle();
         flashControls();
      }
      function wheel(e: WheelEvent) {
         if (!held) return;
         e.preventDefault();
         wheelUsed = true;
         // Wheels/touchpads fire in bursts: one step per notch-ish.
         const now = performance.now();
         if (now - lastStep < 140 || e.deltaY === 0) return;
         lastStep = now;
         subtitleKeysRef.current.step(e.deltaY > 0 ? 1 : -1);
      }
      const release = () => {
         held = false;
      };
      window.addEventListener("keydown", down);
      window.addEventListener("keyup", up);
      window.addEventListener("wheel", wheel, { passive: false });
      window.addEventListener("blur", release);
      return () => {
         window.removeEventListener("keydown", down);
         window.removeEventListener("keyup", up);
         window.removeEventListener("wheel", wheel);
         window.removeEventListener("blur", release);
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, []);

   // Shift alone skips the intro. Armed on keydown and cleared by any other
   // key, so Shift+letter combos and typing never trigger it.
   const shiftArmedRef = useRef(false);
   const skipIntroRef = useRef(skipIntro);
   skipIntroRef.current = skipIntro;
   useEffect(() => {
      function down(e: KeyboardEvent) {
         if (isTypingTarget(e.target)) return;
         shiftArmedRef.current = e.key === "Shift" && !e.repeat && !e.ctrlKey && !e.altKey && !e.metaKey;
      }
      function up(e: KeyboardEvent) {
         if (e.key !== "Shift" || !shiftArmedRef.current) return;
         shiftArmedRef.current = false;
         if (isTypingTarget(e.target)) return;
         skipIntroRef.current();
         flashControls();
      }
      window.addEventListener("keydown", down);
      window.addEventListener("keyup", up);
      return () => {
         window.removeEventListener("keydown", down);
         window.removeEventListener("keyup", up);
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
   }, []);

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
            case "-":
               changeSubtitleDelay(-SUBTITLE_DELAY_STEP);
               break;
            case "=":
            case "+":
               changeSubtitleDelay(SUBTITLE_DELAY_STEP);
               break;
            // One frame back/forward (pauses, like mpv).
            case ",":
               cancelPendingKeyboardSeek();
               videoRef.current?.frameStep(-1);
               break;
            case ".":
               cancelPendingKeyboardSeek();
               videoRef.current?.frameStep(1);
               break;
            case "x":
               void saveFrame();
               break;
            case "n":
               if (onNext) onNext();
               break;
            case "a":
               stepAbLoop();
               break;
            case "e":
               exportLoop();
               break;
            case "[":
               changeSpeed(-1);
               break;
            case "]":
               changeSpeed(1);
               break;
            case "\\":
               changeSpeed(0);
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
      loopA,
      loopB,
      exporting,
      torrentId,
      selectedFile,
      audioTracks,
      activeAudioId,
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
            dragPreviewRef.current ??
            seekPreviewRef.current ??
            (video ? video.currentTime : 0);
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

   // The overlay stays mounted through its fade-out instead of vanishing.
   const LOADING_EXIT_MS = 450;
   const loadingVisible = showBuffering && buffering && !error;
   const [loadingMounted, setLoadingMounted] = useState(loadingVisible);
   useEffect(() => {
      if (loadingVisible) {
         setLoadingMounted(true);
         return;
      }
      const timer = window.setTimeout(() => setLoadingMounted(false), LOADING_EXIT_MS);
      return () => window.clearTimeout(timer);
   }, [loadingVisible]);

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
   const title = displayTitle(anime.title);
   const group = releaseGroup(selectedRelease);
   const resolution = releaseResolution(selectedRelease);

   return (
      <div
         ref={rootRef}
         class={`player-view${controlsVisible || menu ? " controls-shown" : ""}${playlistOpen ? " playlist-open" : ""}`}
         style={{ cursor: cursorHidden ? "none" : "auto" }}
      >
         {/* mpv draws under the transparent webview; this layer sits over
          the picture and deliberately does nothing on click or double-click
          - pause is Space/K or the button, fullscreen is F (app-wide) or
          the button. */}
         <div class="player-video-surface" />

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

         {loadingMounted && !error && (
            <div class={`player-loading player-loading-fade${loadingVisible ? " shown" : ""}`}>
               {/* Mid-episode the readiness fill would sit near full, so a
                seek wait shows a sweeping arc instead. */}
               <Buffering progress={loadingProgress(stats)} indeterminate={hasPlayed} />
               <div class="player-loading-status">
                  {seekingTo != null && hasPlayed
                     ? `Seeking to ${formatTime(seekingTo)}`
                     : hasPlayed
                       ? "Buffering…"
                       : selectedFile
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

         {!error && !hasPlayed && selectedRelease.seeders === 0 && (
            <div class="player-message player-dead-warning" role="status">
               <p>This release has no seeders - it may never start.</p>
               {sortedReleases.length > 1 && (
                  <button class="button button-quiet" onClick={() => setMenu("sources")}>
                     Choose another source
                  </button>
               )}
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

         {exportOpen && loopA != null && loopB != null && (
            <ExportDialog
               defaultName={`${displayTitle(anime.title)} ${episodeKey} ${formatTime(loopA)}-${formatTime(loopB)}`.replace(/:/g, ".")}
               start={loopA}
               end={loopB}
               duration={episodeDuration}
               audioTracks={audioTracks}
               activeAudioId={activeAudioId}
               subtitleLabel={activeSubtitle ? subtitleTrackLabel(activeSubtitle, subtitleTracks.indexOf(activeSubtitle)) : null}
               folder={settings.exportFolder}
               exporting={exporting}
               savedPath={exportSaved}
               error={exportError}
               onExport={(request) => void runExport(request)}
               onClose={() => setExportOpen(false)}
            />
         )}

         {hasPlayed && !error && nextCountdown == null && openingAt(position) && (
            <button class="player-skip-intro" onClick={skipIntro}>
               Skip opening <kbd>Shift</kbd>
            </button>
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
                     <div class="player-menu-title">Subtitles &amp; audio</div>
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
                        <span>Sub delay</span>
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
                     {audioTracks.length > 0 && (
                        <>
                           <div class="player-menu-title">Audio</div>
                           {audioTracks.map((track, i) => (
                              <button
                                 key={track.index}
                                 class={`player-menu-item${track.index === activeAudioId ? " selected" : ""}`}
                                 onClick={() => pickAudio(track.index)}
                              >
                                 {subtitleTrackLabel(track, i)}
                                 <span class="player-menu-meta">{track.codec.toUpperCase()}</span>
                              </button>
                           ))}
                        </>
                     )}
                     <div class="player-menu-row">
                        <span>Audio delay</span>
                        <button
                           class="player-step"
                           onClick={() => changeAudioDelay(-AUDIO_DELAY_STEP)}
                           aria-label="Audio earlier"
                        >
                           −
                        </button>
                        <span class="player-menu-value">{audioDelay.toFixed(1)}s</span>
                        <button
                           class="player-step"
                           onClick={() => changeAudioDelay(AUDIO_DELAY_STEP)}
                           aria-label="Audio later"
                        >
                           +
                        </button>
                     </div>
                  </>
               )}
               {renderedMenu === "sources" && (
                  <>
                     {videoFiles.length > 1 && fileMatched && selectedFile && (
                        <>
                           <div class="player-menu-title">Batch source</div>
                           <div class="player-menu-note">
                              Playing {fileLabel(selectedFile)}. Only this episode is downloaded from the batch.
                           </div>
                        </>
                     )}
                     {videoFiles.length > 1 && !fileMatched && (
                        <>
                           <div class="player-menu-title">Batch source · pick the episode file</div>
                           <div class="player-menu-scroll">
                              {videoFiles.map((file) => (
                                 <button
                                    key={file.index}
                                    class={`player-menu-item${file.index === selectedFile?.index ? " selected" : ""}`}
                                    onClick={() => {
                                       resumeAtRef.current = null;
                                       setFileMatched(true);
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
                     <div class="player-menu-row">
                        <span>Quality for this show</span>
                        <select
                           class="player-menu-select"
                           value={animeResolution ?? ""}
                           onChange={(e) => {
                              const value = (e.target as HTMLSelectElement).value as PreferredResolution | "";
                              setAnimeResolution(anime.id, value || null);
                              setAnimeResolutionState(value || null);
                           }}
                        >
                           <option value="">Default ({settings.preferredResolution === "any" ? "most seeded" : `${settings.preferredResolution}p`})</option>
                           <option value="any">Most seeded</option>
                           <option value="2160">2160p</option>
                           <option value="1080">1080p</option>
                           <option value="720">720p</option>
                           <option value="480">480p</option>
                        </select>
                     </div>
                     <div class="player-menu-title">Source</div>
                     <div class="player-menu-scroll">
                        {sortedReleases.map((r) => {
                           const g = releaseGroup(r);
                           return (
                              <button
                                 key={r.magnet}
                                 class={`player-menu-item player-source-item${r.magnet === selectedRelease.magnet ? " selected" : ""}`}
                                 onClick={() => void switchSource(r)}
                              >
                                 <span class="player-source-title">
                                    {r.title}
                                 </span>
                                 <span class="release-badges">
                                    {releaseBadges(r).map((b) => (
                                       <span key={b.label} class={`release-badge release-badge-${b.kind}`}>
                                          {b.label}
                                       </span>
                                    ))}
                                 </span>
                                 <span class="player-menu-meta">
                                    <span class={`seeders seeders-${seederHealth(r.seeders)}`}>
                                       {r.seeders === 0 ? "no seeders" : `${r.seeders} seeders`}
                                    </span>{" "}
                                    · {r.size}
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
                     video={videoEl}
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
               class={`player-seek-wrap${episodeDuration ? " seekable" : ""}`}
               role="slider"
               aria-label="Seek"
               aria-valuemin={0}
               aria-valuemax={Math.round(episodeDuration)}
               aria-valuenow={Math.round(displayTime)}
               onPointerDown={handleSeekPointerDown}
               onPointerMove={handleSeekPointerMove}
               onPointerUp={handleSeekPointerUp}
               onPointerCancel={handleSeekPointerCancel}
               onMouseLeave={hideSeekTooltip}
            >
               <div class="player-seek-track" />
               {episodeDuration > 0 &&
                  downloadedRanges.map(([start, end]) => (
                     <div
                        key={`d${start}`}
                        class="player-seek-ready"
                        style={{
                           left: `${(start / episodeDuration) * 100}%`,
                           width: `${((end - start) / episodeDuration) * 100}%`,
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
               {episodeDuration > 0 && loopA != null && (
                  <div
                     class="player-seek-loop"
                     style={{
                        left: `${(loopA / episodeDuration) * 100}%`,
                        width: `${(((loopB ?? loopA) - loopA) / episodeDuration) * 100}%`,
                     }}
                  />
               )}
               <div class="player-seek-played" ref={playedFillRef} />
               {episodeDuration > 0 &&
                  chapters
                     .filter((c) => c.time > 0.5 && c.time < episodeDuration - 0.5)
                     .map((c) => (
                        <div
                           key={`c${c.time}`}
                           class="player-seek-chapter"
                           style={{ left: `${(c.time / episodeDuration) * 100}%` }}
                        />
                     ))}
               <div class="player-seek-thumb" ref={seekThumbRef} />
               <div class="player-seek-tooltip" ref={seekTooltipRef} />
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
                  class={`player-speed${loopA != null ? " on" : ""}`}
                  onClick={stepAbLoop}
                  aria-label="A-B loop"
                  title="A-B loop (A: set A, set B, clear)"
               >
                  A–B
               </button>
               {loopA != null && loopB != null && (
                  <button
                     class="player-speed on"
                     onClick={exportLoop}
                     disabled={exporting}
                     aria-label="Export looped clip"
                     title="Export the looped section as MP4… (E)"
                  >
                     {exporting ? "Exporting…" : "Clip"}
                  </button>
               )}
               <button
                  class={`player-speed${speed !== 1 ? " on" : ""}`}
                  onClick={() => changeSpeed(speed >= SPEEDS[SPEEDS.length - 1] ? 0 : 1)}
                  onContextMenu={(e) => {
                     e.preventDefault();
                     changeSpeed(0);
                  }}
                  aria-label="Playback speed"
                  title="Playback speed ([ ] step, \ reset)"
               >
                  {speed}×
               </button>
               <button
                  data-menu-toggle
                  class={`player-icon-button${menu === "subtitles" ? " active" : ""}${activeSubtitleIndex != null ? " on" : ""}`}
                  onClick={() => {
                     setPlaylistOpen(false);
                     setMenu((m) => (m === "subtitles" ? null : "subtitles"));
                  }}
                  aria-label="Subtitles"
                  title="Subtitles (C toggle, C + scroll switch track, -/+ delay)"
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

// Asked once per app run (and again when the mpv path setting changes).
let mpvAvailable: Promise<boolean> | null = null;
let mpvAvailablePath: string | null = null;

/** The player: embedded mpv when it's installed, otherwise the HLS
 * fallback (`HlsPlayerView`). */
export function PlayerView(props: Props) {
   const [useMpv, setUseMpv] = useState<boolean | null>(null);
   useEffect(() => {
      // A failed check (an older backend without the command) isn't
      // evidence mpv is missing - mpv stays the default.
      const path = getSettingsSnapshot().mpvPath;
      if (mpvAvailable == null || mpvAvailablePath !== path) {
         mpvAvailablePath = path;
         mpvAvailable = invoke("set_mpv_path", { path: path || null })
            .then(() => invoke<boolean>("mpv_available"))
            .catch((err) => {
               console.warn("[player] mpv availability check failed, assuming mpv", { err: String(err) });
               return true;
            });
      }
      void mpvAvailable!.then((available) => {
         console.info("[player] backend", { mpv: available });
         setUseMpv(available);
      });
   }, []);
   // hls.js + JASSUB (~450 kB) only load when mpv is missing.
   const [Hls, setHls] = useState<typeof import("./HlsPlayerView").HlsPlayerView | null>(null);
   useEffect(() => {
      if (useMpv !== false || Hls) return;
      console.debug("[player] loading HLS fallback chunk");
      void import("./HlsPlayerView").then((m) => setHls(() => m.HlsPlayerView));
   }, [useMpv, Hls]);
   if (useMpv == null || (useMpv === false && !Hls)) return <div class="player-view hls-player" />;
   return useMpv ? <MpvPlayerView {...props} /> : Hls && <Hls {...props} />;
}
