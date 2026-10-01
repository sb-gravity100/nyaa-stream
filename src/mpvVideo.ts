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

/** One entry of mpv's `chapter-list` property. */
export interface Chapter {
   title: string;
   time: number;
}

interface MpvEvent {
   event: string;
   id?: number;
   name?: string;
   data?: unknown;
   reason?: string;
   file_error?: string;
   /** mpv's playlist entry of a `start-file`/`end-file`. */
   playlist_entry_id?: number;
   /** `stream-corrupt`: the mpv log line's module and text. */
   prefix?: string;
   text?: string;
}

/** Buffer rebuilds on corruption: at most one per REBUILD_MIN_GAP_MS, and
 * none after REBUILD_LIMIT within REBUILD_WINDOW_MS (a spot that stays bad
 * isn't retried forever). */
const REBUILD_MIN_GAP_MS = 5_000;
const REBUILD_WINDOW_MS = 60_000;
const REBUILD_LIMIT = 3;

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
   "speed",
   "chapter-list",
   // The active subtitle track's ASS header: unavailable until the track is
   // actually loaded, which is after `sid` is set - the restyle waits for it.
   "sub-ass-extradata",
] as const;
type Observed = (typeof OBSERVED)[number];

/** Properties that belong to the loaded file (reset on `load`). */
const FILE_PROPERTIES: Observed[] = ["time-pos", "duration", "eof-reached", "demuxer-cache-state", "video-params", "track-list", "seeking", "chapter-list", "sub-ass-extradata"];

// `<video>` fires timeupdate ~4x a second; the seek bar's own rAF loop
// reads `currentTime` for smoothness. Re-rendering the player on every
// mpv frame made the controls sluggish and fought seek bar drags.
const TIMEUPDATE_INTERVAL_MS = 250;
// A seek still waiting on torrent data after this long lets a newer
// requested seek go out (the position stays masked until mpv restarts
// playback, however long the data takes).
const SEEK_RELEASE_MS = 2000;

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
/** mpv `slang` for a settings language code: matched as given and as its
 * ISO 639-2 form, which many releases tag tracks with. */
function subtitleLanguages(language: string): string {
   const ISO_639_2: Record<string, string> = {
      en: "eng", ja: "jpn", es: "spa", fr: "fre,fra", de: "ger,deu", pt: "por", it: "ita", ru: "rus",
      ar: "ara", zh: "chi,zho", ko: "kor", id: "ind", pl: "pol", tr: "tur", vi: "vie", th: "tha",
   };
   const lang = language.toLowerCase();
   return [lang, ISO_639_2[lang]].filter(Boolean).join(",");
}

export class MpvVideo extends EventTarget {
   private props: Partial<Record<Observed, unknown>> = {};
   private timeStamp = 0;
   private loaded = false;
   private started = false;
   private unlisten: UnlistenFn[] = [];
   private disposed = false;
   private lastTimeUpdate = 0;
   /** Target of the seek mpv is working on - newer ones wait for it. */
   private seekTarget: number | null = null;
   /** When recent corruption rebuilds ran (see `rebuildBuffer`). */
   private rebuilds: number[] = [];
   private pendingSeek: number | null = null;
   private seekReleaseTimer: number | undefined;
   /** Positions dropped during the current seek (debug logging). */
   private maskedPositions = 0;
   // The playlist entry of the current `load` (mpv's ids only grow): an
   // `end-file` for any other entry belongs to a replaced file - e.g. a
   // source switch's old torrent failing to open a second later - and must
   // not surface as this file's error. `staleUpTo` covers the gap before the
   // new entry's id is known.
   private entryId: number | null = null;
   private staleUpTo = 0;
   /** Re-asserts the subtitle track once after the first frame (see
    * `checkSubtitlesShowing`). */
   private subtitleCheckTimer: number | undefined;
   private subtitleVisible = true;
   private maxSeenEntryId = 0;
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

