//! Recall/precision of `ShowMatcher` and the query count of `build_candidates`
//! over the hand-labelled real nyaa.si titles in
//! `tests/fixtures/nyaa_titles/franchises.json`.
//!
//! Ground truth: the fixture labels each release with the TV seasons it
//! contains (`[]` = movie/OVA/special, `other_show` = a different show). For a
//! TV entry with fixture season S a release is "true" iff its label contains S.
//! Non-TV entries and the TV spin-off without a season have no per-entry
//! ground truth (the fixture does not say which OVA a `[]` release is), so they
//! only get their candidate queries printed. `unsure` releases are skipped.
//! Absolute-numbered releases with no season marker are labelled with their one
//! real season, so keeping them on other seasons' lists counts as "wrongly
//! kept" here even though PLAN.md accepts that on purpose.

use std::collections::BTreeMap;

use anilist_client::{AnimeTitle, Franchise, Relative};
use serde_json::Value;

use crate::title_match::{build_candidates, ShowMatcher};

const FIXTURE: &str = include_str!("../tests/fixtures/nyaa_titles/franchises.json");

struct Entry {
    franchise: String,
    id: i64,
    format: String,
    season: Option<u32>,
    english: Option<String>,
    romaji: Option<String>,
    synonyms: Vec<String>,
}

struct Release {
    franchise: String,
    title: String,
    seasons: Option<Vec<u32>>,
    unsure: bool,
    other_show: bool,
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn load() -> (Vec<Entry>, Vec<Release>) {
    let json: Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let entries = json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| Entry {
            franchise: e["franchise"].as_str().unwrap().to_string(),
            id: e["id"].as_i64().unwrap(),
            format: e["format"].as_str().unwrap().to_string(),
            season: e["season"].as_u64().map(|s| s as u32),
            english: opt_str(&e["english"]),
            romaji: opt_str(&e["romaji"]),
            synonyms: e["synonyms"].as_array().unwrap().iter().filter_map(opt_str).collect(),
        })
        .collect();
    let releases = json["releases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Release {
            franchise: r["franchise"].as_str().unwrap().to_string(),
            title: r["title"].as_str().unwrap().to_string(),
            seasons: r["seasons"].as_array().map(|a| a.iter().map(|s| s.as_u64().unwrap() as u32).collect()),
            unsure: r["unsure"].as_bool().unwrap_or(false),
            other_show: r["other_show"].as_bool().unwrap_or(false),
        })
        .collect();
    (entries, releases)
}

fn is_tv(format: &str) -> bool {
    matches!(format, "TV" | "TV_SHORT")
}

/// The franchise as the app derives it: the entry's format, whether a TV
/// entry precedes it (fixture order is chronological), the others as relatives.
fn franchise_of(entries: &[Entry], index: usize) -> Franchise {
    let entry = &entries[index];
    let has_tv_prequel = is_tv(&entry.format) && entries[..index].iter().any(|e| e.franchise == entry.franchise && is_tv(&e.format));
    let relatives = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| *i != index && e.franchise == entry.franchise)
        .map(|(i, e)| Relative {
            id: e.id,
            title: AnimeTitle { english: e.english.clone(), romaji: e.romaji.clone(), native: None },
            synonyms: e.synonyms.clone(),
            format: Some(e.format.clone()),
            relation: if !is_tv(&e.format) {
                "SIDE_STORY"
            } else if i < index {
                "PREQUEL"
            } else {
                "SEQUEL"
            }
            .to_string(),
        })
        .collect();
    Franchise { format: Some(entry.format.clone()), has_tv_prequel, relatives }
}

/// `Some(truth)` when the fixture gives ground truth for this entry/release.
fn truth(entry: &Entry, release: &Release) -> Option<bool> {
    if release.unsure || !is_tv(&entry.format) {
        return None;
    }
    let season = entry.season?;
    if release.other_show {
        return Some(false);
    }
    Some(release.seasons.as_ref()?.contains(&season))
}

#[derive(Default)]
struct Stats {
    truths: usize,
    kept: usize,
    wrongly_kept: Vec<String>,
    wrongly_dropped: Vec<String>,
}

