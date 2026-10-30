//! Matching an AniList show to nyaa.si releases: which queries to send
//! (`build_candidates`) and which returned releases actually belong to the
//! show (`ShowMatcher`).
//!
//! nyaa.si's search is a per-word AND match that ignores punctuation, so:
//! - "Fate/strange Fake" and "Fate strange Fake" are the same query;
//! - a query whose words are a superset of another's returns a subset of that
//!   other query's results, so it never adds a release and only costs
//!   requests (which nyaa.si rate limits) - `build_candidates` drops it;
//! - any single-word overlap with an unrelated show matches, so results need
//!   the show's name checked against them afterwards.

use std::collections::BTreeSet;

use anilist_client::Franchise;
use nyaa_client::ToshoRelease;
use release_parse::Kind;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MatchCriterion {
    pub criterion: String,
    pub passed: bool,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchEvaluation {
    pub matched: bool,
    pub reason: String,
    pub checks: Vec<MatchCriterion>,
}

fn add_match_check(
    checks: &mut Vec<MatchCriterion>,
    collect: bool,
    criterion: &str,
    passed: bool,
    evidence: impl FnOnce() -> String,
) {
    if collect {
        checks.push(MatchCriterion { criterion: criterion.into(), passed, evidence: evidence() });
    }
}

/// What the dataset's AniDB id says about a release for one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Belongs to the entry whatever its title says.
    Accept,
    /// Belongs to another entry whatever its title says.
    Reject,
    /// No usable id: the title rules decide.
    UseTitleRules,
}

/// Most queries sent per show - each is at least one nyaa.si request.
const MAX_CANDIDATES: usize = 4;
/// Shortest normalized synonym worth its own query: short aliases match far
/// too many unrelated releases.
const MIN_SYNONYM_LEN: usize = 5;
/// Shortest normalized base name (the short form before a season marker).
const MIN_BASE_NAME_LEN: usize = 5;
const MAX_SEARCH_NAMES: usize = 12;
const MAX_SEARCH_BASES: usize = 6;
const MAX_QUERY_PHRASES: usize = 32;
const MAX_REMOTE_QUERIES: usize = 16;
const MAX_QUERY_ALTERNATIVES: usize = 8;
const MAX_QUERY_CHARS: usize = 600;

/// Apostrophes and their lookalikes: dropped, not separators, so "Journey's"
/// and "Journeys" both become `journeys` (groups write either).
fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '\u{02BC}' | '`' | '\u{00B4}')
}

/// Lowercased words of `text` with all punctuation (`/ : - ! ☆`...) as
/// separators - the way nyaa.si compares titles - except apostrophes, which
/// are removed (`God's` -> `gods`).
pub fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !is_apostrophe(*c))
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokens(text: &str) -> BTreeSet<String> {
    normalize(text).split(' ').filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// Whether `text` is written in Latin script (release titles are searched by
/// Latin names), as opposed to e.g. a Japanese synonym.
fn is_latin(text: &str) -> bool {
    let mut letters = text.chars().filter(|c| c.is_alphabetic()).peekable();
    letters.peek().is_some() && letters.all(|c| c <= '\u{024F}')
}

/// Strips a trailing "Season N"/"Nth Season"/"Part N"/"Final Season"
/// qualifier off an AniList title, e.g. "That Time I Got Reincarnated as a
/// Slime Season 4" -> Some("That Time I Got Reincarnated as a Slime").
/// nyaa.si's search is per-word AND-matching, so a query carrying literal
/// "Season 4" only matches releases whose *title text* also contains
/// "Season" and "4" as separate words. Verified live: ToonsHub numbers this
/// exact show "S04E21" (no "Season" token at all), so the full-title query
/// returns zero of its ~100 real Season 4 releases even though a plain
/// "Slime" search finds every one of them on the first page. Returns None
/// when no such suffix is found.
pub fn strip_season_suffix(title: &str) -> Option<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        let lower = word.trim_end_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if lower == "season" || lower == "cour" {
            // A leading ordinal/number ("4th Season", "2nd Season") or
            // "Final" (e.g. "Final Season") belongs to the suffix too, not
            // the show's own name - cut before it.
            let prev_is_qualifier = i > 0
                && (words[i - 1].chars().next().is_some_and(|c| c.is_ascii_digit()) || words[i - 1].eq_ignore_ascii_case("final"));
            let cut = if prev_is_qualifier { i - 1 } else { i };
            if cut == 0 {
                return None; // "Season" is the whole title - nothing to strip.
            }
            return Some(words[..cut].join(" "));
        }
        if lower == "part" && i > 0 {
            return Some(words[..i].join(" "));
        }
    }
    None
}

/// Drops a trailing roman-numeral season ("Mob Psycho 100 III" -> "Mob
/// Psycho 100"). None when there is none, or it would leave a single word
/// ("Rocky V" is not season five of "Rocky").
fn strip_roman_suffix(title: &str) -> Option<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    let last = words.last()?.trim_end_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    (words.len() > 2 && is_roman_numeral(&last)).then(|| words[..words.len() - 1].join(" "))
}

/// `title` plus its forms without a season qualifier or roman numeral
/// ("... 3rd Season", "... III"), because groups write the bare name.
fn season_free_variants(title: &str) -> Vec<String> {
    let mut variants = vec![title.to_string()];
    variants.extend(strip_season_suffix(title));
    let romans: Vec<String> = variants.iter().filter_map(|v| strip_roman_suffix(v)).collect();
    variants.extend(romans);
    variants
}

/// Every name a release of this show might carry: full titles, with the
/// season qualifier stripped, and Latin-script synonyms. Subtitles are kept:
/// they tell sibling entries apart ("Fate/strange Fake" vs its "Whispers of
/// Dawn" special), which a stripped query would merge.
fn names(english: Option<&str>, romaji: Option<&str>, synonyms: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let latin_synonyms = synonyms.iter().filter(|s| is_latin(s) && normalize(s).len() >= MIN_SYNONYM_LEN);
    for title in [english, romaji].into_iter().flatten().chain(latin_synonyms.map(String::as_str)) {
        all.extend(season_free_variants(title));
    }
    let mut seen = BTreeSet::new();
    all.retain(|name| {
        let key = normalize(name);
        !key.is_empty() && name.chars().count() <= 120 && seen.insert(key)
    });
    all.truncate(MAX_SEARCH_NAMES);
    all
}

/// One nyaa.si query expression. Positive phrases are alternatives; exclusion
/// phrases form one negated OR group, matching nyaa.si's search syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NyaaSearchQuery {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl NyaaSearchQuery {
    pub fn render(&self) -> String {
        self.render_with(None)
    }

    /// Adds an uploader tag to each OR branch (not just the first branch).
    pub fn render_with_required_term(&self, term: &str) -> String {
        self.render_with(Some(term))
    }

    fn render_with(&self, required: Option<&str>) -> String {
        let positives = self
            .includes
            .iter()
            .filter_map(|phrase| quote_phrase(phrase))
            .map(|phrase| required.map_or_else(|| phrase.clone(), |term| format!("{term} {phrase}")))
            .collect::<Vec<_>>()
            .join("|");
        let negatives = self.excludes.iter().filter_map(|phrase| quote_phrase(phrase)).collect::<Vec<_>>().join("|");
        if negatives.is_empty() {
            positives
        } else {
            format!("{positives} -{negatives}")
        }
    }
}

/// Keep nyaa.si expressions short enough for its search endpoint. Exclusions
/// are only a remote-search optimization; ShowMatcher remains authoritative.
fn split_search_query(query: NyaaSearchQuery) -> Vec<NyaaSearchQuery> {
    // A first-season franchise can produce a long exclusion list. Bound it
    // against the longest positive phrase so even a one-phrase chunk fits.
    // Omitted exclusions can add noise to search results, but local matching
    // removes other-season releases after the requests complete.
    let longest = query.includes.iter().max_by_key(|phrase| phrase.chars().count()).cloned();
    let mut excludes = Vec::new();
    if let Some(longest) = longest {
        for exclusion in query.excludes {
            let mut candidate = excludes.clone();
            candidate.push(exclusion.clone());
            let rendered = NyaaSearchQuery { includes: vec![longest.clone()], excludes: candidate.clone() }.render();
            if rendered.chars().count() > MAX_QUERY_CHARS {
                break;
            }
            excludes = candidate;
        }
    }

    let mut chunks = Vec::new();
    let mut includes = Vec::new();
    for phrase in query.includes {
        let mut candidate = includes.clone();
        candidate.push(phrase.clone());
        let rendered = NyaaSearchQuery { includes: candidate, excludes: excludes.clone() }.render();
        if !includes.is_empty() && (includes.len() >= MAX_QUERY_ALTERNATIVES || rendered.chars().count() > MAX_QUERY_CHARS) {
            chunks.push(NyaaSearchQuery { includes: std::mem::take(&mut includes), excludes: excludes.clone() });
        }
        includes.push(phrase);
    }
    if !includes.is_empty() {
        chunks.push(NyaaSearchQuery { includes, excludes });
    }
    chunks
}

