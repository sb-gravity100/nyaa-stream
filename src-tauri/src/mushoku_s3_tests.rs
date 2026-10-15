//! Offline reproduction of the "Mushoku Tensei Season 3 shows few sources /
//! shows stray `Episode N` rows" report (PLAN.md "Mushoku S3 report").
//! `#[ignore]`d, env driven, prints a report:
//!
//! ```text
//! NYAA_DB_COPY=<copy of %APPDATA%/nyaa-stream/nyaa.db> \
//! MUSHOKU_CRAWL=<nyaa-listing-mushoku_tensei_s03e01-p1.json> \
//! [FRANCHISE_JSON=<%LOCALAPPDATA%/nyaa-stream/franchise/178789.json>] \
//! [ANIME_LISTS=<%LOCALAPPDATA%/nyaa-stream/anime_lists.json>] \
//!   cargo test -p nyaa-stream --lib mushoku_s3 -- --ignored --nocapture
//! ```
//!
//! The store copy is opened by `NyaaClient::with_storage` like the app does
//! (never point it at the live database). The remote search is emulated with
//! the store's own `search_hits` for the query "mushoku tensei" (the 1000
//! results nyaa.si served), the local-store merge with `search_local`.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use anilist_client::{AniListClient, Franchise};
use nyaa_client::{Category, EpisodeLabel, NyaaClient, NyaaResult};
use rusqlite::{Connection, OpenFlags};

use crate::anime_lists::AnimeLists;
use crate::title_match::{build_candidates, Decision, ShowMatcher};

const ENTRY: i64 = 178789;
const ENGLISH: &str = "Mushoku Tensei: Jobless Reincarnation Season 3";
const ROMAJI: &str = "Mushoku Tensei III: Isekai Ittara Honki Dasu";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn cache(sub: &str) -> PathBuf {
    dirs::cache_dir().unwrap().join("nyaa-stream").join(sub)
}

/// The rows `groupedSources` (src/App.tsx) files a kept release under for an
/// entry of season `entry`; empty when its season filters drop it. A mirror of
/// the TypeScript for this report only (no scraped file lists, no episode offset).
pub(crate) fn front_groups(result: &NyaaResult, entry: u32) -> Vec<String> {
    let mut label = result.label.clone();
    if let EpisodeLabel::Unknown = label {
        label = EpisodeLabel::Batch { season: entry, episode_range: None, extras: false, seasons: vec![] };
    } else if result.season_ambiguous == Some(true) && result.aid_trusted != Some(true) && entry != 1 {
        if let EpisodeLabel::Episode { seasons, .. } | EpisodeLabel::Batch { seasons, .. } = &label {
            if seasons.is_empty() {
                return Vec::new();
            }
        }
    }
    let (season, seasons) = match &mut label {
        EpisodeLabel::Episode { season, seasons, .. } | EpisodeLabel::Batch { season, seasons, .. } => (season, seasons),
        EpisodeLabel::Movie => return vec!["Movie".into()],
        _ => return vec!["Specials".into()],
    };
    if result.aid_trusted == Some(true) && *season != 0 && *season != entry && !seasons.contains(&entry) {
        *season = entry;
        *seasons = vec![entry];
    }
    let marked = !seasons.is_empty();
    if entry != 1 && marked && *season != entry && !seasons.contains(&entry) {
        return Vec::new();
    }
    let text = |s: u32| if s == 1 { String::new() } else { format!("Season {s} ") };
    match &label {
        EpisodeLabel::Episode { season, number, .. } => vec![format!("{}Episode {number}", text(*season))],
        EpisodeLabel::Batch { season, episode_range: Some([a, b]), .. } => (*a..=*b).map(|n| format!("{}Episode {n}", text(*season))).collect(),
        EpisodeLabel::Batch { season, .. } => vec![format!("{}Batch", text(*season))],
        _ => unreachable!(),
    }
}

