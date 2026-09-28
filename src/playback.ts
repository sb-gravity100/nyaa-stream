import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";
import type { PlaySession, StreamStats, SubtitleInfo } from "./types";

// Torrenting isn't available in the dev-only browser preview (see
// browserFallback.ts) - there's no meaningful fallback for actual playback,
// so these just reject/no-op there rather than pretending to stream.

export async function playMagnet(magnet: string, title: string): Promise<PlaySession> {
  if (!isTauriAvailable()) throw new Error("Playback requires the Tauri app, not the browser preview");
  console.debug("[play_magnet] invoked", { title });
  const session = await invoke<PlaySession>("play_magnet", { magnet, title });
  console.info("[play_magnet] succeeded", { title, torrentId: session.torrentId });
  return session;
}

export async function getStreamStats(torrentId: string): Promise<StreamStats> {
  return invoke<StreamStats>("get_stream_stats", { torrentId });
}

// No browser-fallback path (see this file's top comment): a torrent-less
// dev preview has no subtitle tracks to list either. Rejects while the
// container header hasn't downloaded yet - callers retry.
export async function getSubtitleTracks(torrentId: string): Promise<SubtitleInfo> {
  if (!isTauriAvailable()) return { tracks: [], fonts: [] };
  return invoke<SubtitleInfo>("get_subtitle_tracks", { torrentId });
}

export async function stopPlayback(): Promise<void> {
  if (!isTauriAvailable()) return;
  console.debug("[stop_playback] invoked");
  await invoke("stop_playback");
  console.info("[stop_playback] succeeded");
}
