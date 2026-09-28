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
import type { ASSStyle } from "jassub/dist/worker/util";
import { applySubtitleStyle, playResY } from "./subtitles";

/** How often new events are fetched - the backend appends them as the HLS
 * transcode advances (see torrent-engine's `subtitle_handler`), usually well
 * ahead of the playhead. Only the delta is fetched and fed to libass
 * (`processData`): re-fetching and re-parsing the whole growing script each
 * poll never finished on heavily typeset releases (a 40 MB, ~77k-event
 * Kaleido-subs script rendered nothing). */
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

// libass rasterizes at the canvas's backing resolution. Rendering at
// exactly the displayed size leaves glyph edges with one pixel of coverage
// each, which reads as jagged on large text; supersampling renders the
// layer at N x the displayed size and lets the browser downsample it into
// the same box (a 2x2 box average at 2x - real SSAA). Capped so 4K
// fullscreen doesn't ask libass for an 8K canvas.
const MAX_RENDER_HEIGHT_BY_SUPERSAMPLE: Record<number, number> = { 1: 1440, 2: 2160 };

// Numpad alignments 1-3 are bottom-aligned - identical in libass's internal
// encoding, so this holds whichever form getStyles returns.
const BOTTOM_ALIGNMENTS = new Set([1, 2, 3]);
// How many libass re-layouts the dock-lift animation takes (each is one
// setStyle round trip to the worker).
const LIFT_STEPS = 6;
const LIFT_STEP_MS = 30;

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
  /** Screen pixels at the bottom of the video currently covered by the
   * player's control dock (0 when hidden). Bottom-aligned lines whose
   * margin is smaller get pushed up just enough to clear it. */
  bottomInsetPx: number;
  /** Render-resolution multiplier for anti-aliasing (1 or 2). */
  supersample: number;
}

/**
 * Renders the active subtitle track with libass (JASSUB) over `video`,
 * polling the backend's merged script and swapping it in whenever it
 * grows. Replaces the old `<track>` element, which fetched its WebVTT file
 * exactly once and so only ever showed what had been extracted at that
 * moment.
 */
