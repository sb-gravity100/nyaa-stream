import { labelOf } from "./episodeParser";
import type { NyaaResult, PlayFile } from "./types";

const EXTRA_FILE = /\b(OVA|OAD|ONA|SP|Specials?|NCOP|NCED|Extras?|PV|Trailer)\d*\b/i;
const lastBatch = new Map<number, { magnet: string; files: PlayFile[] }>();

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function magnetHash(magnet: string): string | null {
  return magnet.match(/xt=urn:btih:([a-zA-Z0-9]+)/i)?.[1]?.toLowerCase() ?? null;
}

export function isExtraFile(file: PlayFile): boolean {
  const label = labelOf(file.label, baseName(file.name));
  return EXTRA_FILE.test(baseName(file.name)) || label.kind === "special" || (label.kind === "batch" && label.extras === true);
}

/** Finds the requested episode without selecting another season or an extra. */
export function matchEpisodeFile(files: PlayFile[], episode: number, season: number | null): PlayFile | null {
  console.debug("[player] matching episode in torrent files", { episode, season, fileCount: files.length });
  const regular = files.filter((file) => file.isVideo && !isExtraFile(file));
  const matches = regular.filter((file) => {
    const label = labelOf(file.label, baseName(file.name));
    return label.kind === "episode" && label.number === episode;
  });
  if (matches.length === 0) {
    console.info("[player] batch has no matching episode file", { episode, season });
    return null;
  }
  if (season == null) return matches.reduce((a, b) => b.length > a.length ? b : a);

  const declaredSeasons = new Set(regular.flatMap((file) => {
    const label = labelOf(file.label, baseName(file.name));
    return "seasons" in label ? label.seasons ?? [] : [];
  }));
  const exact = matches.filter((file) => {
    const label = labelOf(file.label, baseName(file.name));
    return label.kind === "episode" && label.seasons?.includes(season);
  });
  if (exact.length > 0) {
    const chosen = exact.reduce((a, b) => b.length > a.length ? b : a);
    console.info("[player] matched explicit season episode file", { episode, season, file: chosen.name });
    return chosen;
  }
  if (declaredSeasons.size > 1 || (declaredSeasons.size === 1 && !declaredSeasons.has(season))) {
    console.warn("[player] batch file belongs to another declared season", { episode, season, declaredSeasons: [...declaredSeasons] });
    return null;
  }
  const ambiguous = matches.filter((file) => {
    const label = labelOf(file.label, baseName(file.name));
    return label.kind === "episode" && (label.seasons?.length ?? 0) === 0;
  });
  const chosen = (ambiguous.length > 0 ? ambiguous : matches).reduce((a, b) => b.length > a.length ? b : a);
  console.info("[player] matched implicit season episode file", { episode, season, file: chosen.name });
  return chosen;
}

export function nextEpisodeFile(files: PlayFile[], current: PlayFile, season: number | null): PlayFile | null {
  const label = labelOf(current.label, baseName(current.name));
  if (label.kind !== "episode") {
    console.debug("[player] next batch file unavailable from an unnumbered file", { file: current.name });
    return null;
  }
  const next = matchEpisodeFile(files, label.number + 1, season ?? label.season);
  console.debug("[player] next batch file resolved", { current: current.name, next: next?.name ?? null });
  return next;
}

/** Keeps only a resolved multi-file source as the next episode's preference. */
export function rememberBatchSource(animeId: number, magnet: string, files: PlayFile[]): void {
  const count = files.filter((file) => file.isVideo && !isExtraFile(file)).length;
  if (count > 1) {
    lastBatch.set(animeId, { magnet, files });
    console.info("[player] remembered batch source", { animeId, fileCount: count });
  } else {
    lastBatch.delete(animeId);
    console.debug("[player] cleared batch source preference", { animeId });
  }
}

export function preferredBatchRelease(animeId: number, releases: NyaaResult[], episode: number | null, season: number | null): NyaaResult | null {
  const previous = lastBatch.get(animeId);
  if (!previous || episode == null || !matchEpisodeFile(previous.files, episode, season)) {
    console.debug("[player] remembered batch does not contain requested episode", { animeId, episode, season });
    return null;
  }
  const hash = magnetHash(previous.magnet);
  const release = releases.find((candidate) => candidate.magnet === previous.magnet || (hash && magnetHash(candidate.magnet) === hash)) ?? null;
  console.debug("[player] remembered batch release lookup", { animeId, episode, season, listed: release != null });
  return release;
}
