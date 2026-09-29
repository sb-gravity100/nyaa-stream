import { useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { SubtitleTrack } from "./types";
import { subtitleTrackLabel } from "./subtitles";
import { updateSettings } from "./settings";

/** `m:ss.d` (or `h:mm:ss.d`) - the clip range's editable form. */
export function formatClipTime(seconds: number): string {
  const clamped = Number.isFinite(seconds) && seconds > 0 ? seconds : 0;
  const tenths = Math.round(clamped * 10);
  const h = Math.floor(tenths / 36000);
  const m = Math.floor((tenths % 36000) / 600);
  const s = Math.floor((tenths % 600) / 10);
  const d = tenths % 10;
  const ss = s.toString().padStart(2, "0");
  return h > 0 ? `${h}:${m.toString().padStart(2, "0")}:${ss}.${d}` : `${m}:${ss}.${d}`;
}

/** Parses `83.5`, `1:23.5` or `1:01:23.5` into seconds; null when it isn't a time. */
export function parseClipTime(text: string): number | null {
  const parts = text.trim().split(":");
  if (parts.length === 0 || parts.length > 3 || parts.some((p) => !/^\d+(\.\d+)?$/.test(p))) return null;
  return parts.reduce((total, part) => total * 60 + parseFloat(part), 0);
}

export interface ExportRequest {
  name: string;
  start: number;
  end: number;
  /** Position within the file's audio tracks. */
  audioPosition: number;
  folder: string;
  /** Burn the active subtitle track into the video. */
  includeSubs: boolean;
}

interface Props {
  defaultName: string;
  start: number;
  end: number;
  /** Episode length, the range's upper bound. */
  duration: number;
  audioTracks: SubtitleTrack[];
  activeAudioId: number | null;
  /** Label of the subtitle track showing now; null when subtitles are off. */
  subtitleLabel: string | null;
  folder: string;
  exporting: boolean;
  /** Path of the clip that was just written. */
  savedPath: string | null;
  error: string | null;
  onExport: (request: ExportRequest) => void;
  onClose: () => void;
}

/** Export options for the looped section: name, range, audio track and
 * destination folder. Rendered inside the player; owns no export logic. */
export function ExportDialog({ defaultName, start, end, duration, audioTracks, activeAudioId, subtitleLabel, folder, exporting, savedPath, error, onExport, onClose }: Props) {
  const [name, setName] = useState(defaultName);
  const [startText, setStartText] = useState(formatClipTime(start));
  const [endText, setEndText] = useState(formatClipTime(end));
  const activePosition = Math.max(0, audioTracks.findIndex((t) => t.index === activeAudioId));
  const [audioPosition, setAudioPosition] = useState(activePosition);
  const [includeSubs, setIncludeSubs] = useState(subtitleLabel != null);
  const [defaultFolder, setDefaultFolder] = useState("");
  const dialogRef = useRef<HTMLDivElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
    nameRef.current?.select();
    invoke<string>("default_clip_folder").then(setDefaultFolder).catch(() => undefined);
  }, []);

  const startSeconds = parseClipTime(startText);
  const endSeconds = parseClipTime(endText);
  const rangeError =
    startSeconds == null || endSeconds == null
      ? "Enter times like 1:23.5"
      : endSeconds <= startSeconds
        ? "The end must be after the start"
        : duration > 0 && endSeconds > duration + 0.5
          ? `The episode is only ${formatClipTime(duration)} long`
          : null;
  const length = startSeconds != null && endSeconds != null && endSeconds > startSeconds ? endSeconds - startSeconds : null;
  const canExport = !exporting && !rangeError && name.trim().length > 0;
  const shownFolder = folder || defaultFolder;

  async function chooseFolder() {
    try {
      const picked = await pickFolder({ directory: true, multiple: false, defaultPath: shownFolder || undefined, title: "Save clips to" });
      if (typeof picked === "string") updateSettings({ exportFolder: picked });
    } catch (err) {
      console.warn("[export] folder picker failed", { err: String(err) });
    }
  }

  function submit() {
    if (!canExport || startSeconds == null || endSeconds == null) return;
    onExport({ name: name.trim(), start: startSeconds, end: endSeconds, audioPosition, folder, includeSubs: includeSubs && subtitleLabel != null });
  }

  return (
    <div
      class="export-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !exporting) onClose();
      }}
    >
      <div
        class="export-dialog"
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Export clip"
        onKeyDown={(e) => {
          // The player's hotkeys must not fire while typing here.
          e.stopPropagation();
          if (e.key === "Escape" && !exporting) onClose();
          if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") submit();
        }}
      >
        <h2 class="export-title">Export clip</h2>

        <label class="export-field">
          <span>File name</span>
          <input ref={nameRef} type="text" class="setting-text-input" disabled={exporting} value={name} onInput={(e) => setName((e.target as HTMLInputElement).value)} />
          <small>Saved as MP4 (H.264 / AAC). An existing file is never overwritten.</small>
        </label>

        <div class="export-range">
          <label class="export-field">
            <span>Start</span>
            <input type="text" class="setting-text-input" disabled={exporting} value={startText} onInput={(e) => setStartText((e.target as HTMLInputElement).value)} />
          </label>
          <label class="export-field">
            <span>End</span>
            <input type="text" class="setting-text-input" disabled={exporting} value={endText} onInput={(e) => setEndText((e.target as HTMLInputElement).value)} />
          </label>
          <div class="export-length">{rangeError ? <span class="export-invalid">{rangeError}</span> : length != null ? `${formatClipTime(length)} long` : ""}</div>
        </div>

        {audioTracks.length > 1 && (
          <label class="export-field">
            <span>Audio track</span>
            <select class="setting-text-input" disabled={exporting} value={audioPosition} onChange={(e) => setAudioPosition(Number((e.target as HTMLSelectElement).value))}>
              {audioTracks.map((track, i) => (
                <option key={track.index} value={i}>
                  {subtitleTrackLabel(track, i)} ({track.codec.toUpperCase()})
                </option>
              ))}
            </select>
          </label>
        )}

        <label class="export-check">
          <input type="checkbox" disabled={exporting || subtitleLabel == null} checked={includeSubs && subtitleLabel != null} onChange={(e) => setIncludeSubs((e.target as HTMLInputElement).checked)} />
          <span>
            Include subtitles
            <small>{subtitleLabel != null ? `Burned in: ${subtitleLabel}, in your subtitle style` : "No subtitle track is showing"}</small>
          </span>
        </label>

        <div class="export-field">
          <span>Save to</span>
          <div class="export-folder">
            <code title={shownFolder}>{shownFolder || "…"}</code>
            <button class="button button-quiet" type="button" onClick={() => void chooseFolder()} disabled={exporting}>
              Change…
            </button>
            {folder && (
              <button class="button button-quiet" type="button" onClick={() => updateSettings({ exportFolder: "" })} disabled={exporting}>
                Reset
              </button>
            )}
          </div>
        </div>

        {error && (
          <p class="export-invalid" role="alert">
            {error}
          </p>
        )}
        {savedPath && !exporting && (
          <p class="export-saved">
            Saved <code>{savedPath}</code>{" "}
            <button class="button button-quiet" type="button" disabled={exporting} onClick={() => void revealItemInDir(savedPath).catch(() => undefined)}>
              Show in folder
            </button>
          </p>
        )}

        <div class="export-actions">
          <button class="button button-quiet" type="button" onClick={onClose} disabled={exporting}>
            {savedPath ? "Close" : "Cancel"}
          </button>
          <button class="button" type="button" onClick={submit} disabled={!canExport}>
            {exporting ? "Exporting…" : savedPath ? "Export again" : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
