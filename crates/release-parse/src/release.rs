// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// nyaa-stream's typed view of the parsed elements.

use std::sync::OnceLock;

use regex::Regex;

use crate::element::{Category, Elements};

/// Season number standing in for "Final Season" (no digit in the title):
/// consistent across releases and never a real season. Same value as
/// `FINAL_SEASON_NUMBER` in the frontend.
pub const FINAL_SEASON: u32 = 9001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One episode.
    Episode,
    /// Several episodes: a range, a season pack or a multi-season pack.
    Batch,
    Movie,
    /// OVA / OAD / ONA / special (not on a TV season's episode grid).
    Special,
    /// Nothing says what it holds (TITLE_ANALYSIS.md §6).
    Unknown,
}

/// What a release title says it contains.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// The show name as written (anitopy's anime_title).
    pub title: Option<String>,
    /// Seasons named, ascending, deduplicated. Empty = no season marker.
    pub seasons: Vec<u32>,
    /// First and last episode (equal for a single episode).
    pub episodes: Option<(u32, u32)>,
    pub kind: Kind,
    /// A season pack that also bundles movies/OVAs/specials ("S01+OVA").
    pub extras: bool,
    pub group: Option<String>,
    pub version: Option<u32>,
}

fn re_final_season() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bfinal\s*season\b").unwrap())
}

fn re_extras() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bS\d{1,2}\s*\+\s*\S|\+\s*(?:movies?|films?|specials?|tv\s*specials?|ovas?|oads?|extras?|bonus)").unwrap())
}

fn number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(c)).map(|c| if c.is_ascii_digit() { c } else { char::from(b'0' + (c as u32 - 0xFF10) as u8) }).collect();
    digits.parse().ok()
}

impl Release {
    pub fn from_elements(title: &str, e: &Elements) -> Release {
        // An SxxEyy season outranks every other season marker.
        let source = if e.episode_seasons().is_empty() { e.get(Category::AnimeSeason) } else { e.episode_seasons() };
        let mut seasons: Vec<u32> = source.iter().filter_map(|s| number(s)).collect();
        seasons.sort_unstable();
        seasons.dedup();
        if seasons.is_empty() && re_final_season().is_match(title) {
            seasons.push(FINAL_SEASON);
        }

        // Whole episode numbers only: "07.5" and "4a" are specials/parts.
        let eps: Vec<u32> = e.get(Category::EpisodeNumber).iter().filter(|s| s.chars().all(|c| c.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(&c))).filter_map(|s| number(s)).collect();
        let episodes = match (eps.iter().min(), eps.iter().max()) {
            (Some(&a), Some(&b)) => Some((a, b)),
            _ => None,
        };

        let types: Vec<String> = e.get(Category::AnimeType).iter().map(|t| t.to_uppercase()).collect();
        let info: Vec<String> = e.get(Category::ReleaseInformation).iter().map(|t| t.to_uppercase()).collect();
        let is_movie = types.iter().any(|t| t == "MOVIE" || t == "GEKIJOUBAN");
        let is_special = types.iter().any(|t| matches!(t.as_str(), "OVA" | "OAD" | "OAV" | "ONA" | "SPECIAL" | "SPECIALS" | "SP"));
        let batch_word = info.iter().any(|t| t == "BATCH" || t == "COMPLETE");

        let kind = match episodes {
            Some((a, b)) if a != b => Kind::Batch,
            Some(_) if seasons.len() > 1 => Kind::Batch,
            Some(_) if batch_word => Kind::Batch,
            Some(_) if is_special && seasons.is_empty() => Kind::Special,
            Some(_) => Kind::Episode,
            None if !seasons.is_empty() || batch_word || e.contains(Category::VolumeNumber) => Kind::Batch,
            None if is_movie => Kind::Movie,
            None if is_special => Kind::Special,
            None => Kind::Unknown,
        };

        Release {
            title: e.first(Category::AnimeTitle).map(str::to_string),
            seasons,
            episodes,
            kind,
            extras: kind == Kind::Batch && re_extras().is_match(title),
            group: e.first(Category::ReleaseGroup).map(str::to_string),
            version: e.first(Category::ReleaseVersion).and_then(number),
        }
    }

    /// The season this release is filed under: its first season, else 1
    /// (nyaa convention: no marker = season 1 or a season-less show).
    pub fn season(&self) -> u32 {
        self.seasons.first().copied().unwrap_or(1)
    }
}

/// Parses a release title into a `Release`.
pub fn parse_release(title: &str) -> Release {
    let e = crate::parse(title);
    Release::from_elements(title, &e)
}
