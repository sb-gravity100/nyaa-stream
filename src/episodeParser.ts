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

// Checked separately from EPISODE_PATTERNS below because it's the only
// pattern that also captures a season number. Allows an optional "v2"/"v3"
// revision suffix directly after the episode number (e.g. "S01E20v2") —
// without it the trailing \b boundary never matches because a digit
// directly followed by a letter has no word-boundary between them, so the
// whole title silently fell to "Unknown".
const SEASON_EPISODE_PATTERN = /\bS(\d{1,2})E(\d{1,4})(?:v\d+)?\b/i;

const EPISODE_PATTERNS: RegExp[] = [
  /\bEp(?:isode)?\.?\s*(\d{1,4})\b/i, // Episode 12 / Ep 12 / Ep.12
  / - (\d{1,4})(?:v\d)?\s*[[(]/, // "Title - 12 [1080p]" / "Title - 12 (v2)"
  / - (\d{1,4})(?:v\d)?\s*$/, // "Title - 12" at end of string
  /\bE(\d{1,4})(?:v\d+)?\b/i, // E12 / E12v2
];

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

// Season/cour markers observed across the real scrape that show up on
// season-collection titles without the literal word "batch" or a digit
// range, e.g. "... S1 - BD (1080p) ...", "... (Season 1) ...". Checked
// only after everything above, so an actual episode is never misread as
// a batch just because "S01" appears in the title too.
const BATCH_PATTERNS: RegExp[] = [/\bseason\s*\d+\b/i, /\bS\d{1,2}\b/, /\bcour\s*\d+\b/i];

// Finds a season number stated anywhere in the title, independent of
// where an episode number (if any) was found — so e.g. "Season 2 ...
// Batch" and a hypothetical "Batch ... Season 2" both resolve the same
// way. Defaults to 1 per real nyaa.si convention: a release with no season
// marker at all is either an explicitly single-season show or (per user
// input) a long-running, season-less show like One Piece/Naruto/Bleach —
// both cases are correctly "season 1" for grouping purposes.
function extractSeasonNumber(title: string): number {
  const seasonWordMatch = title.match(/\bseason\s*(\d+)/i);
  if (seasonWordMatch) return parseInt(seasonWordMatch[1], 10);
  const sMatch = title.match(/\bS(\d{1,2})\b/i);
  if (sMatch) return parseInt(sMatch[1], 10);
  return 1;
}

export type EpisodeLabel =
  | { kind: "episode"; season: number; number: number }
  | { kind: "batch"; season: number; episodeRange: [number, number] | null }
  | { kind: "unknown" };

export function parseEpisode(title: string): EpisodeLabel {
  const seasonEpisodeMatch = title.match(SEASON_EPISODE_PATTERN);
  if (seasonEpisodeMatch) {
    const season = parseInt(seasonEpisodeMatch[1], 10);
    const number = parseInt(seasonEpisodeMatch[2], 10);
    if (!Number.isNaN(season) && !Number.isNaN(number)) {
      return { kind: "episode", season, number };
    }
  }
  if (BATCH_KEYWORD_PATTERN.test(title) || EPISODE_RANGE_PATTERN.test(title)) {
    return { kind: "batch", season: extractSeasonNumber(title), episodeRange: extractEpisodeRange(title) };
  }
  for (const pattern of EPISODE_PATTERNS) {
    const match = title.match(pattern);
    if (match) {
      const number = parseInt(match[match.length - 1], 10);
      if (!Number.isNaN(number)) {
        return { kind: "episode", season: extractSeasonNumber(title), number };
      }
    }
  }
  for (const pattern of BATCH_PATTERNS) {
    if (pattern.test(title)) {
      return { kind: "batch", season: extractSeasonNumber(title), episodeRange: null };
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
      // Season 1 is the overwhelmingly common case (either the show
      // genuinely has one season, or it's an unlabeled long-running show
      // like One Piece/Naruto/Bleach) — only call it out when it's not.
      return label.season === 1 ? `Episode ${label.number}` : `Season ${label.season} Episode ${label.number}`;
    case "batch":
      return label.season === 1 ? "Batch" : `Season ${label.season} Batch`;
    case "unknown":
      return "Unknown";
  }
}
