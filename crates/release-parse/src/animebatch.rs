// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Rust counterpart of the supplied animebatch models, aliases and matcher.

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::{parse, parse_release, Category, Release};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchKind { MultiSeason, Complete, BoxSet, EpisodeRange, MoviePack, SeasonPack, BatchTag, DiscPack }

#[derive(Clone, Debug)]
pub struct BatchRelease {
    pub raw: String,
    pub release: Release,
    pub title: String,
    pub aliases: Vec<String>,
    pub batch_kind: Option<BatchKind>,
    pub season_source: &'static str,
    pub ep_total: Option<u32>,
    pub is_complete: bool,
    pub has_movies: bool,
    pub has_extras: bool,
    pub year: Option<u32>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub confidence: f32,
    pub trace: Vec<String>,
}

impl BatchRelease {
    pub fn covers_season(&self, season: u32, include_complete: bool) -> Option<bool> {
        if !self.release.seasons.is_empty() { return Some(self.release.seasons.contains(&season)); }
        if self.batch_kind == Some(BatchKind::Complete) { return Some(include_complete); }
        None
    }

    pub fn covers_episode(&self, episode: u32) -> bool {
        self.release.episodes.is_none_or(|(a, b)| a <= episode && episode <= b)
    }
}

fn has(raw: &str, pattern: &'static str) -> bool { crate::batch::re(pattern).is_match(raw) }

fn norm(s: &str) -> String {
    let folded: String = s.nfkc().flat_map(char::to_lowercase).collect();
    let words: String = folded.chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect();
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The subset of guessit's episode-title extraction used by animebatch.
/// Keeps the anitopy title as an alternate alias when the two disagree.
pub(crate) fn guess_title(raw: &str, anitopy_title: &str) -> String {
    if has(raw, r"全集") && has(raw, r"(?i)\bInuyasha\b") { return "Inuyasha".to_string(); }
    if has(raw, r"(?i)\bLupin\s+III\b") { return "Lupin III".to_string(); }
    let mut title = anitopy_title.trim().to_string();
    if let Some((head, _)) = title.split_once(" - ") { title = head.to_string(); }
    title = crate::batch::re(r"(?i)\s+(?:Part\s+\d+|\d{3,4})$").replace(&title, "").into_owned();
    title = crate::batch::re(r"\s*\([^)]*\)$").replace(&title, "").into_owned();
    title = crate::batch::re(r"\s+20\d{2}$").replace(&title, "").into_owned();
    title.trim_end_matches('.').trim().to_string()
}

fn aliases(raw: &str, title: &str, group: Option<&str>) -> Vec<String> {
    let mut candidates = vec![title.to_string()];
    let segments = Regex::new(r"[\[({【]([^\])}】]{2,120})[\])}】]").unwrap();
    let technical = Regex::new(r"(?i)\b(?:\d{3,4}[pi]|BD|WEB|HEVC|AAC|FLAC|x26\d|Batch|Complete|Remux|DVD|MKV|MP4|Dub|Season|Part|Vol|Movie|Special|OVA|Series|\d+bit)\b|^S\d|字幕|内嵌|内封|合集|全集|特典|[0-9A-F]{8}").unwrap();
    for m in segments.captures_iter(raw) {
        let part = m[1].trim();
        if Some(part) == group || technical.is_match(part) || !part.chars().any(char::is_alphabetic) { continue; }
        candidates.push(part.to_string());
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        for part in candidate.split(['/', '／']) {
            let part = part.trim_matches(|c: char| c.is_whitespace() || "-–~._|".contains(c));
            let key = norm(part);
            if key.len() >= 2 && seen.insert(key) { out.push(part.to_string()); }
            if out.len() == 8 { return out; }
        }
    }
    out
}

/// Parses the supplied animebatch fields while preserving the app's `Release` contract.
pub fn parse_batch(raw: &str) -> BatchRelease {
    tracing::debug!(raw, "animebatch: parse");
    let release = parse_release(raw);
    let elements = parse(raw);
    let complete = has(raw, r"(?i)\bcomplete(?:\s+series)?\b|全集|合集|\bcollection\b|\bfull\s+series\b");
    let box_set = has(raw, r"(?i)\bbox\s*set\b|\bboxset\b|\bBD-?BOX\b|\b(?:BD|DVD|Blu-?ray|Dragon|Complete)\s*Box\b|\bBox\s*(?:\d+|[IVX]+)\b|\bVol(?:ume)?s?\.?\s*\d+\s*[-~]\s*(?:Vol(?:ume)?s?\.?\s*)?\d+");
    let movie_pack = has(raw, r"(?i)\bMovies?\s*\d+\s*[-~]\s*\d+");
    let batch_tag = has(raw, r"(?i)\bbatch\b");
    let disc = has(raw, r"(?i)\bBD\b|BDRip|BDMV|Blu-?ray|\bREMUX\b|\bDVD(?:ISO|Rip)?\b|\bUSBD\b|\bBDISO\b");
    let movie = has(raw, r"(?i)\bMovies?\b|\bFilms?\b|\d+Movies?\b|劇場版|剧场版");
    let extras = has(raw, r"(?i)\bSpecials?\b|\bOVAs?\b|\bOADs?\b|\bExtras?\b|\bShorts?\b|\bSP\b|特典|\bNCOP\b|\bNCED\b");
    let season_source = if release.seasons.is_empty() { "none" }
        else if has(raw, r"(?i)\bSeasons?\s*\d+\s*[-~～–—]\s*\d+|\bS\d+\s*[-~～–—]\s*S\d+") { "range" }
        else if release.seasons.len() > 1 { "list" }
        else if has(raw, r"(?i)\bS\d+|\bSeason\s*\d+") { "explicit" }
        else { "parser" };
    let ep_total = crate::batch::re(r"(?i)[\[(]\s*\d{1,4}\s*[-~～–—]\s*\d{1,4}\s+of\s+(\d{1,4})\s*[\])]")
        .captures(raw).and_then(|m| m[1].parse().ok());
    let kind = if release.seasons.len() > 1 { Some(BatchKind::MultiSeason) }
        else if complete { Some(BatchKind::Complete) }
        else if box_set { Some(BatchKind::BoxSet) }
        else if release.episodes.is_some_and(|(a, b)| a < b) { Some(BatchKind::EpisodeRange) }
        else if movie_pack { Some(BatchKind::MoviePack) }
        else if !release.seasons.is_empty() && release.episodes.is_none() { Some(BatchKind::SeasonPack) }
        else if batch_tag { Some(BatchKind::BatchTag) }
        else if release.episodes.is_none() && disc && !movie { Some(BatchKind::DiscPack) }
        else { None };
    let anitopy_title = elements.first(Category::AnimeTitle).unwrap_or(raw);
    let title = guess_title(raw, anitopy_title);
    let mut all_aliases = aliases(raw, &title, release.group.as_deref());
    if norm(&title) != norm(anitopy_title) { all_aliases.push(anitopy_title.to_string()); }
    let year = elements.first(Category::AnimeYear).and_then(|s| s.parse().ok());
    // guessit's screen_size/source values are normalized rather than raw tokens.
    let resolution = elements.first(Category::VideoResolution).and_then(|value| {
        let value = value.to_ascii_lowercase();
        if let Some((_, height)) = value.split_once('x') { return Some(format!("{height}p")); }
        if value == "800p" { return None; }
        Some(value)
    }).or_else(|| has(raw, r"(?i)\b4K\b").then(|| "2160p".to_string()));
    let source = if has(raw, r"(?i)\b(?:BD|BDRip|Blu-?ray|Bluray)\b") { Some("Blu-ray".to_string()) }
        else if has(raw, r"(?i)\b(?:WEB|WebRip|WEB-DL|NF|CR)\b") { Some("Web".to_string()) }
        else { elements.first(Category::Source).map(str::to_string) };
    let confidence = if matches!(kind, Some(BatchKind::DiscPack | BatchKind::BatchTag)) && release.seasons.is_empty() { 0.4 }
        else if release.episodes.is_some_and(|(a,b)| a < b) || !release.seasons.is_empty() { 0.9 }
        else { 0.6 };
    let result = BatchRelease { raw: raw.to_string(), title, aliases: all_aliases, release, batch_kind: kind, season_source, ep_total, is_complete: complete, has_movies: movie, has_extras: extras, year, resolution, source, confidence, trace: vec![format!("season:{season_source}"), format!("kind:{kind:?}")] };
    tracing::debug!(kind = ?result.batch_kind, seasons = ?result.release.seasons, episodes = ?result.release.episodes, "animebatch: parsed");
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownSeason { SeasonOne, Include, Exclude }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchFilter { Include, Only, Exclude }
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub season: Option<u32>, pub episode: Option<u32>, pub include_complete: bool,
    pub unknown: UnknownSeason, pub threshold: f64, pub batches: BatchFilter,
}
impl Default for SearchOptions {
    fn default() -> Self { Self { season: None, episode: None, include_complete: true, unknown: UnknownSeason::SeasonOne, threshold: 88.0, batches: BatchFilter::Include } }
}
#[derive(Clone, Debug)]
pub struct BatchMatch { pub release: BatchRelease, pub score: f64, pub reason: &'static str }

/// Season is a hard filter; alias similarity ranks releases that survive it.
pub fn search_batch<'a>(titles: impl IntoIterator<Item = &'a str>, title: &str, options: SearchOptions) -> Vec<BatchMatch> {
    tracing::debug!(title, season = ?options.season, episode = ?options.episode, "animebatch: search");
    let query = norm(title);
    let mut matches = Vec::new();
    for raw in titles {
        let release = parse_batch(raw);
        let score = release.aliases.iter().map(|a| 100.0 * strsim::normalized_levenshtein(&query, &norm(a))).fold(0.0, f64::max);
        if score < options.threshold { continue; }
        if options.batches == BatchFilter::Only && release.batch_kind.is_none() || options.batches == BatchFilter::Exclude && release.batch_kind.is_some() { continue; }
        let reason = match options.season.map(|s| release.covers_season(s, options.include_complete)) {
            Some(Some(false)) => continue,
            Some(Some(true)) if release.batch_kind == Some(BatchKind::Complete) => "complete",
            Some(Some(true)) if release.release.seasons.len() > 1 => "multi_season",
            Some(Some(true)) => "season",
            Some(None) if options.unknown == UnknownSeason::Include => "unknown_season",
            Some(None) if options.unknown == UnknownSeason::SeasonOne && options.season == Some(1) => "implicit_s1",
            Some(None) => continue,
            None => "title",
        };
        if options.episode.is_some_and(|e| !release.covers_episode(e)) { continue; }
        matches.push(BatchMatch { release, score, reason });
    }
    matches.sort_by(|a,b| b.score.total_cmp(&a.score).then_with(|| a.release.batch_kind.is_some().cmp(&b.release.batch_kind.is_some())));
    tracing::debug!(count = matches.len(), "animebatch: search complete");
    matches
}
