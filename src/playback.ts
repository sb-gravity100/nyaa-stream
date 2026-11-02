import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import type { PlaySession, StreamStats } from "./types";

type EpisodeTransition = { animeId: number; targetKey: string; startedAt: number; trigger: "manual" | "autoplay" | "playlist" };
let pendingEpisodeTransition: EpisodeTransition | null = null;

export function markEpisodeTransition(animeId: number, targetKey: string, trigger: EpisodeTransition["trigger"]): void {
  pendingEpisodeTransition = { animeId, targetKey, startedAt: performance.now(), trigger };
  console.info("[player] next-episode navigation started", { animeId, targetKey, trigger });
}

export function takeEpisodeTransition(animeId: number, targetKey: string): EpisodeTransition | null {
  const pending = pendingEpisodeTransition;
  if (!pending || pending.animeId !== animeId || pending.targetKey !== targetKey) return null;
  pendingEpisodeTransition = null;
  if (performance.now() - pending.startedAt > 60_000) {
    console.debug("[player] expired next-episode timing marker", { animeId, targetKey });
    return null;
  }
  console.debug("[player] matched next-episode timing marker", { animeId, targetKey, trigger: pending.trigger });
  return pending;
}

// Torrenting isn't available in the dev-only browser preview (see
// browserFallback.ts) - there's no meaningful fallback for actual playback,
// so these just reject/no-op there rather than pretending to stream.

/** `episode` (`<animeId>:<episodeKey>`) ties the torrent's cached files to
 * a Continue watching entry - see PLAN.md "Download cache". `trace` is the
 * load profiler's start trace (see loadTrace.ts). */
export async function playMagnet(magnet: string, title: string, episode: string, trace: number | null = null): Promise<PlaySession> {
  if (!isTauriAvailable()) throw new Error("Playback requires the Tauri app, not the browser preview");
  console.debug("[play_magnet] invoked", { title, episode, trace });
  const session = await invoke<PlaySession>("play_magnet", { magnet, title, episode, trace });
  console.info("[play_magnet] succeeded", { title, torrentId: session.torrentId, files: session.files.length });
  return session;
}

export async function getStreamStats(torrentId: string, fileIdx: number): Promise<StreamStats> {
  return invoke<StreamStats>("get_stream_stats", { torrentId, fileIdx });
}

// No browser-fallback path (see this file's top comment): a torrent-less
// dev preview has no subtitle tracks to list either. Rejects while the
/** The player opened `fileIdx` (mpv's file-loaded) - ends the backend's
 * open-read recording for the resume buffer. Best effort. */
export async function streamFileLoaded(torrentId: string, fileIdx: number): Promise<void> {
  if (!isTauriAvailable()) return;
  console.debug("[stream_file_loaded] invoked", { torrentId, fileIdx });
  await invoke("stream_file_loaded", { torrentId, fileIdx }).catch((err) =>
    console.warn("[stream_file_loaded] failed", { err: String(err) }),
  );
}

/** Sent with stop_playback for an episode still in progress: the backend
 * saves its resume buffer first (PLAN.md "Continue-watching resume buffer"). */
export interface ResumeRequest {
  animeId: number;
  episodeKey: string;
  fileIdx: number;
  /** Seconds, file time. */
  position: number;
  magnet: string;
}

export async function stopPlayback(resume?: ResumeRequest, nextEpisode = false, playbackGeneration?: number): Promise<void> {
  if (!isTauriAvailable()) return;
  console.debug("[stop_playback] invoked", { hasResume: resume != null, fileIdx: resume?.fileIdx, nextEpisode, playbackGeneration });
  await invoke("stop_playback", { resume: resume ?? null, nextEpisode, playbackGeneration: playbackGeneration ?? null });
  console.info("[stop_playback] succeeded");
}
