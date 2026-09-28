import type { NyaaResult } from "./types";
import type { PreferredResolution } from "./settings";
import { parseSubmitterFromTitle } from "./episodeParser";

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

export interface ReleasePreferences {
  preferredGroup?: string | null;
  preferredResolution?: PreferredResolution;
}

// A release with too few seeders to actually stream isn't worth picking
// just because it matches a preference.
const MIN_PREFERRED_SEEDERS = 3;

function score(release: NyaaResult, prefs: ReleasePreferences): number {
  let value = Math.log10(release.seeders + 1) * 10;
  if (release.seeders < MIN_PREFERRED_SEEDERS) return value;
  const group = releaseGroup(release);
  if (prefs.preferredGroup && group && group.toLowerCase() === prefs.preferredGroup.toLowerCase()) value += 100;
  const resolution = releaseResolution(release);
  if (prefs.preferredResolution && prefs.preferredResolution !== "any" && resolution === prefs.preferredResolution) value += 40;
  return value;
}

// Preferred fansub group first (if remembered and reasonably seeded), then
// preferred resolution, then seeders - fastest/most reliable swarm. Used
// for the play button's auto-pick; `sortReleases` orders the player's
// source picker the same way.
export function bestRelease(releases: NyaaResult[], prefs: ReleasePreferences = {}): NyaaResult {
  return releases.reduce((best, r) => (score(r, prefs) > score(best, prefs) ? r : best));
}

export function sortReleases(releases: NyaaResult[], prefs: ReleasePreferences = {}): NyaaResult[] {
  return [...releases].sort((a, b) => score(b, prefs) - score(a, prefs));
}
