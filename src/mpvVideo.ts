import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SubtitleTrack } from "./types";

/** One entry of mpv's `track-list` property. */
interface MpvTrack {
   id: number;
   type: "video" | "audio" | "sub";
   title?: string;
   lang?: string;
   codec?: string;
   default?: boolean;
   selected?: boolean;
}

interface MpvEvent {
   event: string;
   id?: number;
   name?: string;
   data?: unknown;
   reason?: string;
   file_error?: string;
}

/** Properties mirrored from mpv (observe ids are their index + 1). */
const OBSERVED = [
   "time-pos",
   "duration",
   "pause",
   "paused-for-cache",
   "eof-reached",
   "volume",
   "mute",
   "demuxer-cache-state",
   "video-params",
   "track-list",
   "seeking",
] as const;
type Observed = (typeof OBSERVED)[number];

function command(args: unknown[]): Promise<unknown> {
   return invoke("mpv_command", { args });
}

/**
 * The embedded mpv player behind a small `HTMLVideoElement`-shaped facade
 * (`currentTime`, `paused`, `buffered`, `play()`, DOM-style events...) so
 * the player UI can drive it the way it drove `<video>`. State arrives as
 * mpv `property-change` events forwarded by the backend (`mpv-event`);
 * `currentTime` is interpolated between them so the seek bar moves every
 * frame.
 */
export class MpvVideo extends EventTarget {
   private props: Partial<Record<Observed, unknown>> = {};
   private timeStamp = 0;
   private loaded = false;
   private started = false;
   private unlisten: UnlistenFn[] = [];
   private disposed = false;
   error: { message: string } | null = null;

   /** Starts mpv (idempotent), subscribes to its events and observes the
    * properties the facade mirrors. */
   async attach(): Promise<void> {
      console.debug("[mpv] attaching");
      this.unlisten.push(
         await listen<MpvEvent>("mpv-event", (e) => this.handleEvent(e.payload)),
         await listen("mpv-exit", () => this.fail("The player process exited.")),
      );
      if (this.disposed) return this.detach();
      await invoke("mpv_start");
      await Promise.all(
         OBSERVED.map((name, i) => command(["observe_property", i + 1, name])),
      );
      console.info("[mpv] attached");
   }

   /** Opens `url`, starting at `start` seconds when given. */
   async load(url: string, start: number | null): Promise<void> {
      console.info("[mpv] loading", { url, start });
      this.loaded = false;
      this.started = false;
      this.error = null;
      this.props = {};
      await command(["set_property", "start", start != null && start > 0 ? String(start) : "none"]);
      await command(["set_property", "pause", false]);
      await command(["loadfile", url, "replace"]);
   }

   /** Unloads the file and makes the window opaque again. */
   async detach(): Promise<void> {
      if (this.disposed && this.unlisten.length === 0) return;
      this.disposed = true;
      for (const unlisten of this.unlisten) unlisten();
      this.unlisten = [];
      console.debug("[mpv] detaching");
      await invoke("mpv_stop").catch((err) =>
         console.warn("[mpv] stop failed", { err: String(err) }),
      );
   }

   get currentTime(): number {
      const base = typeof this.props["time-pos"] === "number" ? (this.props["time-pos"] as number) : 0;
      if (!this.playing) return base;
      const elapsed = (performance.now() - this.timeStamp) / 1000;
      // A frame or so of extrapolation at most - enough to smooth the bar
      // between updates without running ahead of a stall.
      return Math.min(base + Math.min(elapsed, 0.25), this.duration || Infinity);
   }

   set currentTime(seconds: number) {
      console.debug("[mpv] seek", { seconds });
      this.props["time-pos"] = seconds;
      this.timeStamp = performance.now();
      void command(["seek", seconds, "absolute"]).catch((err) =>
         console.warn("[mpv] seek failed", { seconds, err: String(err) }),
      );
   }

   get duration(): number {
      const d = this.props.duration;
      return typeof d === "number" && Number.isFinite(d) ? d : 0;
   }

   get paused(): boolean {
      return this.props.pause !== false;
   }

   get muted(): boolean {
      return this.props.mute === true;
   }

   set muted(value: boolean) {
      void command(["set_property", "mute", value]);
   }

   /** 0-1 like `<video>`; mpv's own scale is 0-100. */
   get volume(): number {
      return typeof this.props.volume === "number" ? (this.props.volume as number) / 100 : 1;
   }

   set volume(value: number) {
      void command(["set_property", "volume", Math.round(value * 100)]);
   }

