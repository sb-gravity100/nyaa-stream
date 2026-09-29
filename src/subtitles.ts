import type { SubtitleTrack } from "./types";
import type { SubtitleStyle } from "./settings";

/** Codecs that carry their own real styling - everything else reaches us
 * as ffmpeg-converted ASS with a single generic `Default` style. */
const STYLED_CODECS = new Set(["ass", "ssa"]);

export function isStyledTrack(track: SubtitleTrack): boolean {
  return STYLED_CODECS.has(track.codec);
}

export function subtitleTrackLabel(track: SubtitleTrack, position: number): string {
  const base = track.title ?? track.language?.toUpperCase() ?? `Track ${position + 1}`;
  return track.title && track.language ? `${base} (${track.language})` : base;
}

// Signs/songs tracks commonly ship alongside the main dialogue track in
// fansub releases; their titles give them away.
const SIGNS_PATTERN = /sign|song|forced|karaoke|lyrics/i;

/** Picks the track to enable by default: preferred language first,
 * skipping signs/songs-only tracks, then the container's own default
 * flag within that language, then the container default overall. */
export function defaultSubtitleIndex(tracks: SubtitleTrack[], language: string, enabled: boolean): number | null {
  if (!enabled || tracks.length === 0) return null;
  const lang = language.toLowerCase();
  const inLanguage = tracks.filter((t) => t.language?.toLowerCase().startsWith(lang));
  const dialogue = inLanguage.filter((t) => !SIGNS_PATTERN.test(t.title ?? ""));
  const pick =
    dialogue.find((t) => t.default) ??
    dialogue[0] ??
    inLanguage[0] ??
    tracks.find((t) => t.default) ??
    (tracks.length === 1 ? tracks[0] : undefined);
  return pick?.index ?? null;
}

/** `#rrggbb` + alpha (0 = opaque, 255 = transparent) → ASS `&HAABBGGRR`. */
function assColor(hex: string, alpha = 0): string {
  const clean = hex.replace("#", "").padEnd(6, "0");
  const r = clean.slice(0, 2);
  const g = clean.slice(2, 4);
  const b = clean.slice(4, 6);
  const a = Math.max(0, Math.min(255, Math.round(alpha))).toString(16).padStart(2, "0");
  return `&H${a}${b}${g}${r}`.toUpperCase();
}

export function playResY(script: string): number {
  const match = script.match(/^PlayResY:\s*(\d+)/m);
  return match ? Number(match[1]) : 288;
}

// Styles fansubbers use for regular dialogue - only these get restyled
// when `applyToStyled` is on, so signs/typesetting keep their look.
const DIALOGUE_STYLE_NAME = /^(default|main|dialog(ue)?|top|italics?|flashback|narrat(ion|or)|internal|overlap)\b/i;

/**
 * Rewrites the `Style:` lines of a script with the user's default subtitle
 * style. For plain (converted) tracks every style is restyled - ffmpeg only
 * ever emits `Default`. For real ASS tracks only when `applyToStyled` is on,
 * and then only dialogue-looking styles. Sizes are expressed relative to
 * the script's own `PlayResY` so they're independent of the track's
 * authoring resolution.
 */
export function applySubtitleStyle(script: string, style: SubtitleStyle, styled: boolean): string {
  if (styled && !style.applyToStyled) return script;
  const resY = playResY(script);
  const formatMatch = script.match(/^\[V4\+? Styles\][\s\S]*?^Format:\s*(.+)$/m);
  if (!formatMatch) return script;
  const fields = formatMatch[1].split(",").map((f) => f.trim().toLowerCase());
  const col = (name: string) => fields.indexOf(name.toLowerCase());

  const fontSize = ((style.sizePercent / 100) * resY).toFixed(1);
  const outline = style.background ? "0" : ((style.outlineWidth / 100) * resY).toFixed(2);
  const margin = Math.round((style.marginPercent / 100) * resY).toString();
  const backAlpha = 255 - (style.backgroundOpacity / 100) * 255;

  const overrides: Record<string, string> = {
    fontname: style.fontFamily,
    fontsize: fontSize,
    primarycolour: assColor(style.color),
    outlinecolour: style.background ? assColor(style.backgroundColor, backAlpha) : assColor(style.outlineColor),
    // Shadow colour: Crunchyroll's translucent grey (&HA0404040).
    backcolour: style.background ? assColor(style.backgroundColor, backAlpha) : assColor("#404040", 0xa0),
    bold: style.bold ? "-1" : "0",
    borderstyle: style.background ? "3" : "1",
    outline: style.background ? "2" : outline,
    shadow: String(style.shadow),
    marginv: margin,
  };

  return script.replace(/^Style:\s*(.+)$/gm, (line, body: string) => {
    const values = body.split(",");
    const name = values[col("name")]?.trim() ?? "";
    if (styled && !DIALOGUE_STYLE_NAME.test(name)) return line;
    for (const [field, value] of Object.entries(overrides)) {
      const i = col(field);
      if (i >= 0 && i < values.length) values[i] = value;
    }
    return `Style: ${values.join(",")}`;
  });
}

