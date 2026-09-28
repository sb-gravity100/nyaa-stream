import { useEffect, useState } from "preact/hooks";

// localStorage, same reasoning as library.ts: no backend work, identical in
// the Tauri webview and the browser-only dev fallback.
const STORAGE_KEY = "nyaa-stream:settings";

export interface SubtitleStyle {
  /** Font family libass should use. Resolved from embedded fonts first,
   * then local system fonts (JASSUB `queryFonts: "local"`), falling back
   * to JASSUB's bundled Liberation Sans. */
  fontFamily: string;
  /** Font size as a percentage of video height. */
  sizePercent: number;
  bold: boolean;
  /** `#rrggbb` */
  color: string;
  outlineColor: string;
  /** Outline thickness as a percentage of video height. */
  outlineWidth: number;
  shadow: number;
  /** Opaque box behind text instead of an outline (ASS BorderStyle 3). */
  background: boolean;
  backgroundColor: string;
  /** 0-100 */
  backgroundOpacity: number;
  /** Bottom margin as a percentage of video height. */
  marginPercent: number;
  /** Also restyle the main dialogue style of real (styled) ASS tracks -
   * off by default: fansub styling is usually intentional. Signs/typeset
   * styles are never touched. */
  applyToStyled: boolean;
}

export type PreferredResolution = "any" | "2160" | "1080" | "720" | "480";

export interface Settings {
  subtitleStyle: SubtitleStyle;
  subtitlesEnabled: boolean;
  /** ISO 639 prefix matched against track language tags (e.g. "en"). */
  subtitleLanguage: string;
  preferredResolution: PreferredResolution;
  /** Resume an episode from its saved position. */
  resumePlayback: boolean;
  /** Offer/auto-start the next episode when one ends. */
  autoplayNext: boolean;
  /** Remember the fansub group last played per anime and prefer it. */
  rememberFansubGroup: boolean;
  /** Only list rows that are real episodes per AniList/Kitsu - hides
   * off-list episode numbers, Batch and Unknown rows (see
   * `listedSources` in MediaPage). */
  hideUnlistedSources: boolean;
}

/** Crunchyroll's own dialogue style (its English ASS tracks, e.g. a
 * ToonsHub CR WEB-DL: `Style: Default,Gandhi Sans,24,&H00FFFFFF,...,
 * &H00000000,&HA0404040,-1,...,1,1.2,0.5,2,20,20,20` at PlayResY 360),
 * converted to height percentages. Gandhi Sans is bundled
 * (`src/assets/fonts`). */
export const DEFAULT_SUBTITLE_STYLE: SubtitleStyle = {
  fontFamily: "Gandhi Sans",
  sizePercent: 6.67,
  bold: true,
  color: "#ffffff",
  outlineColor: "#000000",
  outlineWidth: 0.33,
  shadow: 0.5,
  background: false,
  backgroundColor: "#000000",
  backgroundOpacity: 60,
  marginPercent: 5.6,
  applyToStyled: false,
};

/** Bumped when DEFAULT_SUBTITLE_STYLE changes in a way stored settings
 * should adopt: an older stored style is replaced by the new default. */
const SUBTITLE_STYLE_VERSION = 2;

export const DEFAULT_SETTINGS: Settings = {
  subtitleStyle: DEFAULT_SUBTITLE_STYLE,
  subtitlesEnabled: true,
  subtitleLanguage: "en",
  preferredResolution: "1080",
  resumePlayback: true,
  autoplayNext: true,
  rememberFansubGroup: true,
  hideUnlistedSources: true,
};

function load(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<Settings>;
    // Shallow-merged over defaults so settings added later get sane values
    // for users with an older stored blob.
    const stored = parsed as Partial<Settings> & { subtitleStyleVersion?: number };
    const styleCurrent = (stored.subtitleStyleVersion ?? 1) >= SUBTITLE_STYLE_VERSION;
    if (!styleCurrent) console.info("[settings] adopting new default subtitle style (Crunchyroll)");
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      subtitleStyle: styleCurrent ? { ...DEFAULT_SUBTITLE_STYLE, ...(parsed.subtitleStyle ?? {}) } : DEFAULT_SUBTITLE_STYLE,
    };
  } catch (err) {
    console.error("[settings] failed to read from localStorage", { err });
    return DEFAULT_SETTINGS;
  }
}

let current: Settings = load();
const listeners = new Set<(s: Settings) => void>();

export function getSettings(): Settings {
  return current;
}

export function updateSettings(patch: Partial<Settings>): Settings {
  current = { ...current, ...patch };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...current, subtitleStyleVersion: SUBTITLE_STYLE_VERSION }));
  } catch (err) {
    console.error("[settings] failed to write to localStorage", { err });
  }
  console.info("[settings] updated", { keys: Object.keys(patch) });
  listeners.forEach((listener) => listener(current));
  return current;
}

export function resetSettings(): Settings {
  console.info("[settings] reset to defaults");
  return updateSettings(DEFAULT_SETTINGS);
}

/** Live view of the settings store - re-renders on every update. */
export function useSettings(): Settings {
  const [settings, setSettings] = useState(current);
  useEffect(() => {
    listeners.add(setSettings);
    setSettings(current);
    return () => {
      listeners.delete(setSettings);
    };
  }, []);
  return settings;
}