fn quote_phrase(text: &str) -> Option<String> {
    let safe = text.chars().filter(|c| !matches!(c, '"' | '|' | '(' | ')')).collect::<String>();
    let phrase = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    (!phrase.is_empty()).then(|| format!("\"{phrase}\""))
}

fn raw_names(english: Option<&str>, romaji: Option<&str>, synonyms: &[String]) -> Vec<String> {
    let mut all: Vec<String> = [english, romaji]
        .into_iter()
        .flatten()
        .map(str::to_string)
        .chain(synonyms.iter().filter(|s| is_latin(s) && normalize(s).len() >= MIN_SYNONYM_LEN).cloned())
        .collect();
    let mut seen = BTreeSet::new();
    all.retain(|name| {
        let key = normalize(name);
        !key.is_empty() && seen.insert(key)
    });
    all
}

fn search_bases(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], franchise: Option<&Franchise>, seasons: &BTreeSet<u32>, split_entry: bool) -> Vec<String> {
    let mut bases = Vec::new();
    for title in raw_names(english, romaji, synonyms) {
        for variant in season_free_variants(&title) {
            if let Some(base) = base_title(&variant) {
                if !bases.iter().any(|old: &String| normalize(old) == normalize(&base)) {
                    bases.push(base);
                }
            }
        }
    }
    if let Some(franchise) = franchise {
        for base in franchise_base_names(english, romaji, synonyms, franchise) {
            if !bases.iter().any(|old| normalize(old) == normalize(&base)) {
                bases.push(base);
            }
        }
        for relative in franchise.relatives.iter().filter(|relative| is_tv(relative.format.as_deref())) {
            let sibling_seasons = entry_seasons(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms, None);
            let same_season = sibling_seasons.iter().any(|season| seasons.contains(season))
                || (split_entry && relative.relation == "PREQUEL" && sibling_seasons.is_empty());
            if !same_season {
                continue;
            }
            for title in raw_names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms) {
                for variant in season_free_variants(&title) {
                    if let Some(base) = base_title(&variant) {
                        if !bases.iter().any(|old| normalize(old) == normalize(&base)) {
                            bases.push(base);
                        }
                    }
                }
            }
        }
    }
    bases.truncate(MAX_SEARCH_BASES);
    bases
}

fn roman_season(season: u32) -> Option<&'static str> {
    Some(match season {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        8 => "VIII",
        9 => "IX",
        10 => "X",
        _ => return None,
    })
}

fn ordinal_season(season: u32) -> String {
    let suffix = match season % 100 {
        11..=13 => "th",
        _ => match season % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{season}{suffix} Season")
}

fn season_markers(season: u32) -> Vec<String> {
    let mut markers = vec![format!("S{season}"), format!("S{season:02}"), format!("Season {season}"), ordinal_season(season)];
    if let Some(roman) = roman_season(season) {
        markers.push(roman.to_string());
    }
    markers
}

/// Builds remote nyaa.si queries with exact phrase alternatives and explicit
/// season markers. Local database lookups keep using `build_candidates`, whose
/// plain words match the store's AND-search semantics.
pub fn build_search_queries(
    english: Option<&str>,
    romaji: Option<&str>,
    synonyms: &[String],
    tvdb_season: Option<u32>,
    franchise: Option<&Franchise>,
) -> Vec<NyaaSearchQuery> {
    let entry_seasons = entry_seasons(english, romaji, synonyms, tvdb_season);
    let matcher = ShowMatcher::new(english, romaji, synonyms, tvdb_season);
    let matcher = match franchise {
        Some(franchise) => matcher.with_franchise(franchise),
        None => matcher,
    };
    let primary = matcher.primary_season();
    let is_tv_entry = franchise
        .and_then(|f| f.format.as_deref())
        .map(|format| is_tv(Some(format)))
        .unwrap_or(tvdb_season.is_some() || !entry_seasons.is_empty());
    let later_season = is_tv_entry && primary.is_some_and(|season| season > 1);
    let mut exact_names = raw_names(english, romaji, synonyms);

    if later_season {
        if let Some(franchise) = franchise {
            let sibling_tv_names: BTreeSet<String> = franchise
                .relatives
                .iter()
                .filter(|r| is_tv(r.format.as_deref()))
                .flat_map(|r| names(r.title.english.as_deref(), r.title.romaji.as_deref(), &r.synonyms))
                .map(|name| normalize(&name))
                .collect();
            if sibling_tv_names.is_empty() {
                exact_names.retain(|name| !release_seasons(name).is_empty());
            } else {
                exact_names.retain(|name| !sibling_tv_names.contains(&normalize(name)));
            }
        } else {
            exact_names.retain(|name| !release_seasons(name).is_empty());
        }
    }

    let mut queries = Vec::new();
    let first_season = is_tv_entry && primary == Some(1);
    let mut seasons = if first_season { BTreeSet::from([1]) } else { entry_seasons.clone() };
    seasons.extend(primary.filter(|season| !later_season || *season > 1));
    if later_season {
        seasons.retain(|season| *season > 1);
    }
    let split_entry = raw_names(english, romaji, synonyms).iter().any(|name| is_split_part_name(&normalize(name)));
    let bases = if is_tv_entry { search_bases(english, romaji, synonyms, franchise, &seasons, split_entry) } else { Vec::new() };
    let exclusions = if first_season { sibling_exclusion_phrases(&bases, franchise) } else { Vec::new() };

    if !exact_names.is_empty() {
        queries.push(NyaaSearchQuery { includes: exact_names, excludes: exclusions.clone() });
    }

    if is_tv_entry {
        let phrases = bases
            .iter()
            .flat_map(|base| seasons.iter().flat_map(move |season| season_markers(*season).into_iter().map(move |marker| format!("{base} {marker}"))))
            .collect::<Vec<_>>();
        if !phrases.is_empty() {
            let phrases = phrases.into_iter().take(MAX_QUERY_PHRASES).collect();
            queries.push(NyaaSearchQuery { includes: phrases, excludes: Vec::new() });
        }
    }

    let queries = queries.into_iter().flat_map(split_search_query).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let queries = queries.into_iter().filter(|query| seen.insert(query.render())).collect::<Vec<_>>();
    tracing::debug!(
        query_count = queries.len(),
        query_chars = ?queries.iter().map(|query| query.render().chars().count()).collect::<Vec<_>>(),
        alternatives = ?queries.iter().map(|query| query.includes.len()).collect::<Vec<_>>(),
        primary,
        is_tv_entry,
        "built entry-specific nyaa queries"
    );
    queries.into_iter().take(MAX_REMOTE_QUERIES).collect()
}

fn sibling_exclusion_phrases(bases: &[String], franchise: Option<&Franchise>) -> Vec<String> {
    let mut seasons = BTreeSet::new();
    let mut titles = Vec::new();
    if let Some(franchise) = franchise {
        let tv_siblings: Vec<_> = franchise.relatives.iter().filter(|r| is_tv(r.format.as_deref())).collect();
        for relative in &tv_siblings {
            seasons.extend(entry_seasons(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms, None).into_iter().filter(|season| *season > 1));
            titles.extend(raw_names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms));
            titles.extend(names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms));
        }
        seasons.extend(2..=(tv_siblings.len() as u32 + 1).max(2));
    }
    for base in bases {
        for season in &seasons {
            titles.extend(season_markers(*season).into_iter().map(|marker| format!("{base} {marker}")));
        }
    }
    let own_bases: BTreeSet<String> = bases.iter().map(|base| normalize(base)).collect();
    let mut seen = BTreeSet::new();
    titles.retain(|title| {
        let key = normalize(title);
        !key.is_empty() && !own_bases.contains(&key) && seen.insert(key)
    });
    titles.truncate(MAX_QUERY_PHRASES);
    titles
}

/// The nyaa.si queries to run: the show's names minus any whose words are a
/// superset of another's (see the module comment), fewest words first,
/// capped at `MAX_CANDIDATES`.
///
/// For a season 2+ entry the short base name ("Mushoku Tensei") joins the
/// names: groups write it with a season marker, and its words are a subset of
/// the full names', so those are dropped and the query count doesn't grow.
pub fn build_candidates(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], tvdb_season: Option<u32>, franchise: Option<&Franchise>) -> Vec<String> {
    let mut all = names(english, romaji, synonyms);
    if let Some(franchise) = franchise.filter(|f| f.has_tv_prequel) {
        // "Food Wars! The Third Plate" -> "Food Wars!": the bare name groups use.
        all.extend(franchise_base_names(english, romaji, synonyms, franchise));
    }
    if entry_seasons(english, romaji, synonyms, tvdb_season).iter().any(|season| *season >= 2) {
        for title in [english, romaji].into_iter().flatten() {
            let unseasoned = strip_season_suffix(title);
            all.extend(unseasoned.as_deref().into_iter().chain(std::iter::once(title)).filter_map(base_title));
        }
        let mut seen = BTreeSet::new();
        all.retain(|name| seen.insert(normalize(name)));
    }
    let token_sets: Vec<BTreeSet<String>> = all.iter().map(|name| tokens(name)).collect();
    let mut kept: Vec<(usize, &String)> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| !token_sets.iter().enumerate().any(|(j, other)| j != *i && other.is_subset(&token_sets[*i])))
        .collect();
    // Titles first (english, romaji), synonyms only if there's room: `all`
    // is already in that order, and the sort is stable.
    kept.sort_by_key(|(i, _)| token_sets[*i].len());
    kept.into_iter().take(MAX_CANDIDATES).map(|(_, name)| name.clone()).collect()
}

