import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import type { PlaySession, StreamStats, SubtitleInfo } from "./types";

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
// container header hasn't downloaded yet - callers retry.
export async function getSubtitleTracks(torrentId: string, fileIdx: number): Promise<SubtitleInfo> {
  if (!isTauriAvailable()) return { tracks: [], fonts: [] };
  return invoke<SubtitleInfo>("get_subtitle_tracks", { torrentId, fileIdx });
}

// Tells the streaming server what this WebView decodes natively, so it
// only transcodes what it must (see torrent-engine's plan_video). Probed
// with MSE, the same path hls.js feeds.
let decoderSupportSent = false;
export async function reportDecoderSupport(): Promise<void> {
  if (!isTauriAvailable() || decoderSupportSent) return;
  const hevc = typeof MediaSource !== "undefined" && MediaSource.isTypeSupported('video/mp4; codecs="hvc1.1.6.L120.90"');
  console.info("[set_decoder_support] invoked", { hevc });
  await invoke("set_decoder_support", { support: { hevc } });
  decoderSupportSent = true;
}

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

export async function stopPlayback(resume?: ResumeRequest): Promise<void> {
  if (!isTauriAvailable()) return;
  console.debug("[stop_playback] invoked", { resume });
  await invoke("stop_playback", { resume: resume ?? null });
  console.info("[stop_playback] succeeded");
}