   /** Opens `url`, starting at `start` seconds when given. `subtitles`
    * preloads a track in that language (`sid=auto` + `slang` before the
    * load), so its packets are demuxed from the first byte instead of after
    * the on-open pick - PLAN.md "Subtitle preloading". */
   async load(url: string, start: number | null, subtitles?: { language: string; visible: boolean }): Promise<void> {
      console.info("[mpv] loading", { url, start, subtitles });
      window.clearTimeout(this.subtitleCheckTimer);
      if (subtitles) {
         this.subtitleVisible = subtitles.visible;
         await command(["set_property", "slang", subtitleLanguages(subtitles.language)]);
         await command(["set_property", "sid", "auto"]);
         await command(["set_property", "sub-visibility", subtitles.visible]);
      }
      this.loaded = false;
      this.started = false;
      this.error = null;
      // Only per-file state: mpv reports a property again only when it
      // changes, so wiping `pause`/`volume`/`mute` here left them unknown
      // for the whole file (every click then read as "paused" -> play).
      for (const name of FILE_PROPERTIES) delete this.props[name];
      this.seekTarget = null;
      this.pendingSeek = null;
      this.rebuilds = [];
      await command(["set_property", "start", start != null && start > 0 ? String(start) : "none"]);
      await command(["set_property", "pause", false]);
      // The previous close froze (muted) mpv - see `freeze`.
      await command(["set_property", "mute", false]);
      this.entryId = null;
      this.staleUpTo = this.maxSeenEntryId;
      const result = await command(["loadfile", url, "replace"]);
      const id = (result as { playlist_entry_id?: unknown } | null)?.playlist_entry_id;
      if (typeof id === "number" && this.entryId == null) {
         this.entryId = id;
         this.maxSeenEntryId = Math.max(this.maxSeenEntryId, id);
      }
      console.debug("[mpv] loadfile issued", { entryId: this.entryId, staleUpTo: this.staleUpTo });
   }

   /** Whether an event's playlist entry is the current load's. Events
    * without an id (older libmpv) are always current. */
   private isCurrentEntry(id: number | undefined): boolean {
      if (id == null) return true;
      this.maxSeenEntryId = Math.max(this.maxSeenEntryId, id);
      return this.entryId != null ? id === this.entryId : id > this.staleUpTo;
   }

   /** Drops the current file - and mpv's demuxer/cache buffers - at once,
    * so nothing of it keeps playing or rendering. A source switch calls
    * this before `play_magnet` removes the old torrent, which cancels that
    * torrent's open instead of letting it fail. */
   async stop(): Promise<void> {
      console.info("[mpv] stop", { entryId: this.entryId });
      window.clearTimeout(this.seekReleaseTimer);
      this.seekTarget = null;
      this.pendingSeek = null;
      await command(["stop"]);
   }

   /** Silences playback at once - the player's close path calls this
    * before its last-frame grab, so closing doesn't keep playing. */
   async freeze(): Promise<void> {
      await Promise.all([command(["set_property", "pause", true]), command(["set_property", "mute", true])]).catch((err) =>
         console.warn("[mpv] freeze failed", { err: String(err) }),
      );
   }

