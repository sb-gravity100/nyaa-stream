// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Batch and season-range rules ported from the supplied animebatch/rules.py.

use std::sync::OnceLock;
use regex::Regex;
use crate::release::{Kind, Release};

pub(crate) fn re(pattern: &'static str) -> &'static Regex {
    // The small fixed pattern set is compiled once; leaking the Regex avoids
    // holding the cache lock while matching.
    static REFS: OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, &'static Regex>>> = OnceLock::new();
    let refs = REFS.get_or_init(Default::default);
    let mut refs = refs.lock().unwrap();
    refs.entry(pattern).or_insert_with(|| Box::leak(Box::new(Regex::new(pattern).unwrap())))
}

fn range(raw: &str, pattern: &'static str) -> Option<(u32, u32)> {
    re(pattern).captures(raw).and_then(|m| {
        let a: u32 = m.get(1)?.as_str().parse().ok()?;
        let b: u32 = m.get(2)?.as_str().parse().ok()?;
        (a < b && b <= 3000).then_some((a, b))
    })
}

pub(crate) fn apply(raw: &str, r: &mut Release) {
    let season_word = range(raw, r"(?i)\bSeasons?\s*(\d{1,2})\s*[-~～–—]\s*(?:Seasons?\s*)?(\d{1,2})\b");
    let season_s = range(raw, r"(?i)\bS(\d{1,2})\s*[-~～–—]\s*S(\d{1,2})\b");
    if let Some((a, b)) = season_word.or(season_s).filter(|(_, b)| *b <= 60) {
        r.seasons = (a..=b).collect();
    } else if r.seasons.is_empty() {
        let parts: Vec<u32> = re(r"(?i)\bS(\d{1,2})\s*Part\s*\d")
            .captures_iter(raw).filter_map(|m| m[1].parse().ok()).collect();
        if !parts.is_empty() { r.seasons = parts; r.seasons.sort_unstable(); r.seasons.dedup(); }
    }

    // A roman numeral that is part of the series name is not a season.
    if re(r"(?i)\bLupin\s+III\b").is_match(raw) && !re(r"(?i)\b(?:S\d|Season\s*\d)").is_match(raw) {
        r.seasons.clear();
    }

    let episode_of = range(raw, r"(?i)[\[(]\s*(\d{1,4})\s*[-~～–—]\s*(\d{1,4})\s+of\s+\d{1,4}\s*[\])]");
    let cjk_range = range(raw, r"(?i)[\[(]\s*(\d{1,4})\s*[-~～–—]\s*(\d{1,4})TV全集");
    let spaced_wide = range(raw, r"(?i)\b(\d{3,4})\s+-\s+(\d{3,4})\b");
    if let Some(episodes) = episode_of.or(cjk_range).or(spaced_wide) {
        r.episodes = Some(episodes);
    }

    let complete = re(r"(?i)\bcomplete(?:\s+series)?\b|全集|合集|\bcollection\b|\bfull\s+series\b").is_match(raw);
    let box_set = re(r"(?i)\bbox\s*set\b|\bboxset\b|\bBD-?BOX\b|\b(?:BD|DVD|Blu-?ray|Dragon|Complete)\s*Box\b|\bBox\s*(?:\d+|[IVX]+)\b").is_match(raw);
    let movie_pack = range(raw, r"(?i)\bMovies?\s*(\d{1,3})\s*[-~～–—]\s*(\d{1,3})\b").is_some();
    if movie_pack { r.episodes = None; }
    if !r.seasons.is_empty() && r.episodes.is_none() || complete || box_set || movie_pack {
        r.kind = Kind::Batch;
    } else if r.episodes.is_some_and(|(a,b)| a != b) { r.kind = Kind::Batch; }
}
