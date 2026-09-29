//! Fansubber knowledge for narrowing nyaa.si searches to one uploader.
//!
//! nyaa.si can search inside a single user's uploads (`/user/<name>?q=`),
//! which is a handful of results instead of hundreds of pages across every
//! group - far fewer requests against a server that rate limits. But release
//! titles carry a group *tag* (`[SubsPlease]`), not the uploader's account
//! name, and the two only sometimes match. So:
//! - `POPULAR` seeds the list Settings suggests; observed group tags in real
//!   search results are counted on disk and folded in (`GroupCounts`).
//! - a tag's account name is resolved once (`NyaaClient::resolve_fansubber`:
//!   probe `/user/<tag>`, else read the submitter off one of the group's
//!   release pages) and cached, so later searches go straight to the user.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::NyaaResult;

/// Groups most anime releases come from - a starting point before any
/// searches have been counted.
pub(crate) const POPULAR: &[&str] = &[
    "SubsPlease", "Erai-raws", "ToonsHub", "Judas", "EMBER", "ASW", "Tsundere-Raws", "Yameii", "Kaleido-subs", "Ironclad", "DKB", "Commie",
];

/// How long a resolved account name is trusted, and how long "no such
/// account" is remembered before probing again.
pub(crate) const RESOLVED_FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(90 * 24 * 3600);
pub(crate) const UNRESOLVED_FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

/// Group tag counts observed in search results.
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct GroupCounts(pub BTreeMap<String, u32>);

/// The group tag a release title starts with: `[SubsPlease] Show - 01` ->
/// `SubsPlease`. Bracketed non-group prefixes (`[1080p]`) are rejected by
/// requiring a letter start and no spaces.
pub fn group_tag(title: &str) -> Option<&str> {
    let rest = title.trim_start().strip_prefix('[')?;
    let tag = rest[..rest.find(']')?].trim();
    let plausible = tag.chars().next().is_some_and(|c| c.is_alphabetic())
        && !tag.contains(' ')
        && !tag.eq_ignore_ascii_case("batch")
        && tag.len() <= 32;
    plausible.then_some(tag)
}

impl GroupCounts {
    pub(crate) fn add(&mut self, results: &[NyaaResult]) {
        for result in results {
            if let Some(tag) = group_tag(&result.title) {
                *self.0.entry(tag.to_string()).or_default() += 1;
            }
        }
    }

    /// Seeds first (in order), then the most-seen tags, deduplicated
    /// case-insensitively.
    pub(crate) fn popular(&self, limit: usize) -> Vec<String> {
        let mut by_count: Vec<(&String, &u32)> = self.0.iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let observed = by_count.into_iter().filter(|(_, n)| **n >= 3).map(|(tag, _)| tag.clone());
        let mut out: Vec<String> = Vec::new();
        for name in POPULAR.iter().map(|s| s.to_string()).chain(observed) {
            if !out.iter().any(|seen| seen.eq_ignore_ascii_case(&name)) {
                out.push(name);
            }
        }
        out.truncate(limit);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_tags() {
        assert_eq!(group_tag("[SubsPlease] Fate Strange Fake - 13 (1080p)"), Some("SubsPlease"));
        assert_eq!(group_tag("[Erai-raws] Fate/Strange Fake - 13"), Some("Erai-raws"));
        assert_eq!(group_tag("[1080p] Something"), None);
        assert_eq!(group_tag("Fate strange Fake Whispers of Dawn (2023) MULTi"), None);
        assert_eq!(group_tag("[notKaleido-mini] and [Kaleido-mini] Show"), Some("notKaleido-mini"));
    }

    #[test]
    fn popular_merges_seeds_and_observed() {
        let mut counts = GroupCounts::default();
        counts.0.insert("subsplease".into(), 50);
        counts.0.insert("Rare".into(), 1);
        counts.0.insert("NewGroup".into(), 9);
        let popular = counts.popular(50);
        assert_eq!(popular[0], "SubsPlease");
        assert!(popular.contains(&"NewGroup".to_string()));
        assert!(!popular.contains(&"Rare".to_string()));
        assert_eq!(popular.iter().filter(|n| n.eq_ignore_ascii_case("subsplease")).count(), 1);
    }
}