   /** Unloads the file and makes the window opaque again. */
   async detach(): Promise<void> {
      if (this.disposed && this.unlisten.length === 0) return;
      this.disposed = true;
      window.clearTimeout(this.subtitleCheckTimer);
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

   /** Seeks are coalesced: while one is in flight only the latest
    * request is kept, so a drag or held arrow key can't queue dozens of
    * seeks that each wait on torrent pieces. */
   set currentTime(seconds: number) {
      this.props["time-pos"] = seconds;
      this.timeStamp = performance.now();
      this.pendingSeek = seconds;
      this.flushSeek();
   }

   private flushSeek() {
      if (this.seekTarget != null || this.pendingSeek == null) return;
      const seconds = this.pendingSeek;
      this.pendingSeek = null;
      this.seekTarget = seconds;
      console.debug("[mpv] seek", { seconds });
      // Like <video>: a seek waits (possibly long, on undownloaded pieces)
      // until playback-restart fires "playing".
      this.emit("waiting");
      void command(["seek", seconds, "absolute"]).catch((err) => {
         console.warn("[mpv] seek failed", { seconds, err: String(err) });
         this.releaseSeek();
      });
      window.clearTimeout(this.seekReleaseTimer);
      this.seekReleaseTimer = window.setTimeout(() => {
         // Only to unblock a newer request - with none queued, the target
         // stays shown until playback-restart. Releasing it here made the
         // seek bar snap back to the old position while mpv was still
         // waiting on the new position's pieces (seen on a 20:31 click).
         if (this.pendingSeek != null) this.releaseSeek();
      }, SEEK_RELEASE_MS);
   }

   /** Corrupt bytes or video reached mpv (its demuxer resynced or the
    * decoder concealed errors): drop everything mpv has buffered and seek
    * to the current time, so it re-reads from the playhead - the torrent
    * serves those pieces correctly by now. Rate limited. */
   private async rebuildBuffer(e: MpvEvent) {
      const now = performance.now();
      if (!this.loaded) return;
      if (this.seekingTo != null) {
         console.debug("[mpv] corruption during a seek, the seek re-reads anyway", { text: e.text });
         return;
      }
      this.rebuilds = this.rebuilds.filter((at) => now - at < REBUILD_WINDOW_MS);
      const last = this.rebuilds[this.rebuilds.length - 1];
      if (last != null && now - last < REBUILD_MIN_GAP_MS) {
         console.debug("[mpv] corruption right after a rebuild, skipped", { text: e.text });
         return;
      }
      if (this.rebuilds.length >= REBUILD_LIMIT) {
         console.error("[mpv] corruption keeps coming back, not rebuilding again", { rebuilds: this.rebuilds.length, prefix: e.prefix, text: e.text });
         return;
      }
      this.rebuilds.push(now);
      const at = this.currentTime;
      console.warn("[mpv] corruption detected, rebuilding the buffer", { at, prefix: e.prefix, text: e.text });
      try {
         await command(["drop-buffers"]);
      } catch (err) {
         console.warn("[mpv] drop-buffers failed", { err: String(err) });
      }
      this.currentTime = at;
   }

   private releaseSeek() {
      window.clearTimeout(this.seekReleaseTimer);
      this.seekTarget = null;
      this.flushSeek();
   }

   /** Where an unfinished seek is headed, else null. */
   get seekingTo(): number | null {
      return this.pendingSeek ?? this.seekTarget;
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

   /** Audio tracks from mpv's `track-list`, `index` = mpv track id. */
   get audioTracks(): SubtitleTrack[] {
      const tracks = (this.props["track-list"] as MpvTrack[] | undefined) ?? [];
      return tracks
         .filter((t) => t.type === "audio")
         .map((t) => ({
            index: t.id,
            language: t.lang ?? null,
            title: t.title ?? null,
            codec: t.codec ?? "",
            default: t.default === true,
            url: "",
         }));
   }

   /** Id of the audio track mpv is playing, else null. */
   get activeAudioId(): number | null {
      const tracks = (this.props["track-list"] as MpvTrack[] | undefined) ?? [];
      return tracks.find((t) => t.type === "audio" && t.selected)?.id ?? null;
   }

   /** Chapter marks embedded in the file (empty when it has none). */
   get chapters(): Chapter[] {
      const list = this.props["chapter-list"];
      return Array.isArray(list) ? (list as Chapter[]) : [];
   }

   /** Playback speed multiplier (`<video>.playbackRate`). */
   get playbackRate(): number {
      return typeof this.props.speed === "number" ? (this.props.speed as number) : 1;
   }

   set playbackRate(value: number) {
      this.props.speed = value;
      void command(["set_property", "speed", value]);
   }

   setAudio(id: number): void {
      console.debug("[mpv] audio track", { id });
      void command(["set_property", "aid", id]).catch((err) =>
         console.warn("[mpv] aid failed", { id, err: String(err) }),
      );
   }

   /** Steps one video frame forward or back; mpv pauses playback while
    * stepping. */
   frameStep(direction: 1 | -1): void {
      void command([direction > 0 ? "frame-step" : "frame-back-step"]).catch((err) =>
         console.warn("[mpv] frame step failed", { direction, err: String(err) }),
      );
   }

   async play(): Promise<void> {
      await command(["set_property", "pause", false]);
   }

   pause(): void {
      void command(["set_property", "pause", true]);
   }

   /** Selects a subtitle track by mpv id, or hides subtitles (`null`).
    * Hiding keeps `sid` on its track: changing `sid` makes mpv drop the
    * track's packets and re-demux them from earlier in the file - torrent
    * bytes that may not be here yet (the buffering spinner on every EN/off
    * cycle). `sid` only changes for a different track. */
   async setSubtitle(id: number | null): Promise<void> {
      console.debug("[mpv] subtitle track", { id });
      try {
         if (id == null) {
            this.subtitleVisible = false;
            await command(["set_property", "sub-visibility", false]);
            return;
         }
         const current = await command(["get_property", "sid"]).catch(() => null);
         if (current !== id) await command(["set_property", "sid", id]);
         this.subtitleVisible = true;
         await command(["set_property", "sub-visibility", true]);
      } catch (err) {
         console.warn("[mpv] subtitle selection failed", { id, err: String(err) });
      }
   }

   /** Once after the first frame: if a visible track has shown no text for
    * a while, re-select it (off/on) - what a manual toggle used to fix when
    * the track started being demuxed late. */
   private scheduleSubtitleCheck(): void {
      const WINDOW_MS = 10_000;
      const POLL_MS = 1_000;
      const startedAt = performance.now();
      const poll = async () => {
         if (this.disposed || !this.subtitleVisible) return;
         const text = await command(["get_property", "sub-text"]).catch(() => null);
         if (typeof text === "string" && text.trim() !== "") {
            console.debug("[mpv] subtitles showing");
            return;
         }
         if (performance.now() - startedAt < WINDOW_MS) {
            this.subtitleCheckTimer = window.setTimeout(() => void poll(), POLL_MS);
            return;
         }
         const sid = await command(["get_property", "sid"]).catch(() => null);
         if (typeof sid !== "number") return;
         console.info("[mpv] no subtitle text after the first frame, re-asserting the track", { sid });
         await command(["set_property", "sid", "no"]).catch(() => undefined);
         await command(["set_property", "sid", sid]).catch((err) => console.warn("[mpv] sid re-assert failed", { err: String(err) }));
      };
      this.subtitleCheckTimer = window.setTimeout(() => void poll(), POLL_MS);
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
            if (!this.started) {
               console.info("[mpv] first frame");
               this.scheduleSubtitleCheck();
            }
            this.started = true;
            if (this.seekTarget != null || this.pendingSeek != null)
               console.debug("[mpv] seek restarted", { target: this.seekTarget, pending: this.pendingSeek, masked: this.maskedPositions });
            this.maskedPositions = 0;
            this.releaseSeek();
            this.timeStamp = performance.now();
            this.emit("canplay");
            this.emit("playing");
            this.emit("seeked");
            return;
         case "start-file":
            if (this.entryId == null && e.playlist_entry_id != null && this.isCurrentEntry(e.playlist_entry_id)) {
               this.entryId = e.playlist_entry_id;
               console.debug("[mpv] start-file", { entryId: this.entryId });
            }
            return;
         case "stream-corrupt":
            void this.rebuildBuffer(e);
            return;
         case "end-file":
            if (!this.isCurrentEntry(e.playlist_entry_id)) {
               console.info("[mpv] ignoring end-file of a previous entry", {
                  entryId: e.playlist_entry_id,
                  current: this.entryId,
                  reason: e.reason,
                  error: e.file_error,
               });
               return;
            }
            // No playback-restart is coming for a seek into a closed file.
            window.clearTimeout(this.seekReleaseTimer);
            this.seekTarget = null;
            this.pendingSeek = null;
            if (e.reason === "error") this.fail(e.file_error ?? "unknown error");
            return;
         default:
            return;
      }
   }

   private handleProperty(name: Observed, data: unknown) {
      const previous = this.props[name];
      // Positions from before an in-flight seek would snap the seek bar
      // back to where playback was.
      if (name === "time-pos" && (this.seekTarget != null || this.pendingSeek != null)) {
         this.maskedPositions++;
         return;
      }
      this.props[name] = data;
      switch (name) {
         case "time-pos": {
            const now = performance.now();
            this.timeStamp = now;
            if (now - this.lastTimeUpdate >= TIMEUPDATE_INTERVAL_MS) {
               this.lastTimeUpdate = now;
               this.emit("timeupdate");
            }
            return;
         }
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
         case "chapter-list":
            this.emit("chapters");
            return;
         case "sub-ass-extradata":
            this.emit("subheader");
            return;
         case "speed":
            this.emit("ratechange");
            return;
         default:
            return;
      }
   }
}