fn run(entries: &[Entry], releases: &[Release], with_tvdb: bool) -> BTreeMap<String, Stats> {
    let mut out = BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        if !is_tv(&entry.format) || entry.season.is_none() {
            continue;
        }
        let tvdb = if with_tvdb { entry.season } else { None };
        let matcher = ShowMatcher::new(entry.english.as_deref(), entry.romaji.as_deref(), &entry.synonyms, tvdb).with_franchise(&franchise_of(entries, i));
        let mut stats = Stats::default();
        for release in releases.iter().filter(|r| r.franchise == entry.franchise) {
            let Some(expected) = truth(entry, release) else { continue };
            let got = matcher.matches(&release.title);
            if expected {
                stats.truths += 1;
                if got {
                    stats.kept += 1;
                } else {
                    stats.wrongly_dropped.push(release.title.clone());
                }
            } else if got {
                stats.wrongly_kept.push(release.title.clone());
            }
        }
        let name = entry.english.clone().or_else(|| entry.romaji.clone()).unwrap_or_default();
        out.insert(format!("{} {} S{} ({})", entry.franchise, entry.id, entry.season.unwrap(), name), stats);
    }
    out
}

fn report(label: &str, stats: &BTreeMap<String, Stats>) {
    println!("=== {label} ===");
    println!("{:<90} {:>5} {:>5} {:>6} {:>7}", "entry", "true", "kept", "w.kept", "w.drop");
    for (name, s) in stats {
        println!("{:<90} {:>5} {:>5} {:>6} {:>7}", name.chars().take(90).collect::<String>(), s.truths, s.kept, s.wrongly_kept.len(), s.wrongly_dropped.len());
        for t in s.wrongly_dropped.iter().take(10) {
            println!("      DROPPED: {t}");
        }
        for t in s.wrongly_kept.iter().take(10) {
            println!("      KEPT   : {t}");
        }
    }
}

/// (entry id, recall floor, wrongly-kept ceiling) for tvdb = None, then the
/// same two for tvdb = the entry's season. Observed value with a margin.
const FLOORS: &[(i64, f64, usize, f64, usize)] = &[
    (100773, 0.07, 2, 0.30, 2),
    (109963, 0.40, 2, 0.50, 3),
    (114043, 0.39, 2, 0.51, 2),
    (20923, 0.95, 3, 0.95, 3),
    (21518, 0.69, 2, 0.74, 2),
    (99255, 0.54, 2, 0.54, 2),
    (108465, 0.79, 3, 0.79, 3),
    (127720, 0.76, 128, 0.95, 3),
    (146065, 0.89, 8, 0.89, 8),
    (166873, 0.92, 8, 0.92, 8),
    (178789, 0.95, 8, 0.95, 8),
    (217434, 0.66, 3, 0.66, 3),
    (101280, 0.95, 4, 0.95, 4),
    (108511, 0.95, 4, 0.95, 4),
    (116742, 0.95, 4, 0.95, 4),
    (156822, 0.95, 4, 0.95, 4),
    (182205, 0.95, 4, 0.95, 4),
    (217331, 0.28, 2, 0.28, 2),
];

#[test]
fn title_match_fixture_recall_and_precision() {
    let (entries, releases) = load();
    let plain = run(&entries, &releases, false);
    let tvdb = run(&entries, &releases, true);
    report("tvdb_season = None", &plain);
    report("tvdb_season = fixture entry season", &tvdb);
    for (label, stats) in [("none", &plain), ("tvdb", &tvdb)] {
        for (name, s) in stats {
            let recall = if s.truths == 0 { 1.0 } else { s.kept as f64 / s.truths as f64 };
            let id: i64 = name.split(' ').nth(1).unwrap().parse().unwrap();
            let Some(floor) = FLOORS.iter().find(|f| f.0 == id) else { continue };
            let (min_recall, max_wrongly_kept) = if label == "none" { (floor.1, floor.2) } else { (floor.3, floor.4) };
            assert!(recall >= min_recall, "[{label}] {name}: recall {recall:.3} < {min_recall}");
            assert!(s.wrongly_kept.len() <= max_wrongly_kept, "[{label}] {name}: {} wrongly kept > {max_wrongly_kept}", s.wrongly_kept.len());
        }
    }
}

#[test]
fn title_match_fixture_candidate_queries() {
    let (entries, _) = load();
    for (index, entry) in entries.iter().enumerate() {
        let franchise = franchise_of(&entries, index);
        for tvdb in [None, entry.season] {
            let candidates = build_candidates(entry.english.as_deref(), entry.romaji.as_deref(), &entry.synonyms, tvdb, Some(&franchise));
            println!("{} {} {} tvdb={:?}: {:?}", entry.franchise, entry.id, entry.format, tvdb, candidates);
            assert!(candidates.len() <= 4, "{} {}: {} queries", entry.franchise, entry.id, candidates.len());
            assert!(!candidates.is_empty(), "{} {}: no query", entry.franchise, entry.id);
        }
    }
}

/// T8 verification harness over a built AnimeTosho dataset (`#[ignore]`, env-driven).
#[path = "tosho_verify_tests.rs"]
mod tosho_verify;
