import { useEffect, useRef } from "preact/hooks";
import JASSUB from "jassub";
// Explicit asset URLs rather than JASSUB's own `new URL(..., import.meta.url)`
// defaults: those point into Vite's pre-bundled deps directory in dev,
// where the worker/wasm files don't exist.
import workerUrl from "jassub/dist/worker/worker.js?worker&url";
import wasmUrl from "jassub/dist/wasm/jassub-worker.wasm?url";
import modernWasmUrl from "jassub/dist/wasm/jassub-worker-modern.wasm?url";
import defaultFontUrl from "jassub/dist/default.woff2?url";
import type { SubtitleStyle } from "./settings";
import { applySubtitleStyle } from "./subtitles";

/** How often the growing merged ASS script is re-fetched - the backend
 * appends events as the HLS transcode advances (see torrent-engine's
 * `subtitle_handler`), usually well ahead of the playhead. */
const POLL_MS = 3000;

// Placeholder track the renderer starts with, so the worker, WASM and
// embedded fonts all initialize the moment a subtitle track is selected
// instead of on the first successful fetch - by the time the first line
// is due, libass is warm.
const EMPTY_SCRIPT = [
  "[Script Info]",
  "ScriptType: v4.00+",
  "PlayResX: 384",
  "PlayResY: 288",
  "",
  "[Events]",
  "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text",
  "",
].join("\n");

// libass rasterizes at canvas resolution; beyond 1440p (4K fullscreen)
// the extra cost buys nothing visible for subtitles.
const MAX_RENDER_HEIGHT = 1440;

interface Options {
  video: HTMLVideoElement | null;
  /** Merged ASS script URL for the active track; null = subtitles off. */
  url: string | null;
  fonts: string[];
  /** Whether the active track carries real ASS styling. */
  styled: boolean;
  style: SubtitleStyle;
  /** Source-timeline seconds at media time 0 (hls.js initPTS) - see
   * PlayerView's INIT_PTS_FOUND handler. */
  timeOffset: number;
  /** Subtitle delay the user set with the player's sync keys (seconds,
   * positive = later). */
  delay: number;
}

/**
 * Renders the active subtitle track with libass (JASSUB) over `video`,
 * polling the backend's merged script and swapping it in whenever it
 * grows. Replaces the old `<track>` element, which fetched its WebVTT file
 * exactly once and so only ever showed what had been extracted at that
 * moment.
 */
export function useAssRenderer({ video, url, fonts, styled, style, timeOffset, delay }: Options): void {
  const instanceRef = useRef<JASSUB | null>(null);
  const rawRef = useRef<string>("");
  const styleRef = useRef({ style, styled });
  styleRef.current = { style, styled };
  const offsetRef = useRef(timeOffset - delay);
  offsetRef.current = timeOffset - delay;

  // Instance lifetime: one per (video, track URL, fonts). A new track gets
  // a fresh renderer rather than setTrack on the old one, so fonts from a
  // previous release never linger.
  useEffect(() => {
    if (!video || !url) return;
    let cancelled = false;
    let lastLength = -1;
    rawRef.current = "";
    console.info("[subtitles] starting renderer", { url, fonts: fonts.length });
    const instance = new JASSUB({
      video,
      subContent: EMPTY_SCRIPT,
      workerUrl,
      wasmUrl,
      modernWasmUrl,
      fonts,
      availableFonts: { "liberation sans": defaultFontUrl },
      queryFonts: "local",
      timeOffset: offsetRef.current,
      maxRenderHeight: MAX_RENDER_HEIGHT,
    });
    instanceRef.current = instance;
    const started = performance.now();
    instance.ready.then(
      () => console.info("[subtitles] renderer warm", { ms: Math.round(performance.now() - started) }),
      (err) => console.error("[subtitles] renderer failed to start", { err: String(err) }),
    );

    async function poll() {
      if (cancelled) return;
      try {
        const response = await fetch(url as string, { cache: "no-store" });
        if (response.status === 404) {
          console.debug("[subtitles] track not extracted yet");
          return;
        }
        if (!response.ok) {
          console.warn("[subtitles] fetch failed", { status: response.status });
          return;
        }
        const raw = await response.text();
        if (cancelled || raw.length === lastLength) return;
        lastLength = raw.length;
        rawRef.current = raw;
        const content = applySubtitleStyle(raw, styleRef.current.style, styleRef.current.styled);
        await instance.ready;
        if (cancelled) return;
        await instance.renderer.setTrack(content);
        console.debug("[subtitles] track updated", { bytes: raw.length });
      } catch (err) {
        console.error("[subtitles] poll/render failed", { err: String(err) });
      }
    }

    void poll();
    const timer = window.setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      instanceRef.current = null;
      console.debug("[subtitles] destroying renderer");
      void instance.destroy();
    };
    // fonts is a fresh array per probe; join keeps this keyed by content.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [video, url, fonts.join("|")]);

  // Style edits (settings panel) re-render the current script in place.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance || !rawRef.current) return;
    void instance.renderer.setTrack(applySubtitleStyle(rawRef.current, style, styled));
  }, [style, styled]);

  // JASSUB draws at mediaTime + timeOffset; a positive user delay shows
  // cues later, i.e. looks up an earlier source time.
  useEffect(() => {
    const instance = instanceRef.current;
    if (instance) instance.timeOffset = offsetRef.current;
  });
}
