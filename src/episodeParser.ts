// Naive fansub-title episode number parser — a stand-in for the real
// Phase 2 matcher (see PLAN.md "Known gaps"). Good enough to bucket search
// results for manual testing, not a substitute for real parsing.
//
// Verified against 150 real nyaa.si titles for "Frieren: Beyond Journey's
// End" (a search that legitimately spans two seasons plus dozens of fansub
// groups' differing conventions) — see episode-parsing analysis in
// conversation history for the full breakdown. 147/150 classified
// correctly; the remaining 3 have no season/episode/batch signal in the
// title text at all (e.g. "[DB] Sousou no Frieren | Frieren: Beyond
// Journey's End [Dual Audio 10bit BD1080p][HEVC-x265]") and rely on the
// view-page file-count scrape (nyaa-client's fetch_details) as the
// deterministic backstop — confirmed those 3 do resolve correctly there.
//
// Re-verified against 160 live Frieren titles plus a live "That Time I
// Got Reincarnated as a Slime" search after fixing three mismatches found
// in the latter (dual season-relative/absolute numbering in one title,
// "Season N - M" misread as a batch range, and a pipe-terminated dash
// number falling through to no match at all) — see findEpisodeNumber and
// SEASON_DASH_EPISODE_PATTERN below. 156/160 Frieren titles episode,
// 4/160 batch (all correctly), 0 unknown - no regression from the fix.
// crates/nyaa-client/examples/search_debug.rs re-runs this kind of check
// against live nyaa.si data without needing the full app running.

// Checked separately from EPISODE_PATTERNS below because it's the only
// pattern that also captures a season number. Allows an optional "v2"/"v3"
// revision suffix directly after the episode number (e.g. "S01E20v2") —
// without it the trailing \b boundary never matches because a digit
// directly followed by a letter has no word-boundary between them, so the
// whole title silently fell to "Unknown".
const SEASON_EPISODE_PATTERN = /\bS(\d{1,2})E(\d{1,4})(?:v\d+)?\b/i;