   /** 0 = nothing, 1 = metadata, 4 = playing through (`<video>` scale). */
   get readyState(): number {
      if (!this.loaded) return 0;
      return this.started ? 4 : 1;
   }

   get videoWidth(): number {
      return this.videoParam("dw") ?? this.videoParam("w") ?? 0;
   }

   get videoHeight(): number {
      return this.videoParam("dh") ?? this.videoParam("h") ?? 0;
   }

   /** Seekable (already demuxed) ranges, like `<video>.buffered`. */
   get buffered(): { length: number; start(i: number): number; end(i: number): number } {
      const state = this.props["demuxer-cache-state"] as
         | { "seekable-ranges"?: { start: number; end: number }[] }
         | undefined;
      const ranges = state?.["seekable-ranges"] ?? [];
      return {
         length: ranges.length,
         start: (i) => ranges[i].start,
         end: (i) => ranges[i].end,
      };
   }

   /** Subtitle tracks from mpv's `track-list`, `index` = mpv track id. */
   get subtitleTracks(): SubtitleTrack[] {
      const tracks = (this.props["track-list"] as MpvTrack[] | undefined) ?? [];
      return tracks
         .filter((t) => t.type === "sub")
         .map((t) => ({
            index: t.id,
            language: t.lang ?? null,
            title: t.title ?? null,
            codec: t.codec ?? "",
            default: t.default === true,
            url: "",
         }));
   }

   async play(): Promise<void> {
      await command(["set_property", "pause", false]);
   }

   pause(): void {
      void command(["set_property", "pause", true]);
   }

   /** Selects a subtitle track by mpv id, or turns subtitles off. */
   setSubtitle(id: number | null): void {
      console.debug("[mpv] subtitle track", { id });
      void command(["set_property", "sid", id ?? "no"]).catch((err) =>
         console.warn("[mpv] sid failed", { id, err: String(err) }),
      );
   }

   setProperty(name: string, value: unknown): Promise<unknown> {
      return command(["set_property", name, value]).catch((err) => {
         console.warn("[mpv] set_property failed", { name, value, err: String(err) });
      });
   }

   getProperty(name: string): Promise<unknown> {
      return command(["get_property", name]);
   }

   private get playing(): boolean {
      return (
         this.started &&
         this.props.pause === false &&
         this.props["paused-for-cache"] !== true &&
         this.props.seeking !== true
      );
   }

   private videoParam(key: string): number | null {
      const params = this.props["video-params"] as Record<string, unknown> | undefined;
      const v = params?.[key];
      return typeof v === "number" ? v : null;
   }

   private emit(type: string) {
      this.dispatchEvent(new Event(type));
   }

   private fail(message: string) {
      console.error("[mpv] playback error", { message });
      this.error = { message };
      this.emit("error");
   }

   private handleEvent(e: MpvEvent) {
      switch (e.event) {
         case "property-change":
            if (e.name) this.handleProperty(e.name as Observed, e.data);
            return;
         case "file-loaded":
            console.info("[mpv] file loaded");
            this.loaded = true;
            this.emit("loadedmetadata");
            return;
         case "playback-restart":
            // First frame after a load or a seek.
            if (!this.started) console.info("[mpv] first frame");
            this.started = true;
            this.timeStamp = performance.now();
            this.emit("canplay");
            this.emit("playing");
            this.emit("seeked");
            return;
         case "end-file":
            if (e.reason === "error") this.fail(e.file_error ?? "unknown error");
            return;
         default:
            return;
      }
   }

   private handleProperty(name: Observed, data: unknown) {
      const previous = this.props[name];
      this.props[name] = data;
      switch (name) {
         case "time-pos":
            this.timeStamp = performance.now();
            this.emit("timeupdate");
            return;
         case "duration":
            this.emit("durationchange");
            return;
         case "pause":
            if (previous !== data) this.emit(data === false ? "play" : "pause");
            return;
         case "paused-for-cache":
            if (data === true) {
               console.debug("[mpv] buffering");
               this.emit("waiting");
            } else if (previous === true) {
               this.timeStamp = performance.now();
               this.emit("playing");
            }
            return;
         case "eof-reached":
            if (data === true) {
               console.info("[mpv] ended");
               this.emit("ended");
            }
            return;
         case "volume":
         case "mute":
            this.emit("volumechange");
            return;
         case "demuxer-cache-state":
            this.emit("progress");
            return;
         case "track-list":
            this.emit("tracks");
            return;
         default:
            return;
      }
   }
}
