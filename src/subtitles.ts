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