// Checked leftmost-match-wins (see findEpisodeNumber below), not in fixed
// priority order: a title can carry two different numbering schemes at
// once, e.g. Asakura's "...4th Season - 02 [1080p...] | ... Season 4 |
// Episode 74" - "02" is the season-relative number every other fansub
// group's release for the same episode uses, "74" is an absolute-episode
// cross-reference appended near the end. Checking "Episode" before the
// dash patterns unconditionally (the old behavior) always grabbed the
// absolute number instead, filing the release under a bogus episode
// bucket disconnected from every other group's - verified live against
// real nyaa.si data for "That Time I Got Reincarnated as a Slime". The
// season-relative number is reliably the one right after the title, so
// leftmost-wins fixes this without needing to special-case the group.
const EPISODE_PATTERNS: RegExp[] = [
  /\bEp(?:isode)?\.?\s*(\d{1,4})\b/i, // Episode 12 / Ep 12 / Ep.12
  // "Title - 12 [1080p]" / "Title - 12 (v2)" / "Title - 12 | Alt Title" -
  // the "|" terminator matters: without it, a title like "3rd Season - 21
  // | That Time I Got Reincarnated as a Slime Season 3 | S3 [English
  // Dub][1080p]" (real nyaa.si data) had no pattern match its episode
  // number at all (the bracket/end-of-string terminators don't cover a
  // pipe-separated alt title following it) and silently fell through to
  // the season-only batch fallback below, hiding a real single episode
  // behind a bogus "Season 3 Batch" bucket - verified live.
  / - (\d{1,4})(?:v\d)?\s*[[(|]/,
  / - (\d{1,4})(?:v\d)?\s*$/, // "Title - 12" at end of string
];

// Some groups (e.g. Doomdos) write "Season 4 - 92" instead of "S04E92" -
// a season number and an episode number joined by a dash, not a range.
// EPISODE_RANGE_PATTERN below can't tell the difference and would
// misread this as "episodes 4 through 92", filing a single episode as a
// giant batch - verified live. Checked before the batch/range check so
// this wins for exactly this shape; a real range following a bare season
// marker without "Season" directly attached to the first number is
// unaffected.
const SEASON_DASH_EPISODE_PATTERN = /\bseason\s*\d{1,2}\s*-\s*(\d{1,4})\b/i;

// Weakest, most false-positive-prone signal (a bare "E" + digits can
// coincidentally appear in unrelated bracketed metadata) - kept as a
// strict last resort rather than part of the leftmost-wins comparison
// above, so it only ever fires when nothing more specific matched at all.
const FALLBACK_EPISODE_PATTERN = /\bE(\d{1,4})(?:v\d+)?\b/i;

// Leftmost match among EPISODE_PATTERNS, falling back to the weaker bare
// "E12" pattern only if none of them matched anywhere in the title.
function findEpisodeNumber(title: string): number | null {
  let best: { index: number; number: number } | null = null;
  for (const pattern of EPISODE_PATTERNS) {
    const match = title.match(pattern);
    if (!match || match.index == null) continue;
    const number = parseInt(match[match.length - 1], 10);
    if (Number.isNaN(number)) continue;
    if (best === null || match.index < best.index) {
      best = { index: match.index, number };
    }
  }
  if (best) return best.number;
  const fallback = title.match(FALLBACK_EPISODE_PATTERN);
  return fallback ? parseInt(fallback[1], 10) : null;
}

// Checked before EPISODE_PATTERNS' bare "E12" case: a title like "S01
// E15-E28 ... [Batch - Part 2]" contains "E15", which the bare-episode
// pattern would happily (and wrongly) match as a single episode 15 if
// checked first. An explicit "batch" keyword or an episode-range like
// "E15-E28"/"01-38" is a stronger, unambiguous signal that should win.
const BATCH_KEYWORD_PATTERN = /\bbatch\b/i;
const EPISODE_RANGE_PATTERN = /\bE?(\d{1,4})\s*-\s*E?(\d{1,4})\b/i;

// Extracts the actual episode numbers out of a range like "E15-E28" or
// "01-38" so a batch can be spread across the specific episode rows it
// covers instead of sitting in one undifferentiated "Batch" bucket.
// Guards against a bracketed year range like "[2023-2024]" being misread
// as episodes 2023 through 2024 — verified against a real title
// ("[JP-EN] ... S01 [2023-2024] COMPLETE ...") that would otherwise
// produce a nonsense range; that title still correctly classifies as a
// batch via the plain "S01" pattern below, just without a spread range.
function extractEpisodeRange(title: string): [number, number] | null {
  const match = title.match(EPISODE_RANGE_PATTERN);
  if (!match) return null;
  const min = parseInt(match[1], 10);
  const max = parseInt(match[2], 10);
  if (Number.isNaN(min) || Number.isNaN(max) || min >= max) return null;
  if (min >= 1900 || max >= 1900) return null;
  return [min, max];
}

// "Final Season" (Attack on Titan's actual season 4, and used the same way
// by other long-running franchises) names a season with no digit attached
// at all - extractSeasonNumber used to silently default that straight to
// season 1, which collided every "Final Season" release's episode number
// with the real season 1's same episode number in the exact same "Episode
// N" bucket - verified live against real Attack on Titan/Shingeki no
// Kyojin data. There's no way to recover the *actual* season number from
// title text alone (that would need AniList/domain knowledge this regex
// parser doesn't have - see the file's top comment), so this only needs a
// season value that (a) is consistent across all "Final Season" releases
// and (b) can't collide with any real numbered season; the exact number
// is never shown; episodeLabelText renders it back as "Final Season".
const FINAL_SEASON_PATTERN = /\bfinal\s*season\b/i;
const FINAL_SEASON_NUMBER = 9001;

// Season/cour markers observed across the real scrape that show up on
// season-collection titles without the literal word "batch" or a digit
// range, e.g. "... S1 - BD (1080p) ...", "... (Season 1) ...". Checked
// only after everything above, so an actual episode is never misread as
// a batch just because "S01" appears in the title too.
const BATCH_PATTERNS: RegExp[] = [/\bseason\s*\d+\b/i, /\bS\d{1,2}\b/, /\bcour\s*\d+\b/i, FINAL_SEASON_PATTERN];

// Finds a season number stated anywhere in the title, independent of
// where an episode number (if any) was found — so e.g. "Season 2 ...
// Batch" and a hypothetical "Batch ... Season 2" both resolve the same
// way. Defaults to 1 per real nyaa.si convention: a release with no season
// marker at all is either an explicitly single-season show or (per user
// input) a long-running, season-less show like One Piece/Naruto/Bleach —
// both cases are correctly "season 1" for grouping purposes.
const ORDINAL_SEASON_PATTERN = /\b(\d{1,2})(?:st|nd|rd|th)\s+season\b/i;

export function extractSeasonNumber(title: string): number {
  const seasonWordMatch = title.match(/\bseason\s*(\d+)/i);
  if (seasonWordMatch) return parseInt(seasonWordMatch[1], 10);
  // "2nd Season" / "3rd Season" (Erai-raws, SubsPlease and others). Without
  // this they fell through to season 1 and a sequel's episodes were filed
  // under the first season - verified live: Frieren S1 Episode 5 played
  // "[Erai-raws] Sousou no Frieren 2nd Season - 05".
  const ordinalMatch = title.match(ORDINAL_SEASON_PATTERN);
  if (ordinalMatch) return parseInt(ordinalMatch[1], 10);
  const sMatch = title.match(/\bS(\d{1,2})\b/i);
  if (sMatch) return parseInt(sMatch[1], 10);
  if (FINAL_SEASON_PATTERN.test(title)) return FINAL_SEASON_NUMBER;
  return 1;
}

// A season pack that also bundles other entries' content: "S01 + Whispers of
// Dawn", "S01+Movie", "... + TV Special". It holds the season's episodes but
// is a different (larger, mixed) torrent from a plain season pack, so it gets
// its own bucket instead of sharing "Batch" with them.
const BATCH_EXTRAS_PATTERN = /\bS\d{1,2}\s*\+\s*\S|\+\s*(?:movies?|films?|specials?|tv\s*specials?|ovas?|oads?|extras?|bonus)/i;

// `seasons`: every season the title names (set by the backend parser).
export type EpisodeLabel =
  | { kind: "episode"; season: number; number: number; seasons?: number[] }
  | { kind: "batch"; season: number; episodeRange: [number, number] | null; extras?: boolean; seasons?: number[] }
  | { kind: "movie" }
  | { kind: "special" }
  | { kind: "unknown" };

/** A release's or file's label: the one the backend parsed (release-parse,
 * attached to search results and play files), else this file's regex
 * parser on `text` (the browser preview has no backend). A copy, since
 * callers adjust episode numbers in place. */
export function labelOf(label: EpisodeLabel | undefined, text: string): EpisodeLabel {
  return label ? { ...label } : parseEpisode(text);
}

function batchLabel(title: string, withRange = true): Extract<EpisodeLabel, { kind: "batch" }> {
  const label: Extract<EpisodeLabel, { kind: "batch" }> = { kind: "batch", season: extractSeasonNumber(title), episodeRange: withRange ? extractEpisodeRange(title) : null };
  if (BATCH_EXTRAS_PATTERN.test(title)) label.extras = true;
  return label;
}

export function parseEpisode(title: string): EpisodeLabel {
  const seasonEpisodeMatch = title.match(SEASON_EPISODE_PATTERN);
  if (seasonEpisodeMatch) {
    const season = parseInt(seasonEpisodeMatch[1], 10);
    const number = parseInt(seasonEpisodeMatch[2], 10);
    if (!Number.isNaN(season) && !Number.isNaN(number)) {
      return { kind: "episode", season, number };
    }
  }
  if (BATCH_KEYWORD_PATTERN.test(title)) {
    return batchLabel(title);
  }
  const seasonDashMatch = title.match(SEASON_DASH_EPISODE_PATTERN);
  if (seasonDashMatch) {
    const number = parseInt(seasonDashMatch[1], 10);
    if (!Number.isNaN(number)) {
      return { kind: "episode", season: extractSeasonNumber(title), number };
    }
  }
  if (EPISODE_RANGE_PATTERN.test(title)) {
    return batchLabel(title);
  }
  const episodeNumber = findEpisodeNumber(title);
  if (episodeNumber !== null) {
    return { kind: "episode", season: extractSeasonNumber(title), number: episodeNumber };
  }
  for (const pattern of BATCH_PATTERNS) {
    if (pattern.test(title)) {
      return batchLabel(title, false);
    }
  }
  return { kind: "unknown" };
}

// Fansub group tag from the leading "[Name]" bracket, e.g. "[SubsPlease]
// Frieren - 21" -> "SubsPlease". Nyaa.si releases overwhelmingly lead with
// this. Verified against 5 real releases' actual nyaa.si "Submitter" field:
// 4/5 matched exactly (SubsPlease, Tsundere-Raws, 9volt, Judas); 1/5 was
// close but not identical ("[EMBER]" submitted as "Ember_Encodes"). So this
// is a fast, good-enough default for display — not authoritative the way a
// view-page scrape is — which is exactly why it's worth using to skip that
// scrape rather than being 100% precise.
export function parseSubmitterFromTitle(title: string): string | null {
  const match = title.match(/^\[([^\]]+)\]/);
  return match ? match[1].trim() : null;
}

export function episodeLabelText(label: EpisodeLabel): string {
  switch (label.kind) {
    case "episode":
      if (label.season === FINAL_SEASON_NUMBER) return `Final Season Episode ${label.number}`;
      // Season 1 is the overwhelmingly common case (either the show
      // genuinely has one season, or it's an unlabeled long-running show
      // like One Piece/Naruto/Bleach) — only call it out when it's not.
      return label.season === 1 ? `Episode ${label.number}` : `Season ${label.season} Episode ${label.number}`;
    case "batch":
      if (label.season === FINAL_SEASON_NUMBER) return label.extras ? "Final Season Batch + extras" : "Final Season Batch";
      return `${label.season === 1 ? "Batch" : `Season ${label.season} Batch`}${label.extras ? " + extras" : ""}`;
    case "movie":
      return "Movie";
    case "special":
      return "Specials";
    case "unknown":
      return "Unknown";
  }
}
