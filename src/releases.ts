import type { NyaaResult } from "./types";
import type { PreferredResolution } from "./settings";
import { labelOf, parseSubmitterFromTitle } from "./episodeParser";

// localStorage map of AniList id -> fansub group last chosen for that
// anime, so the next episode auto-picks the same group (consistent
// subtitles/typesetting across a season) instead of whichever release
// happens to have the most seeders.
const GROUP_STORAGE_KEY = "nyaa-stream:preferred-groups";

function loadGroups(): Record<string, string> {
  try {
    const raw = localStorage.getItem(GROUP_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch (err) {
    console.error("[releases] failed to read preferred groups", { err });
    return {};
  }
}

export function releaseGroup(release: NyaaResult): string | null {
  return parseSubmitterFromTitle(release.title);
}

export function getPreferredGroup(animeId: number): string | null {
  return loadGroups()[String(animeId)] ?? null;
}

export function setPreferredGroup(animeId: number, group: string | null): void {
  const groups = loadGroups();
  if (group) groups[String(animeId)] = group;
  else delete groups[String(animeId)];
  try {
    localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(groups));
    console.info("[releases] preferred group saved", { animeId, group });
  } catch (err) {
    console.error("[releases] failed to save preferred group", { err });
  }
}

const RESOLUTION_PATTERN = /\b(2160|1080|720|480)p\b|\b(4k)\b/i;

export function releaseResolution(release: NyaaResult): string | null {
  const match = release.title.match(RESOLUTION_PATTERN);
  if (!match) return null;
  return match[1] ?? "2160";
}

// Video codec from the release title. Only the ones WebView2 may not be
// able to decode matter - everything else is assumed H.264.
const HEVC_PATTERN = /\b(hevc|x265|h\.?265)\b/i;
const AV1_PATTERN = /\bav1\b/i;

export type RiskyCodec = "hevc" | "av1";

export function releaseCodec(release: NyaaResult): RiskyCodec | null {
  if (HEVC_PATTERN.test(release.title)) return "hevc";
  if (AV1_PATTERN.test(release.title)) return "av1";
  return null;
}

const CODEC_PROBES: Record<RiskyCodec, string> = {
  hevc: 'video/mp4; codecs="hvc1.1.6.L120.90"',
  av1: 'video/mp4; codecs="av01.0.08M.08"',
};
const codecSupportCache = new Map<RiskyCodec, boolean>();

/** Whether this machine's WebView can decode `codec` through MSE (HEVC
 * needs the OS extension/hardware on Windows - verified live: an HEVC
 * release failed with hls.js bufferAddCodecError). Releases that fail
 * this are transcoded by the streaming server, not rejected. */
export function codecPlayable(codec: RiskyCodec | null): boolean {
  if (codec == null) return true;
  let supported = codecSupportCache.get(codec);
  if (supported === undefined) {
    supported = typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(CODEC_PROBES[codec]);
    codecSupportCache.set(codec, supported);
    console.info("[releases] codec support", { codec, supported });
  }
  return supported;
}

export interface ReleasePreferences {
  preferredGroup?: string | null;
  preferredFansubber?: string;
  preferredResolution?: PreferredResolution;
}

// A release with too few seeders to actually stream isn't worth picking
// just because it matches a preference.
const MIN_PREFERRED_SEEDERS = 3;

/** Whether the title contains every word of the user's preferred-fansubber
 * text ("ToonsHub CR" matches "... 1080p CR WEB-DL ... -ToonsHub"). */
export function matchesFansubber(release: NyaaResult, fansubber: string | undefined): boolean {
  const words = fansubber?.toLowerCase().split(/\s+/).filter(Boolean) ?? [];
  if (words.length === 0) return false;
  const title = release.title.toLowerCase();
  return words.every((word) => title.includes(word));
}

function score(release: NyaaResult, prefs: ReleasePreferences): number {
  let value = Math.log10(release.seeders + 1) * 10;
  // Not natively decodable here: the streaming server transcodes it to
  // H.264, which works but costs GPU/CPU and startup time - so a native
  // release of similar health wins, without burying the others.
  if (!codecPlayable(releaseCodec(release))) value -= 25;
  if (release.seeders < MIN_PREFERRED_SEEDERS) return value;
  const group = releaseGroup(release);
  if (prefs.preferredGroup && group && group.toLowerCase() === prefs.preferredGroup.toLowerCase()) value += 100;
  if (matchesFansubber(release, prefs.preferredFansubber)) value += 60;
  const resolution = releaseResolution(release);
  if (prefs.preferredResolution && prefs.preferredResolution !== "any" && resolution === prefs.preferredResolution) value += 40;
  return value;
}

// Whether the title is a season/batch pack rather than one episode.
export function isBatchRelease(release: NyaaResult): boolean {
  return labelOf(release.label, release.title).kind === "batch";
}

// Healthy single-episode releases rank before healthy batches, which rank
// before anything with too few seeders to count: a season pack listed under
// an episode (see App.tsx's range spreading) should never be auto-picked
// over a real single-episode release just because its swarm is bigger, but
// still beats a dead single.
function tier(release: NyaaResult): number {
  if (release.seeders < MIN_PREFERRED_SEEDERS) return 2;
  return isBatchRelease(release) ? 1 : 0;
}

function rankBefore(a: NyaaResult, b: NyaaResult, prefs: ReleasePreferences): number {
  return tier(a) - tier(b) || score(b, prefs) - score(a, prefs);
}

// This show's remembered fansub group first, then the preferred fansubber
// setting (both only if reasonably seeded), then
// preferred resolution, then seeders - fastest/most reliable swarm. Used
// for the play button's auto-pick; `sortReleases` orders the player's
// source picker the same way.
export function bestRelease(releases: NyaaResult[], prefs: ReleasePreferences = {}): NyaaResult {
  return releases.reduce((best, r) => (rankBefore(r, best, prefs) < 0 ? r : best));
}

export function sortReleases(releases: NyaaResult[], prefs: ReleasePreferences = {}): NyaaResult[] {
  return [...releases].sort((a, b) => rankBefore(a, b, prefs));
}

// ---------------------------------------------------------------- badges

export type BadgeKind = "info" | "good" | "warn";

export interface ReleaseBadge {
  label: string;
  kind: BadgeKind;
}

const BATCH_PATTERN = /\b(batch|complete(?:\s+series)?|season\s*\d*\s*(?:complete|pack))\b|\b\d{2,3}[-~]\d{2,3}\b(?!\s*(?:p|bit|kbps|fps))/i;
const DUAL_AUDIO_PATTERN = /\b(dual[\s._-]?audio|multi[\s._-]?audio|dubbed)\b/i;
const TEN_BIT_PATTERN = /\b(10[\s._-]?bit|hi10p?)\b/i;
const SOURCE_PATTERN = /\b(BD|Blu-?Ray|BDRip|WEB-?DL|WEB-?Rip|DVD)\b/i;

/** Quick-scan facts about a release, read from its title - what makes one
 * source a better pick than another beyond seeders. */
export function releaseBadges(release: NyaaResult): ReleaseBadge[] {
  const title = release.title;
  const badges: ReleaseBadge[] = [];
  if (BATCH_PATTERN.test(title) || isBatchRelease(release)) badges.push({ label: "Batch", kind: "info" });
  if (DUAL_AUDIO_PATTERN.test(title)) badges.push({ label: "Dual audio", kind: "good" });
  const source = title.match(SOURCE_PATTERN);
  if (source) badges.push({ label: source[1].replace(/-/g, "").toUpperCase().replace("BLURAY", "BD"), kind: "info" });
  const codec = releaseCodec(release);
  if (codec) badges.push({ label: codec === "hevc" ? "HEVC" : "AV1", kind: "info" });
  if (TEN_BIT_PATTERN.test(title)) badges.push({ label: "10-bit", kind: "info" });
  return badges;
}

export type SeederHealth = "dead" | "weak" | "ok" | "good";

export function seederHealth(seeders: number): SeederHealth {
  if (seeders <= 0) return "dead";
  if (seeders < MIN_PREFERRED_SEEDERS) return "weak";
  if (seeders < 10) return "ok";
  return "good";
}

// ------------------------------------------------- per-anime resolution

const RESOLUTION_STORAGE_KEY = "nyaa-stream:preferred-resolutions";

function loadResolutions(): Record<string, PreferredResolution> {
  try {
    const raw = localStorage.getItem(RESOLUTION_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch (err) {
    console.error("[releases] failed to read preferred resolutions", { err });
    return {};
  }
}

/** This show's own quality choice, overriding the global setting. */
export function getAnimeResolution(animeId: number): PreferredResolution | null {
  return loadResolutions()[String(animeId)] ?? null;
}

export function setAnimeResolution(animeId: number, resolution: PreferredResolution | null): void {
  const all = loadResolutions();
  if (resolution) all[String(animeId)] = resolution;
  else delete all[String(animeId)];
  try {
    localStorage.setItem(RESOLUTION_STORAGE_KEY, JSON.stringify(all));
    console.info("[releases] preferred resolution saved", { animeId, resolution });
  } catch (err) {
    console.error("[releases] failed to save preferred resolution", { err });
  }
}
