//! Rule-based release-title stem parsing and normalization.

/// Shortest stem worth keeping: shorter names are too generic to own an id.
const MIN_STEM_LEN: usize = 4;

/// Apostrophes and their lookalikes (same set as `title_match::is_apostrophe`).
fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '\u{02BC}' | '`' | '\u{00B4}')
}

/// Lowercased words with punctuation as separators and apostrophes removed.
/// A deliberate copy of `title_match::normalize` (that module lives in the
/// app crate, which depends on this one); `src-tauri` tests that both agree.
pub fn normalize_stem(text: &str) -> String {
    text.chars()
        .filter(|c| !is_apostrophe(*c))
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The stem of a release title and its fansub group (empty when none), or
/// `None` when the title has no usable show name.
pub fn parse_stem(title: &str) -> Option<(String, String)> {
    let release = release_parse::parse_release(title);
    let stem = normalize_stem(release.title.as_deref()?);
    if stem.chars().count() < MIN_STEM_LEN || !stem.chars().any(char::is_alphabetic) {
        return None;
    }
    Some((stem, release.group.map(|g| normalize_stem(&g)).unwrap_or_default()))
}

/// The stem of a release title (see `parse_stem`).
pub fn stem_of(title: &str) -> Option<String> {
    parse_stem(title).map(|(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_drop_group_season_and_episode() {
        let (stem, group) = parse_stem("[SubsPlease] Sousou no Frieren - 12 (1080p) [ABCD1234].mkv").unwrap();
        assert_eq!((stem.as_str(), group.as_str()), ("sousou no frieren", "subsplease"));
        assert_eq!(stem_of("[Erai-raws] Kimetsu no Yaiba 2nd Season - 03 [1080p]").unwrap(), "kimetsu no yaiba");
        assert_eq!(stem_of("[G] Fate/Zero - 05").unwrap(), "fate zero");
        assert_eq!(stem_of("[G] God's Blessing - 01").unwrap(), "gods blessing");
        assert_eq!(stem_of("[G] A - 01"), None); // too short
        assert_eq!(normalize_stem("Fate/strange  Fake!"), "fate strange fake");
    }
}
