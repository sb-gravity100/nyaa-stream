import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import type { PlaySession, StreamStats, SubtitleInfo } from "./types";

// Torrenting isn't available in the dev-only browser preview (see
// browserFallback.ts) - there's no meaningful fallback for actual playback,
// so these just reject/no-op there rather than pretending to stream.

/** How the file starts - drives the engine's download order (see PLAN.md
 * "Fast playback start"): "first" = from 0:00, "resume" = a start time
 * follows (Continue watching / Resume, or a source switch mid-episode). */
export type WatchHint = "first" | "resume";

export async function playMagnet(magnet: string, title: string, watch: WatchHint): Promise<PlaySession> {
  if (!isTauriAvailable()) throw new Error("Playback requires the Tauri app, not the browser preview");
  console.debug("[play_magnet] invoked", { title, watch });
  const session = await invoke<PlaySession>("play_magnet", { magnet, title, watch });
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

export async function stopPlayback(): Promise<void> {
  if (!isTauriAvailable()) return;
  console.debug("[stop_playback] invoked");
  await invoke("stop_playback");
  console.info("[stop_playback] succeeded");
}
