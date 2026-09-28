import type { AnimeMedia } from "./types";

// localStorage, same reasoning as library.ts. One entry per (anime,
// episode key); the episode key is the MediaPage group key ("Episode 5",
// "Season 2 Batch"...) so batches and unknown-numbered releases get
// progress too, with `episode` set only when the group is a numbered
// episode.
const STORAGE_KEY = "nyaa-stream:watch-progress";

/** Past this fraction of the duration an episode counts as watched - the
 * ED/preview usually fills the rest. */
export const COMPLETED_FRACTION = 0.9;
/** Positions this close to the start aren't worth resuming. */
const MIN_RESUME_SECONDS = 20;
const MAX_ENTRIES = 500;

export interface ProgressEntry {
  animeId: number;
  episodeKey: string;
  episode: number | null;
  /** Seconds, in source-file time. */
  position: number;
  duration: number;
  completed: boolean;
  updatedAt: number;
  /** Snapshot for the home page's Continue Watching row. */
  anime: Pick<AnimeMedia, "id" | "title" | "coverImage" | "episodes" | "duration" | "format" | "season" | "seasonYear" | "averageScore" | "description">;
}

type Store = Record<string, ProgressEntry>;

function key(animeId: number, episodeKey: string): string {
  return `${animeId}::${episodeKey}`;
}

function load(): Store {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch (err) {
    console.error("[watchProgress] failed to read from localStorage", { err });
    return {};
  }
}

let store: Store = load();
const listeners = new Set<() => void>();

function persist(): void {
  // Oldest entries dropped first so the blob can't grow without bound.
  const entries = Object.entries(store);
  if (entries.length > MAX_ENTRIES) {
    entries.sort(([, a], [, b]) => b.updatedAt - a.updatedAt);
    store = Object.fromEntries(entries.slice(0, MAX_ENTRIES));
  }
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch (err) {
    console.error("[watchProgress] failed to write to localStorage", { err });
  }
  listeners.forEach((listener) => listener());
}

export function subscribeProgress(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getProgress(animeId: number, episodeKey: string): ProgressEntry | null {
  return store[key(animeId, episodeKey)] ?? null;
}

/** Where to resume from, or null to start at 0 (nothing saved, barely
 * started, or already finished). */
export function resumePosition(animeId: number, episodeKey: string): number | null {
  const entry = getProgress(animeId, episodeKey);
  if (!entry || entry.completed || entry.position < MIN_RESUME_SECONDS) return null;
  return entry.position;
}

export function saveProgress(anime: AnimeMedia, episodeKey: string, episode: number | null, position: number, duration: number): void {
  if (!Number.isFinite(position) || !Number.isFinite(duration) || duration <= 0) return;
  const previous = store[key(anime.id, episodeKey)];
  const completed = position / duration >= COMPLETED_FRACTION || (previous?.completed ?? false);
  store[key(anime.id, episodeKey)] = {
    animeId: anime.id,
    episodeKey,
    episode,
    position,
    duration,
    completed,
    updatedAt: Date.now(),
    anime: {
      id: anime.id,
      title: anime.title,
      coverImage: anime.coverImage,
      episodes: anime.episodes,
      duration: anime.duration,
      format: anime.format,
      season: anime.season,
      seasonYear: anime.seasonYear,
      averageScore: anime.averageScore,
      description: anime.description,
    },
  };
  if (completed && !previous?.completed) console.info("[watchProgress] episode completed", { animeId: anime.id, episodeKey });
  persist();
}

export function setWatched(anime: AnimeMedia, episodeKey: string, episode: number | null, watched: boolean): void {
  console.info("[watchProgress] set watched", { animeId: anime.id, episodeKey, watched });
  if (!watched) {
    delete store[key(anime.id, episodeKey)];
    persist();
    return;
  }
  const previous = store[key(anime.id, episodeKey)];
  const duration = previous?.duration || (anime.duration ?? 24) * 60;
  saveProgress(anime, episodeKey, episode, duration, duration);
}

/** All entries for one anime, keyed by episode key. */
export function progressForAnime(animeId: number): Record<string, ProgressEntry> {
  const out: Record<string, ProgressEntry> = {};
  for (const entry of Object.values(store)) {
    if (entry.animeId === animeId) out[entry.episodeKey] = entry;
  }
  return out;
}

/** Most recent in-progress (unfinished) entry per anime, newest first. */
export function continueWatching(limit = 20): ProgressEntry[] {
  const latestPerAnime = new Map<number, ProgressEntry>();
  for (const entry of Object.values(store)) {
    const current = latestPerAnime.get(entry.animeId);
    if (!current || entry.updatedAt >= current.updatedAt) latestPerAnime.set(entry.animeId, entry);
  }
  return [...latestPerAnime.values()]
    .filter((entry) => !entry.completed && entry.position >= MIN_RESUME_SECONDS)
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, limit);
}