/// Seasons a release title names (ascending; empty = no season marker). An
/// `SxxEyy` marker outranks other markers and multi-season packs give every
/// season: both are `release-parse` rules, so the matcher and the frontend
/// labels can't disagree.
pub fn release_seasons(release_title: &str) -> Vec<u32> {
    let seasons = release_parse::parse_release(release_title).seasons;
    tracing::trace!(release_title, ?seasons, "release seasons");
    seasons
}

/// Season numbers an AniList entry goes by, read from its own titles
/// ("Season 3", "3rd Season", "Mushoku Tensei III", "Final Season").
/// plus its TVDB season (`anime_lists`). Titles that name no season add
/// nothing; the prequel-chain fallback joins this set in a later step.
pub fn entry_seasons(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], tvdb_season: Option<u32>) -> BTreeSet<u32> {
    let seasons: BTreeSet<u32> = [english, romaji]
        .into_iter()
        .flatten()
        .chain(synonyms.iter().map(String::as_str))
        .flat_map(release_seasons)
        .chain(tvdb_season)
        .collect();
    tracing::debug!(?seasons, "entry seasons from titles");
    seasons
}

/// `text` as a normalized haystack: words padded with spaces, so a phrase is
/// found only on word boundaries.
fn haystack(text: &str) -> String {
    format!(" {} ", normalize(text))
}

/// Whether a normalized release title says it is a movie, OVA/OAD or
/// special - release-parse misses "the Movie: Tears of the Azure Sea".
fn names_extra(haystack: &str) -> bool {
    ["movie", "movies", "ova", "ovas", "oad", "oads", "special", "specials"].iter().any(|word| haystack.contains(&format!(" {word} ")))
}

/// Whether a normalized release title says it holds every season ("Complete
/// Series+OVAs", "Full Series", "All Seasons"), whatever markers it lacks.
fn names_whole_series(haystack: &str) -> bool {
    ["complete series", "complete collection", "full series", "all seasons", "complete season", "complete batch"].iter().any(|phrase| haystack.contains(&format!(" {phrase}")))
}

fn is_tv(format: Option<&str>) -> bool {
    matches!(format, Some("TV" | "TV_SHORT"))
}

fn is_roman_numeral(word: &str) -> bool {
    matches!(word, "ii" | "iii" | "iv" | "v" | "vi" | "vii" | "viii" | "ix" | "x")
}

/// The short name groups write before a season marker: the part of a title
/// before the first `:`, a trailing roman numeral removed ("Mushoku Tensei
/// III: Isekai Ittara Honki Dasu" -> "mushoku tensei"). None when it is under
/// 2 words or 5 characters, because shorter names match unrelated shows.
fn base_name(title: &str) -> Option<String> {
    base_title(title).map(|base| normalize(&base))
}

/// `base_name` in the title's own spelling, usable as a search query.
fn base_title(title: &str) -> Option<String> {
    let head = title.split(':').next().unwrap_or(title);
    let mut words: Vec<&str> = head.split_whitespace().collect();
    if words.len() > 1 && words.last().is_some_and(|w| is_roman_numeral(&normalize(w))) {
        words.pop();
    }
    let base = words.join(" ");
    let normalized = normalize(&base);
    (normalized.split(' ').count() >= 2 && normalized.len() >= MIN_BASE_NAME_LEN).then_some(base)
}

/// Whether a normalized title is a later part of a season ("... Part 2",
/// "2nd Cour", "Cour III"): it shares its season with the entry before it.
fn is_split_part_name(normalized: &str) -> bool {
    let words: Vec<&str> = normalized.split(' ').collect();
    words.windows(2).any(|pair| {
        let [a, b] = [pair[0], pair[1]];
        let later = |w: &str| w.parse::<u32>().is_ok_and(|n| n >= 2) || roman_value(w).is_some();
        ((a == "part" || a == "cour") && later(b)) || ((b == "part" || b == "cour") && a.trim_end_matches(|c: char| c.is_alphabetic()).parse::<u32>().is_ok_and(|n| n >= 2))
    })
}

fn is_split_part(english: &Option<String>, romaji: &Option<String>) -> bool {
    [english, romaji].into_iter().flatten().any(|title| is_split_part_name(&normalize(title)))
}

fn roman_value(word: &str) -> Option<u32> {
    Some(match word {
        "ii" => 2,
        "iii" => 3,
        "iv" => 4,
        "v" => 5,
        "vi" => 6,
        "vii" => 7,
        "viii" => 8,
        "ix" => 9,
        "x" => 10,
        _ => return None,
    })
}

/// Words that end a shared name prefix without being part of the name.
fn is_season_word(word: &str) -> bool {
    matches!(word, "season" | "cour" | "part" | "final" | "the") || word.chars().all(|c| c.is_ascii_digit()) || is_roman_numeral(word)
}

/// The short names a later entry shares with the rest of its franchise: the
/// leading words its titles have in common with a relative's ("Food Wars!
/// The Third Plate" and "Food Wars! Shokugeki no Soma" -> "food wars"),
/// minus trailing season words. Groups write that bare name with a season
/// marker, though no title of the entry says it. Normalized; needs 2 words
/// and `MIN_BASE_NAME_LEN` characters, like `base_name`.
pub fn franchise_base_names(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], franchise: &Franchise) -> Vec<String> {
    let own: Vec<String> = names(english, romaji, synonyms).iter().map(|name| normalize(name)).collect();
    shared_bases(&own, franchise)
}

/// `franchise_base_names` for already normalized names.
fn shared_bases(own: &[String], franchise: &Franchise) -> Vec<String> {
    shared_bases_for(own, &franchise.relatives)
}

fn shared_bases_for(own: &[String], relatives: &[anilist_client::Relative]) -> Vec<String> {
    let mut bases: Vec<String> = Vec::new();
    for relative in relatives {
        for title in [relative.title.english.as_deref(), relative.title.romaji.as_deref()].into_iter().flatten() {
            let theirs = normalize(title);
            let theirs: Vec<&str> = theirs.split(' ').collect();
            for name in own {
                let ours: Vec<&str> = name.split(' ').collect();
                let mut shared = ours.iter().zip(&theirs).take_while(|(a, b)| a == b).count();
                while shared > 0 && is_season_word(ours[shared - 1]) {
                    shared -= 1;
                }
                let base = ours[..shared].join(" ");
                // Only a proper prefix of the entry's own name is a shorter form of it.
                if shared >= 2 && shared < ours.len() && base.len() >= MIN_BASE_NAME_LEN && !bases.contains(&base) {
                    bases.push(base);
                }
            }
        }
    }
    tracing::debug!(?bases, "franchise base names");
    bases
}

/// Decides which returned releases belong to one AniList entry: its names
/// plus its season numbers, so a release naming only other
/// seasons is dropped and a short name with this entry's season marker is
/// accepted (see PLAN.md "Season-aware source matching").
pub struct ShowMatcher {
    names: Vec<String>,
    /// Original AniList names before removing season words. A full distinctive
    /// entry title can identify an unmarked release when franchise data is not
    /// available.
    identity_names: Vec<String>,
    bases: Vec<String>,
    seasons: BTreeSet<u32>,
    /// The entry's AniList format (TV, MOVIE, OVA, ...), once known.
    format: Option<String>,
    /// Normalized names of the franchise's other entries.
    siblings: Vec<Vec<String>>,
    /// TV-relative names from an entry in the same season (for aliases absent
    /// from the selected AniList title, such as a translated season name).
    same_season_siblings: Vec<Vec<String>>,
    /// TV-relative names that identify a different season.
    season_siblings: Vec<Vec<String>>,
    /// Normalized short names shared with franchise relatives.
    shared_bases: Vec<String>,
    /// Short names shared specifically with a different TV season.
    shared_season_bases: Vec<String>,
    /// The one season the entry is filed under: TVDB's, else the one its
    /// titles name, else 1 for a first season. None when unknown.
    primary: Option<u32>,
}