#[tokio::test]
#[ignore = "needs NYAA_DB_COPY and MUSHOKU_CRAWL"]
async fn mushoku_s3_report() {
    let db = env_path("NYAA_DB_COPY").expect("NYAA_DB_COPY");
    let crawl = env_path("MUSHOKU_CRAWL").expect("MUSHOKU_CRAWL");
    let franchise_path = env_path("FRANCHISE_JSON").unwrap_or_else(|| cache("franchise").join("178789.json"));
    let franchise: Franchise = serde_json::from_slice(&std::fs::read(franchise_path).expect("franchise json")).expect("franchise parse");
    let lists = AnimeLists::load(env_path("ANIME_LISTS").unwrap_or_else(|| cache("anime_lists.json")));
    let tvdb = lists.tvdb_season(ENTRY);
    let entry_aids = lists.anidb_ids(ENTRY as u32);
    let synonyms: Vec<String> = match AniListClient::new().get_by_id(ENTRY).await {
        Ok(media) => media.synonyms,
        Err(err) => {
            println!("AniList unavailable ({err}), no synonyms");
            Vec::new()
        }
    };
    println!("tvdb season {tvdb:?}, entry aids {entry_aids:?}, synonyms {synonyms:?}");
    let candidates = build_candidates(Some(ENGLISH), Some(ROMAJI), &synonyms, tvdb, Some(&franchise));
    let matcher = ShowMatcher::new(Some(ENGLISH), Some(ROMAJI), &synonyms, tvdb).with_franchise(&franchise);
    let entry_season = matcher.primary_season().unwrap_or(1);
    println!("primary season {entry_season}, candidates {candidates:?}");

    let client = NyaaClient::with_storage(std::env::temp_dir().join("mushoku-s3-cache"), &db);
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let hits: HashSet<i64> = conn
        .prepare("SELECT release_id FROM search_hits WHERE key = '1_0|mushoku tensei'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    println!("search_hits for the query 'mushoku tensei': {}", hits.len());

    // The local merge: every stored release with the candidate words (what search_local_releases returns).
    let mut stored = Vec::new();
    for candidate in &candidates {
        stored.extend(client.search_local(candidate, Category::AllAnime).await);
    }
    let mut seen = HashSet::new();
    stored.retain(|r| seen.insert(r.view_url.clone()));
    println!("search_local merge: {} releases", stored.len());

    for r in stored.iter_mut() {
        if matcher.decide(&r.title, None, &entry_aids) == Decision::UseTitleRules && matcher.matches(&r.title) {
            crate::annotate_title_decided(&matcher, r);
        }
    }
    let decide = |r: &NyaaResult| -> (bool, Decision) {
        let d = matcher.decide(&r.title, None, &entry_aids);
        let keep = match d {
            Decision::Accept => true,
            Decision::Reject => false,
            Decision::UseTitleRules => matcher.matches(&r.title),
        };
        (keep, d)
    };
    let id_of = |r: &NyaaResult| nyaa_client::release_id(&r.view_url).unwrap_or(0);

    let manual: serde_json::Value = serde_json::from_slice(&std::fs::read(crawl).unwrap()).unwrap();
    println!("\n=== the manual crawl ({} items) ===", manual["items"].as_array().unwrap().len());
    for item in manual["items"].as_array().unwrap() {
        let id = item["id"].as_i64().unwrap();
        let in_hits = hits.contains(&id);
        match stored.iter().find(|r| id_of(r) == id) {
            None => println!("{id} {} NOT IN LOCAL MERGE in_1000={in_hits} {}", item["categoryId"], item["title"]),
            Some(r) => {
                let (keep, d) = decide(r);
                let group = if keep { front_groups(r, entry_season) } else { Vec::new() };
                println!("{id} {} in_1000={in_hits} keep={keep} {d:?} label={:?} group={group:?}\n    {}", item["categoryId"], r.label, r.title);
            }
        }
    }

    println!("\n=== page rows ===");
    let remote: Vec<&NyaaResult> = stored.iter().filter(|r| hits.contains(&id_of(r))).collect();
    report_rows(&remote, &decide, entry_season, "the 1000 nyaa results");
    let all: Vec<&NyaaResult> = stored.iter().collect();
    report_rows(&all, &decide, entry_season, "the full local merge");
}

fn report_rows(rows: &[&NyaaResult], decide: &dyn Fn(&NyaaResult) -> (bool, Decision), entry_season: u32, name: &str) {
    let mut groups: BTreeMap<String, Vec<&NyaaResult>> = BTreeMap::new();
    let (mut kept, mut dropped_front) = (0, 0);
    for r in rows {
        if !decide(r).0 {
            continue;
        }
        kept += 1;
        let rows = front_groups(r, entry_season);
        if rows.is_empty() {
            dropped_front += 1;
        }
        for g in rows {
            groups.entry(g).or_default().push(r);
        }
    }
    println!("--- {name}: {} releases, {kept} kept by the matcher, {dropped_front} dropped by the season filter", rows.len());
    for (g, rs) in &groups {
        println!("  {g}: {}", rs.len());
        if g.starts_with("Episode ") || g.starts_with("Batch") {
            for r in rs.iter().take(8) {
                println!("      {} {}", nyaa_client::release_id(&r.view_url).unwrap_or(0), r.title);
            }
        }
    }
}

/// Title-only pass over a full nyaa listing crawl (`{"items":[{id,date,title,..}]}`), no store
/// copy needed: runs the app's `EpisodeLabel::of` and `ShowMatcher` on every title for the
/// Season 3 entry and writes one TSV row each (id, date, keep, decision, label, title) to
/// `MUSHOKU_OUT`. Used to find naming styles the rules miss (PLAN.md "Learned release-to-entry matcher").
///
/// ```text
/// MUSHOKU_CRAWL=<nyaa-crawl-mushoku_tensei-14pages.json> MUSHOKU_OUT=<out.tsv> \
///   cargo test -p nyaa-stream --lib mushoku_crawl_titles -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "needs MUSHOKU_CRAWL and MUSHOKU_OUT"]
async fn mushoku_crawl_titles() {
    let crawl = env_path("MUSHOKU_CRAWL").expect("MUSHOKU_CRAWL");
    let out = env_path("MUSHOKU_OUT").expect("MUSHOKU_OUT");
    let franchise_path = env_path("FRANCHISE_JSON").unwrap_or_else(|| cache("franchise").join("178789.json"));
    let franchise: Franchise = serde_json::from_slice(&std::fs::read(franchise_path).expect("franchise json")).expect("franchise parse");
    let lists = AnimeLists::load(env_path("ANIME_LISTS").unwrap_or_else(|| cache("anime_lists.json")));
    let tvdb = lists.tvdb_season(ENTRY);
    let entry_aids = lists.anidb_ids(ENTRY as u32);
    let synonyms: Vec<String> = match AniListClient::new().get_by_id(ENTRY).await {
        Ok(media) => media.synonyms,
        Err(err) => {
            println!("AniList unavailable ({err}), no synonyms");
            Vec::new()
        }
    };
    let matcher = ShowMatcher::new(Some(ENGLISH), Some(ROMAJI), &synonyms, tvdb).with_franchise(&franchise);
    println!("tvdb season {tvdb:?}, entry aids {entry_aids:?}, primary season {:?}", matcher.primary_season());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(crawl).unwrap()).unwrap();
    let mut tsv = String::from("id\tdate\tkeep\tdecision\tlabel\ttitle\n");
    for item in json["items"].as_array().unwrap() {
        let title = item["title"].as_str().unwrap();
        let d = matcher.decide(title, None, &entry_aids);
        let keep = match d {
            Decision::Accept => true,
            Decision::Reject => false,
            Decision::UseTitleRules => matcher.matches(title),
        };
        let label = EpisodeLabel::of(title);
        tsv.push_str(&format!("{}\t{}\t{keep}\t{d:?}\t{label:?}\t{}\n", item["id"], item["date"].as_str().unwrap_or(""), title.replace('\t', " ")));
    }
    std::fs::write(out, tsv).unwrap();
}