export function useAssRenderer({ video, url, fonts, styled, style, timeOffset, delay, bottomInsetPx, supersample }: Options): void {
  const instanceRef = useRef<JASSUB | null>(null);
  const rawRef = useRef<string>("");
  // Styles as the current script defines them (before any dock lift), and
  // the lift currently applied, in script (PlayResY) units.
  const baseStylesRef = useRef<ASSStyle[] | null>(null);
  const appliedLiftRef = useRef(0);
  const insetRef = useRef(bottomInsetPx);
  insetRef.current = bottomInsetPx;
  const supersampleRef = useRef(supersample);
  supersampleRef.current = supersample;
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
    // Events already loaded (the backend's X-Subtitle-Events cursor).
    let loadedEvents = 0;
    let loaded = false;
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
      prescaleFactor: supersampleRef.current,
      prescaleHeightLimit: 4320,
      maxRenderHeight: MAX_RENDER_HEIGHT_BY_SUPERSAMPLE[supersampleRef.current] ?? 1440,
    });
    instanceRef.current = instance;
    const started = performance.now();
    instance.ready.then(
      () => console.info("[subtitles] renderer warm", { ms: Math.round(performance.now() - started) }),
      (err) => console.error("[subtitles] renderer failed to start", { err: String(err) }),
    );

    // A large first load can outlast the poll interval; overlapping polls
    // would append the same events twice.
    let inFlight = false;
    async function poll() {
      if (cancelled || inFlight) return;
      inFlight = true;
      try {
        const response = await fetch(`${url}${(url as string).includes("?") ? "&" : "?"}from=${loadedEvents}`, { cache: "no-store" });
        if (response.status === 404) {
          console.debug("[subtitles] track not extracted yet");
          return;
        }
        if (!response.ok) {
          console.warn("[subtitles] fetch failed", { status: response.status });
          return;
        }
        const total = Number(response.headers.get("x-subtitle-events") ?? "0");
        const text = await response.text();
        if (cancelled) return;
        await instance.ready;
        if (cancelled) return;
        if (!loaded) {
          // First response: the whole script so far, header included.
          rawRef.current = text;
          await instance.renderer.setTrack(applySubtitleStyle(text, styleRef.current.style, styleRef.current.styled));
          // A new script resets styles: re-read the originals and re-apply
          // the current dock lift on top of them.
          await captureBaseStyles(instance);
          await applyLift(instance, liftTarget(), 1);
          loaded = true;
          console.info("[subtitles] track loaded", { events: total, bytes: text.length });
        } else if (text.length > 0) {
          // Just the new Dialogue lines - appended, no re-parse.
          rawRef.current += text;
          await instance.renderer.processData(text);
          console.debug("[subtitles] events appended", { added: total - loadedEvents, total });
        }
        loadedEvents = total;
      } catch (err) {
        console.error("[subtitles] poll/render failed", { err: String(err) });
      } finally {
        inFlight = false;
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

  async function captureBaseStyles(instance: JASSUB) {
    baseStylesRef.current = (await instance.renderer.getStyles()) as ASSStyle[];
    appliedLiftRef.current = 0;
  }

  /** The dock inset converted to script units (libass lays out bottom
   * margins in PlayResY units, scaled to the displayed video height). */
  function liftTarget(): number {
    const inset = insetRef.current;
    const shownHeight = video?.getBoundingClientRect().height ?? 0;
    if (inset <= 0 || shownHeight <= 0 || !rawRef.current) return 0;
    return (inset / shownHeight) * playResY(rawRef.current);
  }

  /** Raises each bottom-aligned style's MarginV to at least `lift` (script
   * units), animating over `steps` re-layouts. Styles already sitting that
   * high are left alone - only lines the dock actually covers move - and
   * positioned signs (\pos, \move) ignore margins entirely. */
  async function applyLift(instance: JASSUB, lift: number, steps: number) {
    const base = baseStylesRef.current;
    if (!base) return;
    const from = appliedLiftRef.current;
    appliedLiftRef.current = lift;
    for (let step = 1; step <= steps; step++) {
      // Ease-out between the previous and new lift.
      const t = 1 - Math.pow(1 - step / steps, 3);
      const current = from + (lift - from) * t;
      await Promise.all(
        base.map((original, index) => {
          if (!BOTTOM_ALIGNMENTS.has(original.Alignment)) return undefined;
          const margin = Math.max(original.MarginV, Math.round(current));
          return instance.renderer.setStyle({ ...original, MarginV: margin }, index);
        }),
      );
      if (instanceRef.current !== instance) return;
      if (steps > 1) await new Promise((resolve) => setTimeout(resolve, LIFT_STEP_MS));
    }
    // Repaint even while paused (JASSUB otherwise waits for the next frame).
    await instance.resize(true);
  }

  // Style edits (settings panel) re-render the current script in place.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance || !rawRef.current) return;
    void (async () => {
      await instance.renderer.setTrack(applySubtitleStyle(rawRef.current, style, styled));
      await captureBaseStyles(instance);
      await applyLift(instance, liftTarget(), 1);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [style, styled]);

  // Dock shown/hidden: slide covered lines up or back down.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance || !baseStylesRef.current) return;
    const target = liftTarget();
    if (Math.abs(target - appliedLiftRef.current) < 0.5) return;
    void applyLift(instance, target, LIFT_STEPS).catch((err) => console.debug("[subtitles] lift failed", { err: String(err) }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bottomInsetPx]);

  // Anti-aliasing level change: new backing resolution, same layout.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance) return;
    instance.prescaleFactor = supersample;
    instance.maxRenderHeight = MAX_RENDER_HEIGHT_BY_SUPERSAMPLE[supersample] ?? 1440;
    console.info("[subtitles] supersampling", { factor: supersample });
    void instance.resize(true);
  }, [supersample]);

  // JASSUB draws at mediaTime + timeOffset; a positive user delay shows
  // cues later, i.e. looks up an earlier source time.
  useEffect(() => {
    const instance = instanceRef.current;
    if (instance) instance.timeOffset = offsetRef.current;
  });
}
