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

/// Most queries sent per show - each is at least one nyaa.si request.
const MAX_CANDIDATES: usize = 4;
/// Shortest normalized synonym worth its own query: short aliases match far
/// too many unrelated releases.
const MIN_SYNONYM_LEN: usize = 5;
/// Shortest normalized base name (the short form before a season marker).
const MIN_BASE_NAME_LEN: usize = 5;

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

/// Every name a release of this show might carry: full titles, with the
/// season qualifier stripped, and Latin-script synonyms. Subtitles are kept:
/// they tell sibling entries apart ("Fate/strange Fake" vs its "Whispers of
/// Dawn" special), which a stripped query would merge.
fn names(english: Option<&str>, romaji: Option<&str>, synonyms: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for base in [english, romaji].into_iter().flatten() {
        all.push(base.to_string());
        let unseasoned = strip_season_suffix(base);
        all.extend(unseasoned);
    }
    all.extend(synonyms.iter().filter(|s| is_latin(s) && normalize(s).len() >= MIN_SYNONYM_LEN).cloned());
    let mut seen = BTreeSet::new();
    all.retain(|name| {
        let key = normalize(name);
        !key.is_empty() && seen.insert(key)
    });
    all
}

/// The nyaa.si queries to run: the show's names minus any whose words are a
/// superset of another's (see the module comment), fewest words first,
/// capped at `MAX_CANDIDATES`.
pub fn build_candidates(english: Option<&str>, romaji: Option<&str>, synonyms: &[String]) -> Vec<String> {
    let all = names(english, romaji, synonyms);
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

fn is_roman_numeral(word: &str) -> bool {
    matches!(word, "ii" | "iii" | "iv" | "v" | "vi" | "vii" | "viii" | "ix" | "x")
}

/// The short name groups write before a season marker: the part of a title
/// before the first `:`, a trailing roman numeral removed ("Mushoku Tensei
/// III: Isekai Ittara Honki Dasu" -> "mushoku tensei"). None when it is under
/// 2 words or 5 characters, because shorter names match unrelated shows.
fn base_name(title: &str) -> Option<String> {
    let head = title.split(':').next().unwrap_or(title);
    let normalized = normalize(head);
    let mut words: Vec<&str> = normalized.split(' ').collect();
    if words.len() > 1 && words.last().is_some_and(|w| is_roman_numeral(w)) {
        words.pop();
    }
    let base = words.join(" ");
    (words.len() >= 2 && base.len() >= MIN_BASE_NAME_LEN).then_some(base)
}

/// Decides which returned releases belong to one AniList entry: its names
/// plus its season numbers, so a release naming only other
/// seasons is dropped and a short name with this entry's season marker is
/// accepted (see PLAN.md "Season-aware source matching").
pub struct ShowMatcher {
    names: Vec<String>,
    bases: Vec<String>,
    seasons: BTreeSet<u32>,
}

impl ShowMatcher {
    pub fn new(english: Option<&str>, romaji: Option<&str>, synonyms: &[String], tvdb_season: Option<u32>) -> Self {
        let titles = names(english, romaji, synonyms);
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
        let matcher = ShowMatcher { names: titles.iter().map(|name| normalize(name)).collect(), bases, seasons: entry_seasons(english, romaji, synonyms, tvdb_season) };
        tracing::debug!(names = ?matcher.names, bases = ?matcher.bases, seasons = ?matcher.seasons, "show matcher built");
        matcher
    }

    /// Whether `release_title` belongs to this entry.
    pub fn matches(&self, release_title: &str) -> bool {
        let haystack = haystack(release_title);
        let release_seasons = release_seasons(release_title);
        // Markers that name only other seasons rule the release out, even
        // when a full name matches ("Slime" names every season). With no
        // season known for the entry nothing is ruled out.
        if !self.seasons.is_empty() && !release_seasons.is_empty() && !release_seasons.iter().any(|season| self.seasons.contains(season)) {
            tracing::trace!(release_title, ?release_seasons, entry_seasons = ?self.seasons, "rejected: other season");
            return false;
        }
        if self.names.iter().any(|name| haystack.contains(&format!(" {name} "))) {
            return true;
        }
        // The short name needs this entry's season: the marker, or season 1
        // for a release that has none.
        let effective = if release_seasons.is_empty() { vec![1] } else { release_seasons };
        let season_ok = effective.iter().any(|season| self.seasons.contains(season));
        let accepted = season_ok && self.bases.iter().any(|base| haystack.contains(&format!(" {base} ")));
        if accepted {
            tracing::trace!(release_title, "accepted: base name + season");
        }
        accepted
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
        );
        assert_eq!(candidates, vec!["Tensura", "Tensei shitara Slime Datta Ken", "That Time I Got Reincarnated as a Slime"]);
    }

    #[test]
    fn fate_strange_fake_collapses_to_one_query() {
        let candidates = build_candidates(Some("Fate/strange Fake"), Some("Fate/strange Fake"), &[]);
        assert_eq!(candidates, vec!["Fate/strange Fake"]);
        // The subtitle tells the special apart from the TV series: kept.
        let candidates = build_candidates(Some("Fate/strange Fake: Whispers of Dawn"), Some("Fate/strange Fake: Whispers of Dawn"), &[]);
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