impl ShowMatcher {
    pub fn new(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], tvdb_season: Option<u32>) -> Self {
        let titles = names(english, romaji, synonyms);
        let identity_names = raw_names(english, romaji, synonyms).iter().map(|name| normalize(name)).collect();
        let mut bases: Vec<String> = Vec::new();
        for title in [english, romaji].into_iter().flatten() {
            let unseasoned = strip_season_suffix(title);
            for candidate in unseasoned.as_deref().into_iter().chain(std::iter::once(title)) {
                if let Some(base) = base_name(candidate) {
                    if !bases.contains(&base) {
                        bases.push(base);
                    }
                }
            }
        }
        let matcher = ShowMatcher { names: titles.iter().map(|name| normalize(name)).collect(), identity_names, bases, seasons: entry_seasons(english, romaji, synonyms, tvdb_season), format: None, siblings: Vec::new(), same_season_siblings: Vec::new(), season_siblings: Vec::new(), shared_bases: Vec::new(), shared_season_bases: Vec::new(), primary: tvdb_season.or_else(|| entry_seasons(english, romaji, synonyms, None).iter().next().copied()) };
        tracing::debug!(names = ?matcher.names, bases = ?matcher.bases, seasons = ?matcher.seasons, "show matcher built");
        matcher
    }

    /// Adds what the franchise says: a TV entry with no TV prequel is season
    /// 1, the entry's format keeps movies/OVAs/specials off a TV season's
    /// list, and the other entries' names let a release that names a sibling
    /// more specifically go to it.
    pub fn with_franchise(mut self, franchise: &Franchise) -> Self {
        self.format = franchise.format.clone();
        if is_tv(self.format.as_deref()) && !franchise.has_tv_prequel {
            self.seasons.insert(1);
            self.primary.get_or_insert(1);
        }
        // No TVDB season and none in the titles: the number of earlier
        // TV seasons. Only a display/pick hint (never in `seasons`, which
        // rejects releases): split cours ("Part 2") count as one season.
        if self.primary.is_none() && is_tv(self.format.as_deref()) && franchise.has_tv_prequel {
            let earlier = franchise
                .relatives
                .iter()
                .filter(|r| r.relation == "PREQUEL" && is_tv(r.format.as_deref()) && !is_split_part(&r.title.english, &r.title.romaji))
                .count() as u32;
            let own_part = self.names.iter().any(|name| is_split_part_name(name));
            self.primary = Some(if own_part { earlier } else { earlier + 1 }.max(1));
        }
        self.shared_bases = shared_bases(&self.names, franchise);
        for base in &self.shared_bases {
            if !self.bases.contains(&base) {
                self.bases.push(base.clone());
            }
        }
        self.siblings = franchise
            .relatives
            .iter()
            .map(|relative| names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms).iter().map(|name| normalize(name)).collect())
            .collect();
        let mut own_seasons = self.seasons.clone();
        own_seasons.extend(self.primary);
        let own_split = self.names.iter().any(|name| is_split_part_name(name));
        let mut different_season_relatives = Vec::new();
        self.same_season_siblings.clear();
        self.season_siblings.clear();
        for relative in franchise.relatives.iter().filter(|relative| is_tv(relative.format.as_deref())) {
            let sibling_seasons = entry_seasons(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms, None);
            let sibling_names: Vec<String> = names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms)
                .iter()
                .map(|name| normalize(name))
                .collect();
            let same_season = sibling_seasons.iter().any(|season| own_seasons.contains(season))
                || (own_split && relative.relation == "PREQUEL" && sibling_seasons.is_empty());
            if same_season {
                self.same_season_siblings.push(sibling_names);
            } else {
                self.season_siblings.push(sibling_names);
                different_season_relatives.push(relative.clone());
            }
        }
        self.shared_season_bases = shared_bases_for(&self.names, &different_season_relatives);
        tracing::debug!(format = ?self.format, seasons = ?self.seasons, siblings = self.siblings.len(), same_season_siblings = self.same_season_siblings.len(), shared_bases = ?self.shared_bases, "show matcher given its franchise");
        self
    }

    /// The season a roman numeral right after one of the short names gives.
    fn roman_season(&self, haystack: &str) -> Option<u32> {
        self.bases.iter().find_map(|base| {
            let pattern = format!(" {base} ");
            let rest = &haystack[haystack.find(&pattern)? + pattern.len()..];
            roman_value(rest.split(' ').next()?)
        })
    }

    /// Words in the longest of `names` found in `haystack` (0 = none).
    fn longest_match(haystack: &str, names: &[String]) -> usize {
        names.iter().filter(|name| haystack.contains(&format!(" {name} "))).map(|name| name.split(' ').count()).max().unwrap_or(0)
    }

    /// The season the entry is filed under (see `primary`).
    pub fn primary_season(&self) -> Option<u32> {
        self.primary
    }

    /// The season a roman numeral after this entry's short name gives an
    /// otherwise unmarked release ("Mushoku Tensei III - 01" -> 3). `release-parse`
    /// cannot know names, so its label says season 1 for these; the caller
    /// corrects the label with this. None when the title has a season marker
    /// of its own or no such numeral.
    pub fn roman_season_of(&self, release_title: &str) -> Option<u32> {
        if !release_seasons(release_title).is_empty() {
            return None;
        }
        self.roman_season(&haystack(release_title))
    }

    /// Whether an unmarked release only names what a franchise sibling also
    /// uses. Without a season marker, a shared title is ambiguous on any
    /// season page. A trusted AniDB id or an unshared full title identifies the
    /// selected entry.
    pub fn season_ambiguous(&self, release_title: &str) -> bool {
        let has_season_context = self.primary.is_some_and(|season| season > 1) || !self.season_siblings.is_empty() || !self.shared_season_bases.is_empty();
        if !has_season_context {
            return false;
        }
        let haystack = haystack(release_title);
        if !self.named_seasons(release_title, &haystack).is_empty() || names_whole_series(&haystack) {
            return false;
        }
        let found: Vec<&String> = self.names.iter().filter(|name| haystack.contains(&format!(" {name} "))).collect();
        if found.is_empty() {
            let ambiguous = Self::longest_match(&haystack, &self.shared_season_bases) > 0;
            tracing::trace!(release_title, ambiguous, "unmarked release matches a different-season franchise base");
            return ambiguous;
        }
        let ambiguous = if self.season_siblings.is_empty() {
            if self.same_season_siblings.is_empty() {
                !found.iter().any(|name| self.identity_names.contains(name))
            } else {
                false
            }
        } else {
            found.iter().all(|name| self.season_siblings.iter().any(|siblings| siblings.contains(name)))
        };
        tracing::trace!(release_title, ambiguous, "unmarked release shares a different-season franchise title");
        ambiguous
    }

    /// The season numbers `release_title` names (explicit markers, else a
    /// roman numeral right after a known short name).
    fn named_seasons(&self, release_title: &str, haystack: &str) -> Vec<u32> {
        let mut release_seasons = release_seasons(release_title);
        // "Mushoku Tensei II Isekai ..." - a roman numeral right after a
        // known short name is its season (release-parse can't know names).
        if release_seasons.is_empty() {
            release_seasons.extend(self.roman_season(haystack));
        }
        release_seasons
    }

    /// Whether the release's explicit season markers name only other seasons
    /// than this entry's (never with no season known for the entry).
    fn names_only_other_seasons(&self, release_seasons: &[u32]) -> bool {
        !self.seasons.is_empty() && !release_seasons.is_empty() && !release_seasons.iter().any(|season| self.seasons.contains(season))
    }

    /// The decision for a release whose AniDB id may be known (see
    /// `Decision` and PLAN.md "AnimeTosho dataset"). `entry_aids` is the
    /// entry's AniDB id set; empty (unmapped entry) never decides.
    pub fn decide(&self, release_title: &str, known: Option<&ToshoRelease>, entry_aids: &[u32]) -> Decision {
        let Some(known) = known.filter(|k| k.aid != 0) else { return Decision::UseTitleRules };
        if entry_aids.is_empty() {
            return Decision::UseTitleRules;
        }
        let ours = entry_aids.contains(&known.aid);
        let decision = match (known.approved, ours) {
            (1..=u8::MAX, true) => Decision::Accept,
            (1..=u8::MAX, false) => Decision::Reject,
            // AnimeTosho's own guess: a hint only. It vouches for the release
            // unless the title explicitly names only other seasons; it never
            // rejects on its own.
            (0, true) => {
                let hay = haystack(release_title);
                if self.names_only_other_seasons(&self.named_seasons(release_title, &hay)) || self.season_ambiguous(release_title) {
                    Decision::UseTitleRules
                } else {
                    Decision::Accept
                }
            }
            (0, false) => Decision::UseTitleRules,
        };
        tracing::trace!(release_title, aid = known.aid, approved = known.approved, ours, ?decision, "aid decision");
        decision
    }

    /// `matches`, with the dataset's AniDB id deciding where it can
    /// (`filter_for_show` in lib.rs does the same per result).
    #[cfg(test)]
    pub fn matches_with_aid(&self, release_title: &str, known: Option<&ToshoRelease>, entry_aids: &[u32]) -> bool {
        match self.decide(release_title, known, entry_aids) {
            Decision::Accept => true,
            Decision::Reject => false,
            Decision::UseTitleRules => self.matches(release_title),
        }
    }

    /// Whether `release_title` belongs to this entry.
    pub fn matches(&self, release_title: &str) -> bool {
        self.evaluate(release_title, false).matched
    }

    /// The same title/season rules as `matches`, with the checks that
    /// determined the result for the developer search analyzer.
    pub fn explain(&self, release_title: &str) -> MatchEvaluation {
        self.evaluate(release_title, true)
    }

    fn evaluate(&self, release_title: &str, collect: bool) -> MatchEvaluation {
        let mut checks = Vec::new();
        let haystack = haystack(release_title);
        let release_seasons = self.named_seasons(release_title, &haystack);
        add_match_check(&mut checks, collect, "Season compatibility", !self.names_only_other_seasons(&release_seasons), || {
            if release_seasons.is_empty() {
                format!("No explicit season marker; entry seasons: {:?}", self.seasons)
            } else {
                format!("Release seasons {:?}; entry seasons {:?}", release_seasons, self.seasons)
            }
        });
        // Markers that name only other seasons rule the release out, even
        // when a full name matches ("Slime" names every season). With no
        // season known for the entry nothing is ruled out.
        if self.names_only_other_seasons(&release_seasons) {
            tracing::trace!(release_title, ?release_seasons, entry_seasons = ?self.seasons, "rejected: other season");
            return MatchEvaluation { matched: false, reason: "Release names another season".into(), checks };
        }
        let ambiguous = self.season_ambiguous(release_title);
        add_match_check(&mut checks, collect, "Franchise title ambiguity", !ambiguous, || {
            if ambiguous { "Unmarked title is shared with a different-season franchise entry" } else { "No conflicting shared-title evidence" }.into()
        });
        if ambiguous {
            tracing::trace!(release_title, entry_season = ?self.primary, "rejected: unmarked title is shared with an earlier season");
            return MatchEvaluation { matched: false, reason: "Unmarked title is ambiguous across seasons".into(), checks };
        }
        // Movies, OVAs and specials are filed under their own entry, not
        // a TV season's episode list.
        // A pack naming several seasons may bundle them ("S01-S05 + OVA").
        let parsed = release_parse::parse_release(release_title);
        // (A season pack that bundles extras - "S01 + OVA" - is still its season's.)
        let tv_content_ok = !is_tv(self.format.as_deref())
            || release_seasons.len() >= 2
            || !(matches!(parsed.kind, Kind::Movie | Kind::Special)
                || (names_extra(&haystack) && !parsed.extras && !names_whole_series(&haystack)));
        add_match_check(&mut checks, collect, "Release type fits this entry", tv_content_ok, || {
            if is_tv(self.format.as_deref()) {
                format!("Entry format {:?}; parsed release kind {:?}", self.format, parsed.kind)
            } else {
                format!("Entry format {:?} is not TV-limited", self.format)
            }
        });
        if !tv_content_ok {
            tracing::trace!(release_title, "rejected: movie/special on a TV entry");
            return MatchEvaluation { matched: false, reason: "Movie or special is excluded from a TV entry".into(), checks };
        }
        // How specifically the release names this entry: a full name, else
        // the short name - which needs this entry's season: the marker, or
        // season 1 for a release that has none.
        // A whole-series pack, or (when this entry's season is unknown) a
        // multi-season pack, can't contradict the entry's season.
        let whole = (release_seasons.is_empty() && names_whole_series(&haystack)) || (self.seasons.is_empty() && release_seasons.len() >= 2);
        let in_season = release_seasons.iter().any(|season| self.seasons.contains(season)) || whole;
        let mut own = Self::longest_match(&haystack, &self.names);
        let mut name_basis = if own > 0 { "Selected entry title" } else { "No selected title yet" };
        if own == 0 {
            own = self.same_season_siblings.iter().map(|names| Self::longest_match(&haystack, names)).max().unwrap_or(0);
            if own > 0 {
                name_basis = "Same-season franchise alias";
                tracing::trace!(release_title, "same-season sibling title identifies the selected season");
            }
        }
        if own == 0 {
            let effective_ok = in_season || (release_seasons.is_empty() && self.seasons.contains(&1));
            own = if effective_ok { Self::longest_match(&haystack, &self.bases) } else { 0 };
            if own == 0 {
                add_match_check(&mut checks, collect, "Title identifies this entry", false, || {
                    format!("No whole-word entry/alias match; short-name season condition {effective_ok}; entry seasons {:?}, release seasons {:?}", self.seasons, release_seasons)
                });
                return MatchEvaluation { matched: false, reason: "Release title does not identify this entry".into(), checks };
            }
            name_basis = "Short title plus compatible season";
            tracing::trace!(release_title, "short name + season");
        }
        add_match_check(&mut checks, collect, "Title identifies this entry", true, || format!("{name_basis}; whole-series pack: {whole}"));
        // A sibling's longer, more specific name wins ("Food Wars! The Second
        // Plate" is not the first season's). A season marker that is this
        // entry's keeps the release here: the sibling is then a split cour or
        // its own season-qualified name ("... Season 3"), not another season.
        let sibling = if in_season { 0 } else { self.siblings.iter().map(|names| Self::longest_match(&haystack, names)).max().unwrap_or(0) };
        add_match_check(&mut checks, collect, "Selected title is at least as specific as siblings", sibling <= own, || format!("Selected-name score {own}; sibling-name score {sibling}"));
        if sibling > own {
            tracing::trace!(release_title, own, sibling, "rejected: names a sibling more specifically");
            return MatchEvaluation { matched: false, reason: "A franchise sibling is named more specifically".into(), checks };
        }
        MatchEvaluation { matched: true, reason: format!("Matched by {name_basis}"), checks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ignores_punctuation_and_case() {
        assert_eq!(normalize("Fate/strange Fake: Whispers-of Dawn!"), "fate strange fake whispers of dawn");
    }

    #[test]
    fn stem_normalisation_matches_normalize() {
        // nyaa-client keeps a copy of `normalize` for the stem mapper; they must agree.
        for text in ["Fate/strange Fake", "God's Blessing!", "Re:Zero \u{2019}s \u{2606} Ⅱ", "  Mixed   CASE-title "] {
            assert_eq!(nyaa_client::stem::normalize_stem(text), normalize(text));
        }
    }

    #[test]
    fn normalize_removes_apostrophes() {
        assert_eq!(normalize("Frieren: Beyond Journey's End"), "frieren beyond journeys end");
        assert_eq!(normalize("Frieren Beyond Journeys End"), "frieren beyond journeys end");
        assert_eq!(normalize("Kono Subarashii: God\u{2019}s Blessing"), "kono subarashii gods blessing");
        assert_eq!(normalize("Gods` Game"), "gods game");
    }

    #[test]
    fn apostrophe_titles_match_either_spelling() {
        let matcher = ShowMatcher::new(Some("Frieren: Beyond Journey's End"), None, &[], None);
        assert!(matcher.matches("[Group] Frieren Beyond Journeys End - 01 [1080p]"));
        assert!(matcher.matches("[Group] Frieren Beyond Journey's End - 01 [1080p]"));
    }

    #[test]
    fn release_seasons_follow_release_parse() {
        assert_eq!(release_seasons("[Breeze] Mushoku Tensei S03E12 [1080p]"), vec![3]);
        assert_eq!(release_seasons("[MiniMTBB] Mushoku Tensei S3 - 14 | Jobless Reincarnation"), vec![3]);
        assert_eq!(release_seasons("[Anime Time] Mushoku Tensei S01-S04 [BD]"), vec![1, 2, 3, 4]);
        assert_eq!(release_seasons("[VARYG] Tensei shitara Slime S04E03 (That Time I Got Reincarnated as a Slime 3rd Season)"), vec![4]);
        assert!(release_seasons("[SubsPlease] Frieren - 01 (1080p)").is_empty());
    }

    #[test]
    fn entry_seasons_come_from_titles() {
        let s = |v: &[u32]| v.iter().copied().collect::<BTreeSet<u32>>();
        assert_eq!(entry_seasons(Some("That Time I Got Reincarnated as a Slime Season 4"), Some("Tensei shitara Slime Datta Ken 4th Season"), &[], None), s(&[4]));
        assert_eq!(entry_seasons(Some("Mushoku Tensei: Jobless Reincarnation Season 3"), Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"), &[], None), s(&[3]));
        assert_eq!(entry_seasons(Some("Frieren: Beyond Journey's End"), Some("Sousou no Frieren"), &[], None), s(&[]));
    }

    #[test]
    fn tvdb_season_joins_the_title_seasons() {
        let s = |v: &[u32]| v.iter().copied().collect::<BTreeSet<u32>>();
        // Demon Slayer's Swordsmith Village: "Season 3" on Crunchyroll, S4 on TVDB.
        assert_eq!(entry_seasons(Some("Demon Slayer: Kimetsu no Yaiba Swordsmith Village Arc"), None, &[], Some(3)), s(&[3]));
        assert_eq!(entry_seasons(Some("Kimetsu no Yaiba Season 3"), None, &[], Some(4)), s(&[3, 4]));
        let matcher = ShowMatcher::new(Some("Frieren: Beyond Journey's End"), Some("Sousou no Frieren"), &[], Some(1));
        assert!(matcher.matches("[Group] Sousou no Frieren S01E05 [1080p]"));
        assert!(!matcher.matches("[Group] Sousou no Frieren S02E01 [1080p]"));
    }

    fn relative(english: &str, romaji: &str, relation: &str, format: &str) -> anilist_client::Relative {
        anilist_client::Relative {
            id: 1,
            title: anilist_client::AnimeTitle { english: Some(english.into()), romaji: Some(romaji.into()), native: None },
            synonyms: Vec::new(),
            format: Some(format.into()),
            relation: relation.into(),
        }
    }

    fn food_wars(has_tv_prequel: bool, english: &str, romaji: &str) -> ShowMatcher {
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel,
            relatives: vec![
                relative("Food Wars! The Second Plate", "Shokugeki no Souma: Ni no Sara", "SEQUEL", "TV"),
                relative("Food Wars! Shokugeki no Soma OVA", "Shokugeki no Souma OVA", "SIDE_STORY", "OVA"),
            ],
        };
        ShowMatcher::new(Some(english), Some(romaji), &[], None).with_franchise(&franchise)
    }

    #[test]
    fn sibling_with_a_longer_name_takes_its_release() {
        let first = food_wars(false, "Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert!(first.matches("[Group] Food Wars! Shokugeki no Soma - 05 [1080p]"));
        assert!(!first.matches("[Reza] Food Wars! The Second Plate [BDRip 1080p]"));
        // Same words as the sibling's -> the entry itself, not the sibling.
        let second = ShowMatcher::new(Some("Food Wars! The Second Plate"), Some("Shokugeki no Souma: Ni no Sara"), &[], None);
        assert!(second.matches("[Reza] Food Wars! The Second Plate [BDRip 1080p]"));
    }

    #[test]
    fn franchise_shares_a_bare_name_with_later_entries() {
        let third = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: true,
            relatives: vec![relative("Food Wars! Shokugeki no Soma", "Shokugeki no Souma", "PREQUEL", "TV")],
        };
        let english = Some("Food Wars! The Third Plate");
        let romaji = Some("Shokugeki no Souma: San no Sara");
        assert_eq!(franchise_base_names(english, romaji, &[], &third), vec!["food wars", "shokugeki no souma"]);
        assert_eq!(build_candidates(english, romaji, &[], Some(3), Some(&third)), vec!["food wars", "shokugeki no souma"]);
        let matcher = ShowMatcher::new(english, romaji, &[], Some(3)).with_franchise(&third);
        assert!(matcher.matches("[crane0922] Food Wars! Season 3 (BD 1080p HEVC Opus) [Dual Audio]"));
        assert!(!matcher.matches("[crane0922] Food Wars! Season 4 (BD 1080p HEVC Opus) [Dual Audio]"));
    }

    /// Mushoku Tensei Season 3 (AniList 178789) with its prequels, as in the franchise cache.
    fn mushoku_s3() -> ShowMatcher {
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: true,
            relatives: vec![
                relative("Mushoku Tensei: Jobless Reincarnation Season 2 Part 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu Part 2", "PREQUEL", "TV"),
                relative("Mushoku Tensei: Jobless Reincarnation Season 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "PREQUEL", "TV"),
                relative("Mushoku Tensei: Jobless Reincarnation Cour 2", "Mushoku Tensei: Isekai Ittara Honki Dasu Part 2", "PREQUEL", "TV"),
                relative("Mushoku Tensei: Jobless Reincarnation", "Mushoku Tensei: Isekai Ittara Honki Dasu", "PREQUEL", "TV"),
                relative("", "Mushoku Tensei III: Isekai Ittara Honki Dasu Part 2", "SEQUEL", "TV"),
            ],
        };
        let synonyms = ["Mushoku Tensei: Isekai Ittara Honki Dasu 3rd Season".to_string()];
        ShowMatcher::new(Some("Mushoku Tensei: Jobless Reincarnation Season 3"), Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"), &synonyms, Some(3)).with_franchise(&franchise)
    }

    #[test]
    fn unmarked_release_naming_only_what_earlier_seasons_share_is_ambiguous() {
        let m = mushoku_s3();
        // Season 1's BD and TV releases are ambiguous for the third season.
        for title in [
            "[Diogo4D-NF93] [BD][1080p] Mushoku Tensei Isekai Ittara Honki Dasu - 01 [127C0B18].mkv",
            "[SubsPlease] Mushoku Tensei: Jobless Reincarnation - 03 (1080p)",
            "[EA]Mushoku_Tensei_-_Isekai_Ittara_Honki_Dasu_01-11_End_[1920x1080][BDRIP][HEVC][Flac]",
        ] {
            assert!(!m.matches(title), "{title}");
            assert!(m.season_ambiguous(title), "{title}");
        }
        // Naming the entry itself, or carrying a season marker: not ambiguous.
        for title in [
            "[Diogo4D-NF93] [WEB][1080p] Mushoku Tensei III Isekai Ittara Honki Dasu - 01 [ABBEC7B2].mkv",
            "[shincaps] Mushoku Tensei III ~Isekai Ittara Honki Dasu~ - 01-02 (AT-X 1440x1080 MPEG2 AAC).ts",
            "[Breeze] Mushoku Tensei S03E12 [1080p AV1] | Jobless Reincarnation (weekly)",
            "[Judas] Mushoku Tensei (Jobless Reincarnation) - S03E01 [1080p]",
            "[DKB] Mushoku Tensei: Isekai Ittara Honki Dasu - S03E01 [1080p]",
        ] {
            assert!(m.matches(title), "{title}");
            assert!(!m.season_ambiguous(title), "{title}");
        }
        // The franchise base alone is not kept at all (this entry is not season 1).
        assert!(!m.matches("[Erai-raws] Mushoku Tensei - 01 [1080p]"));
    }

    #[test]
    fn later_season_without_franchise_data_needs_a_marker_or_distinctive_full_title() {
        let mushoku = ShowMatcher::new(Some("Mushoku Tensei: Jobless Reincarnation Season 3"), Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"), &[], Some(3));
        assert!(!mushoku.matches("[Group] Mushoku Tensei - 05 [1080p]"));
        assert!(!mushoku.matches("[Group] Mushoku Tensei: Jobless Reincarnation - 05 [1080p]"));
        assert!(mushoku.matches("[Group] Mushoku Tensei III - 05 [1080p]"));

        let food_wars = ShowMatcher::new(Some("Food Wars! The Second Plate"), Some("Shokugeki no Souma: Ni no Sara"), &[], Some(2));
        assert!(food_wars.matches("[Group] Food Wars! The Second Plate - 05 [1080p]"));
    }

    #[test]
    fn roman_numeral_after_the_name_gives_the_label_season() {
        let m = mushoku_s3();
        assert_eq!(m.roman_season_of("[Diogo4D-NF93] [WEB][1080p] Mushoku Tensei III Isekai Ittara Honki Dasu - 01 [ABBEC7B2].mkv"), Some(3));
        assert_eq!(m.roman_season_of("[shincaps] Mushoku Tensei III ~Isekai Ittara Honki Dasu~ - 14 (BS11).ts"), Some(3));
        assert_eq!(m.roman_season_of("[Judas] Mushoku Tensei (Jobless Reincarnation) - S03E01"), None);
        assert_eq!(m.roman_season_of("[BD] Mushoku Tensei Isekai Ittara Honki Dasu - 01"), None);
    }

    #[test]
    fn first_season_entries_reject_unmarked_shared_titles() {
        let franchise = Franchise { format: Some("TV".into()), has_tv_prequel: false, relatives: vec![relative("Mushoku Tensei: Jobless Reincarnation Season 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "SEQUEL", "TV")] };
        let first = ShowMatcher::new(Some("Mushoku Tensei: Jobless Reincarnation"), Some("Mushoku Tensei: Isekai Ittara Honki Dasu"), &[], None).with_franchise(&franchise);
        let shared_unmarked = "[SubsPlease] Mushoku Tensei: Jobless Reincarnation - 03 (1080p)";
        assert!(first.season_ambiguous(shared_unmarked));
        assert!(!first.matches(shared_unmarked));
        assert!(first.matches("[Group] Mushoku Tensei: Jobless Reincarnation S01E03"));
    }

    #[test]
    fn same_season_siblings_do_not_take_each_others_releases() {
        // Split cour: the sibling's "... Season 3" name is longer, but the release is season 3.
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: true,
            relatives: vec![relative("Mushoku Tensei: Jobless Reincarnation Season 3", "Mushoku Tensei III: Isekai Ittara Honki Dasu", "PREQUEL", "TV")],
        };
        let part2 = ShowMatcher::new(None, Some("Mushoku Tensei III: Isekai Ittara Honki Dasu Part 2"), &["Mushoku Tensei: Jobless Reincarnation".to_string()], None).with_franchise(&franchise);
        assert!(part2.matches("[Doomdos] - Mushoku Tensei: Jobless Reincarnation Season 3 - 第11话 - [1080p]"));
    }

    #[test]
    fn roman_numeral_after_the_short_name_is_a_season() {
        let first = ShowMatcher::new(Some("Mushoku Tensei: Jobless Reincarnation"), Some("Mushoku Tensei: Isekai Ittara Honki Dasu"), &[], Some(1));
        assert!(!first.matches("[New-raws] Mushoku Tensei II Isekai Ittara Honki Dasu - 00~12"));
        assert!(first.matches("[Group] Mushoku Tensei Isekai Ittara Honki Dasu - 05"));
    }

    #[test]
    fn extras_words_keep_movies_off_tv_but_not_season_packs() {
        let tv = food_wars(false, "Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert!(!tv.matches("[Koten_Gars] Shokugeki no Souma - OADs [DVD]"));
        assert!(tv.matches("[Anitsu] Shokugeki no Souma S01 + OVA [BD 1080p]"));
        assert!(tv.matches("[Sokudo] Shokugeki no Souma S1-S5+OVA v2 [1080p BD]"));
    }

    #[test]
    fn complete_series_packs_belong_to_every_season() {
        let third = food_wars(true, "Food Wars! The Third Plate", "Shokugeki no Souma: San no Sara");
        assert!(third.matches("[DB] Shokugeki no Souma | Food Wars! (Complete Series+OVAs) [Dual Audio 10bit BD1080p][HEVC-x265]"));
        assert!(third.matches("[Anime Time] Food Wars! (Shokugeki no Soma) S1 - S5 [Full Series][BD][Dual Audio]"));
    }

    #[test]
    fn primary_season_falls_back_to_the_prequel_chain_without_split_cours() {
        let tv = |english: &str, romaji: &str| relative(english, romaji, "PREQUEL", "TV");
        let primary = |english: Option<&str>, romaji: Option<&str>, relatives: Vec<anilist_client::Relative>, tvdb: Option<u32>| {
            let franchise = Franchise { format: Some("TV".into()), has_tv_prequel: !relatives.is_empty(), relatives };
            ShowMatcher::new(english, romaji, &[], tvdb).with_franchise(&franchise).primary_season()
        };
        // Food Wars! The Second Plate: one TV season before it.
        let s1 = tv("Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert_eq!(primary(Some("Food Wars! The Second Plate"), Some("Shokugeki no Souma: Ni no Sara"), vec![s1.clone()], None), Some(2));
        // Slime S2 Part 2 shares season 2 with its own prequel.
        let slime = vec![tv("That Time I Got Reincarnated as a Slime Season 2", "Tensei shitara Slime Datta Ken 2nd Season"), tv("That Time I Got Reincarnated as a Slime", "Tensei shitara Slime Datta Ken")];
        assert_eq!(primary(None, Some("Tensei shitara Slime Datta Ken 2nd Season Part 2"), slime, None), Some(2));
        // Mushoku S3 Part 2 behind S1, Cour 2, S2, S2 Part 2, S3.
        let mushoku = vec![
            tv("Mushoku Tensei: Jobless Reincarnation Season 3", "Mushoku Tensei III: Isekai Ittara Honki Dasu"),
            tv("Mushoku Tensei: Jobless Reincarnation Season 2 Part 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu Part 2"),
            tv("Mushoku Tensei: Jobless Reincarnation Season 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu"),
            tv("Mushoku Tensei: Jobless Reincarnation Cour 2", "Mushoku Tensei: Isekai Ittara Honki Dasu Part 2"),
            tv("Mushoku Tensei: Jobless Reincarnation", "Mushoku Tensei: Isekai Ittara Honki Dasu"),
        ];
        assert_eq!(primary(None, Some("Mushoku Tensei III: Isekai Ittara Honki Dasu Part 2"), mushoku, None), Some(3));
        // TVDB wins over everything.
        assert_eq!(primary(Some("Food Wars! The Second Plate"), None, vec![s1], Some(2)), Some(2));
    }

    #[test]
    fn first_season_flag_adds_season_one() {
        let first = food_wars(false, "Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert!(!first.matches("[Group] Shokugeki no Souma S02E05 [1080p]"));
        assert!(first.matches("[Group] Shokugeki no Souma S01E05 [1080p]"));
        // A sequel's chain has a TV prequel: no season is assumed.
        let later = food_wars(true, "Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert!(later.matches("[Group] Shokugeki no Souma S02E05 [1080p]"));
    }

    #[test]
    fn movies_and_specials_stay_off_a_tv_entry() {
        let tv = food_wars(false, "Food Wars! Shokugeki no Soma", "Shokugeki no Souma");
        assert!(!tv.matches("[Group] Shokugeki no Souma OVA [BD 1080p]"));
        assert!(!tv.matches("[Group] Shokugeki no Souma Movie [BD 1080p]"));
        let ova = ShowMatcher::new(Some("Food Wars! Shokugeki no Soma OVA"), Some("Shokugeki no Souma OVA"), &[], None)
            .with_franchise(&Franchise { format: Some("OVA".into()), has_tv_prequel: false, relatives: Vec::new() });
        assert!(ova.matches("[Group] Shokugeki no Souma OVA [BD 1080p]"));
    }

    #[test]
    fn rejects_releases_of_other_seasons() {
        let matcher = ShowMatcher::new(Some("That Time I Got Reincarnated as a Slime Season 3"), Some("Tensei shitara Slime Datta Ken 3rd Season"), &[], None);
        assert!(matcher.matches("[Group] Tensei shitara Slime Datta Ken S03E05 [1080p]"));
        assert!(matcher.matches("[Group] That Time I Got Reincarnated as a Slime - 05 (3rd Season)"));
        assert!(!matcher.matches("[ToonsHub] That Time I Got Reincarnated as a Slime S04E21 [1080p]"));
        assert!(matcher.matches("[Group] That Time I Got Reincarnated as a Slime S01-S04 [BD]"));
    }

    #[test]
    fn short_name_with_season_marker_is_accepted() {
        let matcher = ShowMatcher::new(Some("Mushoku Tensei: Jobless Reincarnation Season 3"), Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"), &[], None);
        assert!(matcher.matches("[Breeze] Mushoku Tensei S03E12 [1080p]"));
        assert!(matcher.matches("[MiniMTBB] Mushoku Tensei S3 - 14 | Jobless Reincarnation"));
        assert!(matcher.matches("[DKB] Mushoku Tensei: Isekai Ittara Honki Dasu - S03E14"));
        assert!(!matcher.matches("[SubsPlease] Mushoku Tensei S2 - 05"));
        // No marker = season 1, which isn't this entry.
        assert!(!matcher.matches("[ASW] Mushoku Tensei - 05"));
        assert!(!matcher.matches("[SubsPlease] Fate Grand Order S03E01"));
    }

    #[test]
    fn unknown_entry_season_keeps_todays_rules() {
        let matcher = ShowMatcher::new(Some("Frieren: Beyond Journey's End"), Some("Sousou no Frieren"), &[], None);
        assert!(matcher.matches("[Group] Sousou no Frieren S02E01 [1080p]"));
        assert!(matcher.matches("[Group] Frieren Beyond Journeys End - 05"));
        assert!(!matcher.matches("[Group] Fate strange Fake - 05"));
    }

    fn tosho(aid: u32, approved: u8) -> ToshoRelease {
        ToshoRelease { aid, eid: 7, approved }
    }

    #[test]
    fn trusted_aid_decides_both_ways_over_the_title() {
        let matcher = ShowMatcher::new(Some("That Time I Got Reincarnated as a Slime Season 3"), Some("Tensei shitara Slime Datta Ken 3rd Season"), &[], None);
        let ours = [111, 112];
        // A season marker for another season does not matter: the id wins.
        let other_season = "[ToonsHub] That Time I Got Reincarnated as a Slime S04E21 [1080p]";
        assert!(!matcher.matches(other_season));
        for approved in [1, 2, 3] {
            assert_eq!(matcher.decide(other_season, Some(&tosho(112, approved)), &ours), Decision::Accept);
            assert!(matcher.matches_with_aid(other_season, Some(&tosho(112, approved)), &ours));
            // A matching title is still another entry's when the id says so.
            assert_eq!(matcher.decide("[Group] Tensei shitara Slime Datta Ken S03E05", Some(&tosho(999, approved)), &ours), Decision::Reject);
            assert!(!matcher.matches_with_aid("[Group] Tensei shitara Slime Datta Ken S03E05", Some(&tosho(999, approved)), &ours));
        }
    }

    #[test]
    fn guessed_aid_is_only_a_hint() {
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: true,
            relatives: vec![relative("That Time I Got Reincarnated as a Slime", "Tensei shitara Slime Datta Ken", "PREQUEL", "TV")],
        };
        let matcher = ShowMatcher::new(Some("That Time I Got Reincarnated as a Slime Season 3"), Some("Tensei shitara Slime Datta Ken 3rd Season"), &[], None).with_franchise(&franchise);
        let ours = [111];
        // Right entry, nothing against it in the title: accepted even though the title rules would say no.
        let nameless = "[Group] Something Else Entirely - 05";
        assert!(!matcher.matches(nameless));
        assert_eq!(matcher.decide(nameless, Some(&tosho(111, 0)), &ours), Decision::Accept);
        // Right entry but the title names only another season: title rules decide (reject).
        let other_season = "[ToonsHub] That Time I Got Reincarnated as a Slime S04E21 [1080p]";
        assert_eq!(matcher.decide(other_season, Some(&tosho(111, 0)), &ours), Decision::UseTitleRules);
        assert!(!matcher.matches_with_aid(other_season, Some(&tosho(111, 0)), &ours));
        // Another entry's guess never rejects by itself.
        let ok = "[Group] Tensei shitara Slime Datta Ken S03E05 [1080p]";
        assert_eq!(matcher.decide(ok, Some(&tosho(999, 0)), &ours), Decision::UseTitleRules);
        assert!(matcher.matches_with_aid(ok, Some(&tosho(999, 0)), &ours));
        let shared_unmarked = "[SubsPlease] That Time I Got Reincarnated as a Slime - 05 [1080p]";
        assert_eq!(matcher.decide(shared_unmarked, Some(&tosho(111, 0)), &ours), Decision::UseTitleRules);
        assert!(!matcher.matches_with_aid(shared_unmarked, Some(&tosho(111, 0)), &ours));
        assert_eq!(matcher.decide(shared_unmarked, Some(&tosho(111, 1)), &ours), Decision::Accept);
    }

    #[test]
    fn no_usable_aid_keeps_the_title_rules() {
        let matcher = ShowMatcher::new(Some("Frieren: Beyond Journey's End"), None, &[], None);
        let title = "[Group] Frieren Beyond Journeys End - 05";
        assert_eq!(matcher.decide(title, None, &[1]), Decision::UseTitleRules);
        assert_eq!(matcher.decide(title, Some(&tosho(0, 3)), &[1]), Decision::UseTitleRules);
        // Entry with no AniDB mapping: nothing to compare against.
        assert_eq!(matcher.decide(title, Some(&tosho(5, 3)), &[]), Decision::UseTitleRules);
        assert!(matcher.matches_with_aid(title, None, &[1]));
        assert!(!matcher.matches_with_aid("[Group] Fate strange Fake - 05", None, &[1]));
    }

    #[test]
    fn later_season_remote_queries_require_a_season_or_unique_entry_name() {
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: true,
            relatives: vec![
                relative("Mushoku Tensei: Jobless Reincarnation", "Mushoku Tensei: Isekai Ittara Honki Dasu", "PREQUEL", "TV"),
                relative("Mushoku Tensei: Jobless Reincarnation Season 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "PREQUEL", "TV"),
            ],
        };
        let queries = build_search_queries(
            Some("Mushoku Tensei: Jobless Reincarnation Season 3"),
            Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"),
            &[],
            Some(3),
            Some(&franchise),
        );
        let rendered: Vec<String> = queries.iter().map(NyaaSearchQuery::render).collect();
        assert!(rendered.iter().any(|q| q.contains("\"Mushoku Tensei S03\"")));
        assert!(rendered.iter().any(|q| q.contains("\"Mushoku Tensei III: Isekai Ittara Honki Dasu\"")));
        assert!(!queries.iter().any(|q| q.includes.iter().any(|phrase| normalize(phrase) == "mushoku tensei")));
        assert!(!rendered.iter().any(|q| q.contains("\"Mushoku Tensei: Jobless Reincarnation\"")));
    }

    #[test]
    fn first_season_base_query_excludes_sibling_season_phrases() {
        let franchise = Franchise {
            format: Some("TV".into()),
            has_tv_prequel: false,
            relatives: vec![relative("Mushoku Tensei: Jobless Reincarnation Season 2", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "SEQUEL", "TV")],
        };
        let queries = build_search_queries(
            Some("Mushoku Tensei: Jobless Reincarnation"),
            Some("Mushoku Tensei: Isekai Ittara Honki Dasu"),
            &[],
            None,
            Some(&franchise),
        );
        let rendered: Vec<String> = queries.iter().map(NyaaSearchQuery::render).collect();
        assert!(!queries.iter().any(|q| q.includes.iter().any(|phrase| normalize(phrase) == "mushoku tensei")));
        assert!(rendered.iter().any(|q| q.contains(" -\"Mushoku Tensei: Jobless Reincarnation Season 2\"")));
        assert!(rendered.iter().any(|q| q.contains("\"Mushoku Tensei S02\"")));
        assert!(rendered.iter().any(|q| q.contains("\"Mushoku Tensei: Jobless Reincarnation Season 2\"")));
    }

    #[test]
    fn long_first_season_exclusions_are_bounded_to_nyaa_query_limit() {
        let query = NyaaSearchQuery {
            includes: vec!["Show Name".into(), "Show Name Alternative".into()],
            excludes: (0..21).map(|i| format!("Show Name sibling alias {i} season 2")).collect(),
        };
        let chunks = split_search_query(query);
        assert!(chunks.iter().all(|query| query.render().chars().count() <= MAX_QUERY_CHARS));
        assert!(chunks.iter().all(|query| query.excludes.len() < 21));
        assert!(chunks.iter().all(|query| !query.includes.is_empty()));
    }

    #[test]
    fn movie_remote_queries_use_full_entry_names() {
        let franchise = Franchise { format: Some("MOVIE".into()), has_tv_prequel: true, relatives: Vec::new() };
        let queries = build_search_queries(Some("Your Name."), Some("Kimi no Na wa."), &[], None, Some(&franchise));
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].render(), "\"Your Name.\"|\"Kimi no Na wa.\"");
    }

    #[test]
    fn required_uploader_term_is_repeated_for_each_or_branch() {
        let query = NyaaSearchQuery { includes: vec!["Show S2".into(), "Show Season 2".into()], excludes: vec!["Show S1".into()] };
        assert_eq!(query.render_with_required_term("SubsPlease"), "SubsPlease \"Show S2\"|SubsPlease \"Show Season 2\" -\"Show S1\"");
    }

    #[test]
    fn base_names_drop_roman_numerals_and_subtitles() {
        assert_eq!(base_name("Mushoku Tensei III: Isekai Ittara Honki Dasu").as_deref(), Some("mushoku tensei"));
        assert_eq!(base_name("Mushoku Tensei: Jobless Reincarnation").as_deref(), Some("mushoku tensei"));
        assert_eq!(base_name("Fate"), None);
        assert_eq!(base_name("Tensura"), None);
    }

    #[test]
    fn season_stripping() {
        assert_eq!(strip_season_suffix("That Time I Got Reincarnated as a Slime Season 4").as_deref(), Some("That Time I Got Reincarnated as a Slime"));
        assert_eq!(strip_season_suffix("Kono Subarashii Sekai ni Shukufuku wo! 2nd Season").as_deref(), Some("Kono Subarashii Sekai ni Shukufuku wo!"));
    }

    #[test]
    fn superset_queries_are_dropped() {
        let candidates = build_candidates(
            Some("That Time I Got Reincarnated as a Slime Season 4"),
            Some("Tensei shitara Slime Datta Ken 4th Season"),
            &["Tensura".to_string(), "\u{8EE2}\u{751F}".to_string()],
            None,
            None,
        );
        assert_eq!(candidates, vec!["Tensura", "Tensei shitara Slime Datta Ken", "That Time I Got Reincarnated as a Slime"]);
    }

    #[test]
    fn synonyms_and_roman_numerals_are_stripped_in_queries() {
        let candidates = build_candidates(Some("Mob Psycho 100 III"), Some("Mob Psycho 100 III"), &["Mob Psycho 100 3rd Season".to_string()], None, None);
        assert_eq!(candidates, vec!["Mob Psycho 100"]);
        assert_eq!(strip_roman_suffix("Rocky V"), None);
        assert_eq!(strip_roman_suffix("Mob Psycho 100 III").as_deref(), Some("Mob Psycho 100"));
    }

    #[test]
    fn later_seasons_query_the_short_base_name() {
        let candidates = build_candidates(Some("Mushoku Tensei: Jobless Reincarnation Season 3"), Some("Mushoku Tensei III: Isekai Ittara Honki Dasu"), &[], None, None);
        assert_eq!(candidates, vec!["Mushoku Tensei"]);
        // Season 1 keeps its full names.
        let first = build_candidates(Some("Mushoku Tensei: Jobless Reincarnation"), Some("Mushoku Tensei: Isekai Ittara Honki Dasu"), &[], Some(1), None);
        assert_eq!(first, vec!["Mushoku Tensei: Jobless Reincarnation", "Mushoku Tensei: Isekai Ittara Honki Dasu"]);
        // A TVDB season >= 2 alone is enough.
        let tvdb = build_candidates(Some("Kimetsu no Yaiba: Yuukaku-hen"), None, &[], Some(2), None);
        assert_eq!(tvdb, vec!["Kimetsu no Yaiba"]);
    }

    #[test]
    fn fate_strange_fake_collapses_to_one_query() {
        let candidates = build_candidates(Some("Fate/strange Fake"), Some("Fate/strange Fake"), &[], None, None);
        assert_eq!(candidates, vec!["Fate/strange Fake"]);
        // The subtitle tells the special apart from the TV series: kept.
        let candidates = build_candidates(Some("Fate/strange Fake: Whispers of Dawn"), Some("Fate/strange Fake: Whispers of Dawn"), &[], None, None);
        assert_eq!(candidates, vec!["Fate/strange Fake: Whispers of Dawn"]);
    }

    #[test]
    fn matches_show_needs_whole_word_name() {
        let matcher = ShowMatcher::new(Some("Fate/strange Fake"), None, &[], None);
        assert!(matcher.matches("[Kaleido-subs] Fate strange Fake - 13 (S01E13)"));
        assert!(matcher.matches("[Erai-raws] Fate/Strange Fake - 13 [1080p]"));
        assert!(!matcher.matches("[SubsPlease] Fate Grand Order - 13 (1080p)"));
        assert!(!matcher.matches("[SubsPlease] Fate strange Fakery - 13"));
    }
}
