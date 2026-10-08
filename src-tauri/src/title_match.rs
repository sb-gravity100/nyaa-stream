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
use release_parse::Kind;

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
        !key.is_empty() && seen.insert(key)
    });
    all
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
    let mut bases: Vec<String> = Vec::new();
    for relative in &franchise.relatives {
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
    bases: Vec<String>,
    seasons: BTreeSet<u32>,
    /// The entry's AniList format (TV, MOVIE, OVA, ...), once known.
    format: Option<String>,
    /// Normalized names of the franchise's other entries.
    siblings: Vec<Vec<String>>,
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
        let matcher = ShowMatcher { names: titles.iter().map(|name| normalize(name)).collect(), bases, seasons: entry_seasons(english, romaji, synonyms, tvdb_season), format: None, siblings: Vec::new() };
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
        }
        for base in shared_bases(&self.names, franchise) {
            if !self.bases.contains(&base) {
                self.bases.push(base);
            }
        }
        self.siblings = franchise
            .relatives
            .iter()
            .map(|relative| names(relative.title.english.as_deref(), relative.title.romaji.as_deref(), &relative.synonyms).iter().map(|name| normalize(name)).collect())
            .collect();
        tracing::debug!(format = ?self.format, seasons = ?self.seasons, siblings = self.siblings.len(), "show matcher given its franchise");
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

    /// Whether `release_title` belongs to this entry.
    pub fn matches(&self, release_title: &str) -> bool {
        let haystack = haystack(release_title);
        let mut release_seasons = release_seasons(release_title);
        // "Mushoku Tensei II Isekai ..." - a roman numeral right after a
        // known short name is its season (release-parse can't know names).
        if release_seasons.is_empty() {
            release_seasons.extend(self.roman_season(&haystack));
        }
        // Markers that name only other seasons rule the release out, even
        // when a full name matches ("Slime" names every season). With no
        // season known for the entry nothing is ruled out.
        if !self.seasons.is_empty() && !release_seasons.is_empty() && !release_seasons.iter().any(|season| self.seasons.contains(season)) {
            tracing::trace!(release_title, ?release_seasons, entry_seasons = ?self.seasons, "rejected: other season");
            return false;
        }
        // Movies, OVAs and specials are filed under their own entry, not
        // a TV season's episode list.
        // A pack naming several seasons may bundle them ("S01-S05 + OVA").
        let parsed = release_parse::parse_release(release_title);
        // (A season pack that bundles extras - "S01 + OVA" - is still its season's.)
        if is_tv(self.format.as_deref()) && release_seasons.len() < 2 && (matches!(parsed.kind, Kind::Movie | Kind::Special) || (names_extra(&haystack) && !parsed.extras)) {
            tracing::trace!(release_title, "rejected: movie/special on a TV entry");
            return false;
        }
        // How specifically the release names this entry: a full name, else
        // the short name - which needs this entry's season: the marker, or
        // season 1 for a release that has none.
        let in_season = release_seasons.iter().any(|season| self.seasons.contains(season));
        let mut own = Self::longest_match(&haystack, &self.names);
        if own == 0 {
            let effective_ok = in_season || (release_seasons.is_empty() && self.seasons.contains(&1));
            own = if effective_ok { Self::longest_match(&haystack, &self.bases) } else { 0 };
            if own == 0 {
                return false;
            }
            tracing::trace!(release_title, "short name + season");
        }
        // A sibling's longer, more specific name wins ("Food Wars! The Second
        // Plate" is not the first season's). A season marker that is this
        // entry's keeps the release here: the sibling is then a split cour or
        // its own season-qualified name ("... Season 3"), not another season.
        let sibling = if in_season { 0 } else { self.siblings.iter().map(|names| Self::longest_match(&haystack, names)).max().unwrap_or(0) };
        if sibling > own {
            tracing::trace!(release_title, own, sibling, "rejected: names a sibling more specifically");
            return false;
        }
        true
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
