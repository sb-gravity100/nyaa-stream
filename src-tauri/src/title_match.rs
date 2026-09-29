//! Matching an AniList show to nyaa.si releases: which queries to send
//! (`build_candidates`) and which returned releases actually belong to the
//! show (`matches_show`).
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

/// Lowercased words of `text` with all punctuation (`/ : - ! ☆ '`...) as
/// separators - the way nyaa.si compares titles.
pub fn normalize(text: &str) -> String {
    text.chars()
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

/// Whether a release title contains one of the show's names as whole words.
/// `names` are the output of `match_names`.
pub fn matches_show(release_title: &str, names: &[String]) -> bool {
    let haystack = format!(" {} ", normalize(release_title));
    names.iter().any(|name| haystack.contains(&format!(" {name} ")))
}

/// The normalized names `matches_show` compares against.
pub fn match_names(english: Option<&str>, romaji: Option<&str>, synonyms: &[String]) -> Vec<String> {
    names(english, romaji, synonyms).iter().map(|name| normalize(name)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ignores_punctuation_and_case() {
        assert_eq!(normalize("Fate/strange Fake: Whispers-of Dawn!"), "fate strange fake whispers of dawn");
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
        let names = match_names(Some("Fate/strange Fake"), None, &[]);
        assert!(matches_show("[Kaleido-subs] Fate strange Fake - 13 (S01E13)", &names));
        assert!(matches_show("[Erai-raws] Fate/Strange Fake - 13 [1080p]", &names));
        assert!(!matches_show("[SubsPlease] Fate Grand Order - 13 (1080p)", &names));
        assert!(!matches_show("[SubsPlease] Fate strange Fakery - 13", &names));
    }
}