/// Peer-consistent season (PLAN.md L1c) over the Mushoku crawl: prints every release that
/// `nyaa_client::peer_seasons` gives a season, as `date | season | title`, and the count.
/// No truth here; the list is reviewed by hand.
///
/// ```text
/// MUSHOKU_CRAWL=<nyaa-crawl-mushoku_tensei-14pages.json> \
///   cargo test -p nyaa-stream --lib mushoku_peer_season -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs MUSHOKU_CRAWL"]
fn mushoku_peer_season() {
    let crawl = env_path("MUSHOKU_CRAWL").expect("MUSHOKU_CRAWL");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(crawl).unwrap()).unwrap();
    let items = json["items"].as_array().unwrap();
    let pairs: Vec<(&str, &str)> = items.iter().map(|item| (item["title"].as_str().unwrap_or(""), item["date"].as_str().unwrap_or(""))).collect();
    let inputs: Vec<nyaa_client::PeerRelease> = pairs.iter().map(|(title, published)| nyaa_client::PeerRelease { title, published }).collect();
    let seasons = nyaa_client::peer_seasons(&inputs);
    let mut propagated = 0u32;
    for ((title, date), season) in pairs.iter().zip(&seasons) {
        if let Some(season) = season {
            propagated += 1;
            println!("{date} | {season} | {title}");
        }
    }
    println!("propagated {propagated} of {} releases", items.len());
}