/** What `applyMpvSubtitleStyle` needs from the player (`MpvVideo`). */
interface MpvSubtitleTarget {
  setProperty(name: string, value: unknown): Promise<unknown>;
  getProperty(name: string): Promise<unknown>;
}

/** `#rrggbb` + opacity 0-1 → mpv's `#AARRGGBB`. */
function mpvColor(hex: string, opacity = 1): string {
  const a = Math.round(Math.max(0, Math.min(1, opacity)) * 255).toString(16).padStart(2, "0");
  return `#${a}${hex.replace("#", "").padEnd(6, "0")}`.toUpperCase();
}

// mpv's sub-* sizes are pixels at a 720px-tall window; our style is in
// percent of video height.
const MPV_REFERENCE_HEIGHT = 720;
// Plain tracks used to reach libass as ffmpeg-converted ASS, whose
// PlayResY (288) the stored `shadow` value is relative to.
const CONVERTED_PLAY_RES_Y = 288;

/**
 * The mpv options (`sub-*`) that make mpv draw subtitles in the user's default
 * style. Plain tracks go through mpv's own `sub-*` options (mpv styles
 * non-ASS subtitles with them). Real ASS tracks are only touched when
 * `applyToStyled` is on, and then only their dialogue-looking styles, via
 * `sub-ass-style-overrides` sized against the track's own `PlayResY` - the
 * same rules `applySubtitleStyle` applied to scripts. Shared by the live
 * player (`applyMpvSubtitleStyle`) and clip export (burned-in subtitles).
 */
export async function mpvSubtitleSettings(
  mpv: Pick<MpvSubtitleTarget, "getProperty">,
  style: SubtitleStyle,
  styled: boolean,
): Promise<Record<string, string | number | boolean>> {
  const px = (percent: number) => (percent / 100) * MPV_REFERENCE_HEIGHT;
  const backOpacity = style.backgroundOpacity / 100;
  const settings: Record<string, string | number | boolean> = {
    "sub-font": style.fontFamily,
    "sub-font-size": px(style.sizePercent),
    "sub-bold": style.bold,
    "sub-color": mpvColor(style.color),
    "sub-border-style": style.background ? "opaque-box" : "outline-and-shadow",
    "sub-border-color": style.background ? mpvColor(style.backgroundColor, backOpacity) : mpvColor(style.outlineColor),
    "sub-back-color": style.background ? mpvColor(style.backgroundColor, backOpacity) : mpvColor("#404040", 0x5f / 255),
    "sub-border-size": style.background ? 2 : px(style.outlineWidth),
    "sub-shadow-offset": (style.shadow / CONVERTED_PLAY_RES_Y) * MPV_REFERENCE_HEIGHT,
    // Integer option - mpv rejects fractional pixels.
    "sub-margin-y": Math.round(px(style.marginPercent)),
    "sub-ass-style-overrides": "",
  };
  if (!styled || !style.applyToStyled) return settings;

  // The active track's script header: its PlayResY and style names.
  let header = "";
  try {
    header = String((await mpv.getProperty("sub-ass-extradata")) ?? "");
  } catch (err) {
    console.debug("[subtitles] no ASS header for restyle yet", { err: String(err) });
  }
  const resY = playResY(header);
  const names = [...header.matchAll(/^Style:\s*([^,]+),/gm)].map((m) => m[1].trim()).filter((n) => DIALOGUE_STYLE_NAME.test(n));
  const backAlpha = 255 - backOpacity * 255;
  const fields: Record<string, string> = {
    Fontname: style.fontFamily,
    Fontsize: ((style.sizePercent / 100) * resY).toFixed(1),
    PrimaryColour: assColor(style.color),
    OutlineColour: style.background ? assColor(style.backgroundColor, backAlpha) : assColor(style.outlineColor),
    BackColour: style.background ? assColor(style.backgroundColor, backAlpha) : assColor("#404040", 0xa0),
    Bold: style.bold ? "-1" : "0",
    BorderStyle: style.background ? "3" : "1",
    Outline: style.background ? "2" : ((style.outlineWidth / 100) * resY).toFixed(2),
    Shadow: String(style.shadow),
    MarginV: Math.round((style.marginPercent / 100) * resY).toString(),
  };
  // Commas separate overrides, so a font name can't contain one.
  const overrides = names.flatMap((name) => Object.entries(fields).map(([k, v]) => `${name}.${k}=${v.replace(/,/g, "")}`));
  console.debug("[subtitles] restyling ASS dialogue styles", { names, resY });
  settings["sub-ass-style-overrides"] = overrides.join(",");
  return settings;
}

/** Applies the user's default subtitle style to the playing mpv. */
export async function applyMpvSubtitleStyle(mpv: MpvSubtitleTarget, style: SubtitleStyle, styled: boolean): Promise<void> {
  const settings = await mpvSubtitleSettings(mpv, style, styled);
  await Promise.all(Object.entries(settings).map(([name, value]) => mpv.setProperty(name, value)));
}
