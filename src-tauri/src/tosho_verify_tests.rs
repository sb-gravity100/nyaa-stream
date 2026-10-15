//! T8 verification of the AnimeTosho dataset work (PLAN.md "Verification
//! (T8)"). Offline harness, `#[ignore]`d: it needs a built dataset and prints
//! a report, it asserts nothing about the numbers.
//!
//! ```text
//! TOSHO_DATASET=<tosho-dataset-*.db> TOSHO_SCRATCH=<dir> [SYMPHO_CRAWL=<crawl.json>] \
//!   cargo test -p nyaa-stream --lib tosho_verify -- --ignored --nocapture
//! ```
//!
//! `TOSHO_SCRATCH` holds the downloaded Fribb map (`anime_lists.json`, fetched
//! by `AnimeLists` itself when missing) and the scratch release store the
//! dataset is imported into (`NyaaClient::import_tosho`, so the app's own
//! lookups are what get measured). Symphogear's entries come from AniList.
//! Datasets are never committed.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use anilist_client::{AniListClient, Franchise};
use nyaa_client::{check_batch, BatchIssue, EpisodeLabel, NyaaClient, ToshoRelease};
use rusqlite::{Connection, OpenFlags};

use super::{franchise_of, is_tv, load, Entry};
use crate::anime_lists::{decide_entry_season, AnimeLists, SeasonSource};
use crate::title_match::{Decision, ShowMatcher};

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn matcher(e: &Entry, f: &Franchise, tvdb: Option<u32>) -> ShowMatcher {
    ShowMatcher::new(e.english.as_deref(), e.romaji.as_deref(), &e.synonyms, tvdb).with_franchise(f)
}

/// What `filter_for_show` does per result: the id decides where it can.
fn combined(m: &ShowMatcher, title: &str, known: Option<&ToshoRelease>, aids: &[u32]) -> (bool, Decision) {
    let d = m.decide(title, known, aids);
    let keep = match d {
        Decision::Accept => true,
        Decision::Reject => false,
        Decision::UseTitleRules => m.matches(title),
    };
    (keep, d)
}

/// `groupedSources` season filters in src/App.tsx (rule 1: season-1 page drops
/// a lone later-season marker, rule 2: later-season page drops a marker naming
/// only other seasons), as they stood before `aid_trusted`.
fn frontend_drops(label: &EpisodeLabel, entry_season: u32) -> bool {
    let (season, seasons) = match label {
        EpisodeLabel::Episode { season, seasons, .. } | EpisodeLabel::Batch { season, seasons, .. } => (*season, seasons),
        _ => return false,
    };
    if entry_season == 1 {
        return season > 1 && seasons.len() < 2;
    }
    !seasons.is_empty() && season != entry_season && !seasons.contains(&entry_season)
}

fn name_of(e: &Entry) -> String {
    e.english.clone().or_else(|| e.romaji.clone()).unwrap_or_default()
}

/// Symphogear's TV seasons by TVDB numbering (what release groups write).
fn symphogear_season(romaji: &str) -> Option<u32> {
    let r = romaji.trim();
    if !r.starts_with("Senki Zesshou Symphogear") {
        return None;
    }
    let tail = r.trim_start_matches("Senki Zesshou Symphogear").trim();
    match tail {
        "" => Some(1),
        "G" => Some(2),
        "GX" => Some(3),
        "AXZ" => Some(4),
        "XV" => Some(5),
        _ => None,
    }
}

async fn symphogear(anilist: &AniListClient) -> Vec<(Entry, Franchise)> {
    let mut out = Vec::new();
    let found = anilist.search("Symphogear", 25).await.expect("AniList search");
    for media in found {
        println!("anilist symphogear candidate: {} {:?} {:?} {:?}", media.id, media.format, media.title.romaji, media.title.english);
        let Some(season) = media.title.romaji.as_deref().and_then(symphogear_season) else { continue };
        if !is_tv(media.format.as_deref().unwrap_or("")) {
            continue;
        }
        let franchise = anilist.franchise(media.id).await.expect("AniList franchise");
        let entry = Entry {
            franchise: "symphogear".into(),
            id: media.id,
            format: media.format.clone().unwrap_or_default(),
            season: Some(season),
            english: media.title.english.clone(),
            romaji: media.title.romaji.clone(),
            synonyms: media.synonyms.clone(),
        };
        out.push((entry, franchise));
    }
    out.sort_by_key(|(e, _)| e.season);
    out
}

#[derive(Default, Clone, Copy)]
struct Tally {
    tp: usize,
    fp: usize,
    fn_: usize,
    tn: usize,
}

impl Tally {
    fn add(&mut self, truth: bool, got: bool) {
        match (truth, got) {
            (true, true) => self.tp += 1,
            (false, true) => self.fp += 1,
            (true, false) => self.fn_ += 1,
            (false, false) => self.tn += 1,
        }
    }
    fn recall(&self) -> f64 {
        if self.tp + self.fn_ == 0 { f64::NAN } else { self.tp as f64 / (self.tp + self.fn_) as f64 }
    }
    fn precision(&self) -> f64 {
        if self.tp + self.fp == 0 { f64::NAN } else { self.tp as f64 / (self.tp + self.fp) as f64 }
    }
    fn show(&self) -> String {
        format!("tp {:>5} fp {:>5} fn {:>5} tn {:>6}  recall {:>6.3} precision {:>6.3}", self.tp, self.fp, self.fn_, self.tn, self.recall(), self.precision())
    }
}

#[derive(Clone)]
#[allow(dead_code)]
struct PoolRow {
    id: i64,
    title: String,
    known: Option<ToshoRelease>,
}

const FRANCHISE_WORDS: &[(&str, &[&str])] = &[
    ("foodwars", &["food wars", "shokugeki no soma", "shokugeki no souma"]),
    ("mushoku", &["mushoku tensei", "jobless reincarnation"]),
    ("slime", &["slime", "tensei shitara"]),
    ("symphogear", &["symphogear"]),
];

fn pool(conn: &Connection, faids: &[u32], words: &[&str]) -> Vec<PoolRow> {
    let aid_list = faids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let mut sql = String::from(
        "SELECT r.id, r.title, a.aid, a.eid, a.approved FROM releases r LEFT JOIN release_aid a ON a.nyaa_id = r.id WHERE 0",
    );
    if !aid_list.is_empty() {
        sql.push_str(&format!(" OR a.aid IN ({aid_list})"));
    }
    for i in 0..words.len() {
        sql.push_str(&format!(" OR r.title LIKE ?{}", i + 1));
    }
    let like: Vec<String> = words.iter().map(|w| format!("%{w}%")).collect();
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt
        .query_map(rusqlite::params_from_iter(like.iter()), |row| {
            let aid: Option<i64> = row.get(2)?;
            Ok(PoolRow {
                id: row.get(0)?,
                title: row.get(1)?,
                known: aid.map(|a| ToshoRelease { aid: a as u32, eid: row.get::<_, i64>(3).unwrap_or(0) as u32, approved: row.get::<_, i64>(4).unwrap_or(0) as u8 }),
            })
        })
        .unwrap();
    rows.map(Result::unwrap).collect()
}

#[tokio::test]
#[ignore = "needs TOSHO_DATASET (a built tosho dataset), TOSHO_SCRATCH and network for Fribb/AniList"]
async fn tosho_verify() {
    let dataset = env_path("TOSHO_DATASET").expect("TOSHO_DATASET");
    let scratch = env_path("TOSHO_SCRATCH").expect("TOSHO_SCRATCH");
    std::fs::create_dir_all(&scratch).unwrap();

    // The Fribb map (downloaded by the app's own code when missing) and the app store with the dataset imported.
    let lists = AnimeLists::load(scratch.join("anime_lists.json"));
    lists.refresh_if_stale().await;
    let client = NyaaClient::with_storage(scratch.join("cache"), &scratch.join("verify-store.db"));
    if client.tosho_info().await.is_none() {
        let stats = client.import_tosho(&dataset).await.expect("import dataset");
        println!("imported dataset: {stats:?}");
    }
    println!("tosho info: {:?}", client.tosho_info().await);
    let conn = Connection::open_with_flags(&dataset, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();

    // ---- subjects: fixture TV entries with a season + Symphogear ----
    let (entries, fixture_releases) = load();
    let franchises: Vec<Franchise> = (0..entries.len()).map(|i| franchise_of(&entries, i)).collect();
    let mut subjects: Vec<(Entry, Franchise)> = entries.into_iter().zip(franchises).filter(|(e, _)| is_tv(&e.format) && e.season.is_some()).collect();
    let anilist = AniListClient::new();
    subjects.extend(symphogear(&anilist).await);

    // ---- (1) season truth ----
    println!("\n=== (1) entry season: expected vs Fribb vs dataset vs decided ===");
    println!("{:<12} {:>7} {:<44} {:>3} {:>5} {:<28} {:>7} {:>5}  verdict", "franchise", "id", "name", "exp", "fribb", "aid:ds_season/offset", "decided", "fallb");
    let mut season_stats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut entry_season_of: HashMap<i64, u32> = HashMap::new();
    for (e, f) in &subjects {
        let exp = e.season.unwrap();
        let fribb = lists.tvdb_season(e.id);
        let aids = lists.anidb_ids(e.id as u32);
        let mut ds = Vec::new();
        let mut chosen = None;
        for aid in &aids {
            let row = client.anidb_tvdb(*aid).await;
            ds.push(match &row {
                Some(r) => format!("{aid}:{}/{}", r.season.map_or("-".into(), |s| s.to_string()), r.episode_offset),
                None => format!("{aid}:none"),
            });
            if chosen.is_none() && row.as_ref().is_some_and(|r| r.season.is_some_and(|s| s > 0)) {
                chosen = row;
            }
        }
        let fallback = matcher(e, f, fribb).primary_season();
        let (decided, source) = decide_entry_season(chosen.as_ref(), fallback);
        entry_season_of.insert(e.id, decided.unwrap_or(1));
        let ok = |s: Option<u32>| s == Some(exp);
        let verdict = match (ok(decided), ok(fribb), ok(fallback), source) {
            (true, true, _, _) => "ok (both)",
            (true, false, _, SeasonSource::Dataset) => "ok: dataset right, Fribb wrong/absent",
            (true, false, _, SeasonSource::Fallback) => "ok via fallback, Fribb alone wrong",
            (false, true, _, SeasonSource::Dataset) => "REGRESSION: dataset overrides a right Fribb",
            (false, _, true, SeasonSource::Dataset) => "REGRESSION: dataset overrides a right title/Fribb fallback",
            (false, _, _, _) => "WRONG (no source right)",
        };
        *season_stats.entry(verdict).or_default() += 1;
        println!(
            "{:<12} {:>7} {:<44} {:>3} {:>5} {:<28} {:>7} {:>5}  {verdict}",
            e.franchise,
            e.id,
            name_of(e).chars().take(44).collect::<String>(),
            exp,
            fribb.map_or("-".into(), |s| s.to_string()),
            ds.join(" "),
            format!("{}{}", decided.map_or("-".into(), |s| s.to_string()), if source == SeasonSource::Dataset { "D" } else { "F" }),
            fallback.map_or("-".into(), |s| s.to_string())
        );
    }
    println!("season verdict counts: {season_stats:?}");

    // ---- (2) release attribution on the dataset's own releases ----
    println!("\n=== (2) release attribution (dataset pool: franchise aids + title words) ===");
    let mut pools: HashMap<&str, Vec<PoolRow>> = HashMap::new();
    for (name, words) in FRANCHISE_WORDS {
        let faids: BTreeSet<u32> = subjects.iter().filter(|(e, _)| e.franchise == *name).flat_map(|(e, _)| lists.anidb_ids(e.id as u32)).collect();
        let rows = pool(&conn, &faids.iter().copied().collect::<Vec<_>>(), words);
        let trusted = rows.iter().filter(|r| r.known.is_some_and(|k| k.aid != 0 && k.approved > 0)).count();
        let hint = rows.iter().filter(|r| r.known.is_some_and(|k| k.aid != 0 && k.approved == 0)).count();
        println!("pool {name}: aids {faids:?}, {} releases ({trusted} trusted aid, {hint} hint aid, {} no aid)", rows.len(), rows.len() - trusted - hint);
        pools.insert(name, rows);
    }
    let (mut tot_title, mut tot_comb) = (Tally::default(), Tally::default());
    let (mut tot_hint_in, mut tot_hint_back, mut tot_hint_new) = (0usize, 0usize, 0usize);
    let (mut fe_accepted, mut fe_dropped) = (0usize, 0usize);
    let mut fe_samples: Vec<String> = Vec::new();
    for (e, f) in &subjects {
        let aids = lists.anidb_ids(e.id as u32);
        if aids.is_empty() {
            println!("-- {} {} S{}: no AniDB mapping, id rules inactive", e.franchise, e.id, e.season.unwrap());
            continue;
        }
        let m = matcher(e, f, lists.tvdb_season(e.id));
        let (mut tt, mut tc) = (Tally::default(), Tally::default());
        let (mut fixed_fp, mut fixed_fn): (Vec<&PoolRow>, Vec<&PoolRow>) = (Vec::new(), Vec::new());
        let (mut hint_in, mut hint_new, mut hint_back) = (0, 0, 0);
        let mut hint_new_samples = Vec::new();
        for row in &pools[e.franchise.as_str()] {
            let Some(k) = row.known.filter(|k| k.aid != 0) else { continue };
            let title_ok = m.matches(&row.title);
            let (comb_ok, d) = combined(&m, &row.title, Some(&k), &aids);
            if k.approved > 0 {
                let truth = aids.contains(&k.aid);
                tt.add(truth, title_ok);
                tc.add(truth, comb_ok);
                if comb_ok && d == Decision::Accept {
                    fe_accepted += 1;
                    if frontend_drops(&EpisodeLabel::of(&row.title), entry_season_of[&e.id]) {
                        fe_dropped += 1;
                        if fe_samples.len() < 12 {
                            fe_samples.push(format!("S{} {}: {}", entry_season_of[&e.id], e.id, row.title));
                        }
                    }
                }
                if title_ok && !truth {
                    fixed_fp.push(row);
                }
                if !title_ok && truth {
                    fixed_fn.push(row);
                }
            } else if aids.contains(&k.aid) {
                hint_in += 1;
                if d == Decision::UseTitleRules {
                    hint_back += 1;
                }
                if comb_ok && !title_ok {
                    hint_new += 1;
                    if hint_new_samples.len() < 4 {
                        hint_new_samples.push(row.title.clone());
                    }
                }
            }
        }
        tot_title.tp += tt.tp;
        tot_title.fp += tt.fp;
        tot_title.fn_ += tt.fn_;
        tot_title.tn += tt.tn;
        tot_comb.tp += tc.tp;
        tot_comb.fp += tc.fp;
        tot_comb.fn_ += tc.fn_;
        tot_comb.tn += tc.tn;
        tot_hint_in += hint_in;
        tot_hint_back += hint_back;
        tot_hint_new += hint_new;
        println!("-- {} {} S{} {} aids {:?}", e.franchise, e.id, e.season.unwrap(), name_of(e), aids);
        println!("   trusted  title-rules : {}", tt.show());
        println!("   trusted  id+title    : {}", tc.show());
        println!("   hint aid in set: {hint_in}; decide leaves to title (names other season): {hint_back}; hint adds over title rules: {hint_new}");
        for s in &hint_new_samples {
            println!("      HINT-ACCEPT (title rules reject): {s}");
        }
        for r in fixed_fp.iter().take(4) {
            println!("      title rules WRONGLY KEPT (aid {}): {}", r.known.unwrap().aid, r.title);
        }
        for r in fixed_fn.iter().take(4) {
            println!("      title rules WRONGLY DROPPED (aid {}): {}", r.known.unwrap().aid, r.title);
        }
        let _ = (fixed_fp.len(), fixed_fn.len());
    }
    println!("TOTAL trusted title-rules: {}", tot_title.show());
    println!("TOTAL trusted id+title   : {}", tot_comb.show());
    println!("TOTAL hint (approved 0) with aid in the entry's set: {tot_hint_in}; left to title rules (names only other seasons): {tot_hint_back}; accepted beyond the title rules: {tot_hint_new}");
    println!("FRONTEND season filters (emulated) on trusted-aid Accepts: {fe_dropped} of {fe_accepted} would be dropped by groupedSources");
    for s in &fe_samples {
        println!("      FE-DROP {s}");
    }

    // ---- (2b) independent check: the hand labels of the fixture vs the dataset's aid on the same titles ----
    println!("\n=== (2b) fixture hand labels vs dataset aid (exact-title join) ===");
    let titles: HashSet<&str> = fixture_releases.iter().map(|r| r.title.as_str()).collect();
    let mut by_title: HashMap<String, ToshoRelease> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT r.title, a.aid, a.eid, a.approved FROM releases r JOIN release_aid a ON a.nyaa_id = r.id").unwrap();
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, ToshoRelease { aid: row.get::<_, i64>(1)? as u32, eid: row.get::<_, i64>(2)? as u32, approved: row.get::<_, i64>(3)? as u8 }))).unwrap();
        for row in rows {
            let (t, k) = row.unwrap();
            if titles.contains(t.as_str()) {
                by_title.insert(t, k);
            }
        }
    }
    println!("fixture releases {} of which in the dataset with an aid: {} (trusted {})", fixture_releases.len(), by_title.len(), by_title.values().filter(|k| k.approved > 0).count());
    let (mut ft, mut fc) = (Tally::default(), Tally::default());
    let (mut ftr, mut fcr) = (Tally::default(), Tally::default());
    let (mut regressions, mut improvements) = (0usize, 0usize);
    for (e, f) in subjects.iter().filter(|(e, _)| e.franchise != "symphogear") {
        let aids = lists.anidb_ids(e.id as u32);
        let m = matcher(e, f, lists.tvdb_season(e.id));
        let (mut tt, mut tc) = (Tally::default(), Tally::default());
        let mut diffs = Vec::new();
        for r in fixture_releases.iter().filter(|r| r.franchise == e.franchise) {
            let Some(k) = by_title.get(&r.title) else { continue };
            if r.unsure {
                continue;
            }
            let truth = if r.other_show { false } else if let Some(s) = &r.seasons { s.contains(&e.season.unwrap()) } else { continue };
            let known = if aids.is_empty() { None } else { Some(k) };
            let title_ok = m.matches(&r.title);
            let (comb_ok, d) = combined(&m, &r.title, known, &aids);
            tt.add(truth, title_ok);
            tc.add(truth, comb_ok);
            let (trusted_t, trusted_c) = (k.approved > 0, k.approved > 0);
            if trusted_t {
                ftr.add(truth, title_ok);
            }
            if trusted_c {
                fcr.add(truth, comb_ok);
            }
            if comb_ok != truth && title_ok == truth {
                diffs.push(format!("REGRESSION {} ({:?}, aid {} appr {}, label seasons {:?}): {}", if comb_ok { "now kept" } else { "now dropped" }, d, k.aid, k.approved, r.seasons, r.title));
                regressions += 1;
            } else if comb_ok == truth && title_ok != truth {
                improvements += 1;
            }
        }
        ft.tp += tt.tp;
        ft.fp += tt.fp;
        ft.fn_ += tt.fn_;
        ft.tn += tt.tn;
        fc.tp += tc.tp;
        fc.fp += tc.fp;
        fc.fn_ += tc.fn_;
        fc.tn += tc.tn;
        println!("-- {} {} S{} {} aids {:?}", e.franchise, e.id, e.season.unwrap(), name_of(e), aids);
        println!("   title rules : {}", tt.show());
        println!("   id+title    : {}", tc.show());
        for d in &diffs {
            println!("      {d}");
        }
    }
    println!("fixture-labelled decisions changed by the aid: {regressions} regressions (title rules right, id+title wrong), {improvements} improvements");
    println!("TOTAL fixture-labelled, found in dataset: title rules {}", ft.show());
    println!("TOTAL fixture-labelled, found in dataset: id+title    {}", fc.show());
    println!("  only trusted-aid releases: title rules {}", ftr.show());
    println!("  only trusted-aid releases: id+title    {}", fcr.show());

    // ---- (3) Symphogear XV crawl ----
    if let Some(crawl) = env_path("SYMPHO_CRAWL") {
        println!("\n=== (3) Symphogear crawl {} ===", crawl.display());
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&crawl).unwrap()).unwrap();
        let items = json["items"].as_array().unwrap();
        let ids: Vec<i64> = items.iter().filter_map(|i| i["id"].as_i64()).collect();
        let known = client.aids_of(&ids).await;
        println!("crawl items {}, known to the dataset {}", items.len(), known.len());
        for (e, f) in subjects.iter().filter(|(e, _)| e.franchise == "symphogear") {
            let aids = lists.anidb_ids(e.id as u32);
            let m = matcher(e, f, lists.tvdb_season(e.id));
            let (mut kept, mut kept_title) = (Vec::new(), 0usize);
            for item in items {
                let id = item["id"].as_i64().unwrap_or(0);
                let title = item["title"].as_str().unwrap_or("");
                let k = known.get(&id);
                let (ok, d) = combined(&m, title, k, &aids);
                if m.matches(title) {
                    kept_title += 1;
                }
                if ok {
                    kept.push((id, title.to_string(), d, k.copied()));
                }
            }
            println!("-- S{} {} (id {}) aids {:?}: kept {} by id+title, {} by title rules alone", e.season.unwrap(), name_of(e), e.id, aids, kept.len(), kept_title);
            if e.season == Some(5) {
                let mut eps = BTreeSet::new();
                let (mut batches, mut other) = (0, 0);
                for (id, title, d, k) in &kept {
                    let label = EpisodeLabel::of(title);
                    match &label {
                        EpisodeLabel::Episode { number, .. } => {
                            eps.insert(*number);
                        }
                        EpisodeLabel::Batch { .. } => batches += 1,
                        _ => other += 1,
                    }
                    println!("   KEPT {id} {d:?} {:?} {label:?} | {title}", k.map(|k| (k.aid, k.approved, k.eid)));
                }
                println!("   XV kept episodes {eps:?}, batches {batches}, other {other}");
                for item in items {
                    let id = item["id"].as_i64().unwrap_or(0);
                    if !kept.iter().any(|(k, ..)| *k == id) {
                        println!("   DROP {id} {:?} | {}", known.get(&id).map(|k| (k.aid, k.approved)), item["title"].as_str().unwrap_or(""));
                    }
                }
            }
        }
    }
}

/// Labeller coverage over the dataset's trusted releases (`approved` 1-3) whose AniDB entry has a
/// plain TVDB season (> 0, no `mapping` rules): writes one TSV row per release - TVDB season,
/// episode offset, what `EpisodeLabel::of` read - to `LABEL_OUT`, so the season/episode misses can
/// be counted by naming style (PLAN.md "Learned release-to-entry matcher", L1).
///
/// ```text
/// TOSHO_DATASET=<tosho-dataset-*.db> LABEL_OUT=<out.tsv> \
///   cargo test -p nyaa-stream --lib tosho_label_coverage -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs TOSHO_DATASET and LABEL_OUT"]
fn tosho_label_coverage() {
    let db = env_path("TOSHO_DATASET").expect("TOSHO_DATASET");
    let out = env_path("LABEL_OUT").expect("LABEL_OUT");
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.published, a.aid, a.eid, t.season, t.episode_offset, r.title \
             FROM releases r JOIN release_aid a ON a.nyaa_id = r.id \
             JOIN anidb_tvdb t ON t.anidb_id = a.aid \
             WHERE a.approved > 0 AND t.season > 0 AND t.mapping IS NULL",
        )
        .unwrap();
    let mut tsv = String::from("id\tpublished\taid\teid\ttvdb_season\toffset\tkind\tlabel_season\tmarked\tnumber\ttitle\n");
    let mut n = 0u32;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, String>(6)?)))
        .unwrap();
    for row in rows {
        let (id, published, aid, eid, season, offset, title) = row.unwrap();
        let (kind, ls, marked, number) = match EpisodeLabel::of(&title) {
            EpisodeLabel::Episode { season, number, seasons } => ("episode", season as i64, !seasons.is_empty(), number as i64),
            EpisodeLabel::Batch { season, episode_range, seasons, .. } => ("batch", season as i64, !seasons.is_empty(), episode_range.map(|r| r[0] as i64).unwrap_or(-1)),
            EpisodeLabel::Movie => ("movie", -1, false, -1),
            EpisodeLabel::Special => ("special", -1, false, -1),
            EpisodeLabel::Unknown => ("unknown", -1, false, -1),
            #[allow(unreachable_patterns)]
            _ => ("other", -1, false, -1),
        };
        tsv.push_str(&format!("{id}\t{published}\t{aid}\t{eid}\t{season}\t{offset}\t{kind}\t{ls}\t{marked}\t{number}\t{}\n", title.replace('\t', " ")));
        n += 1;
    }
    std::fs::write(out, tsv).unwrap();
    println!("labelled {n} trusted releases");
}

/// Multi-file torrents the labeller cannot fully map: runs `EpisodeLabel::of_path` on every video
/// file of every dataset torrent that has a stored file list, and counts torrents whose files are
/// all mapped, partly mapped, or not mapped, plus duplicate (season, episode) numbers. Writes the
/// unmapped video paths to `FILES_OUT` (TSV: nyaa_id, path) to find the naming styles behind them,
/// and, when `DUPS_OUT` is set, every file sharing a (season, episode) with another file of its torrent.
///
/// ```text
/// TOSHO_DATASET=<tosho-dataset-*.db> FILES_OUT=<out.tsv> \
///   cargo test -p nyaa-stream --lib tosho_file_label_coverage -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs TOSHO_DATASET and FILES_OUT"]
fn tosho_file_label_coverage() {
    let db = env_path("TOSHO_DATASET").expect("TOSHO_DATASET");
    let out = env_path("FILES_OUT").expect("FILES_OUT");
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut stmt = conn.prepare("SELECT nyaa_id, path FROM release_files ORDER BY nyaa_id, idx").unwrap();
    let is_video = |p: &str| {
        let l = p.to_lowercase();
        [".mkv", ".mp4", ".avi", ".ts", ".m2ts", ".webm", ".wmv", ".mov", ".flv", ".ogm"].iter().any(|e| l.ends_with(e))
    };
    let mut tsv = String::from("nyaa_id\tpath\n");
    let mut dups = String::from("nyaa_id\tseason\tnumber\tpath\n");
    let (mut torrents, mut no_video) = (0u32, 0u32);
    let (mut all_mapped, mut partly, mut none_mapped, mut dup_numbers) = (0u32, 0u32, 0u32, 0u32);
    let (mut files_total, mut files_unknown) = (0u64, 0u64);
    let mut cur: Option<i64> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut flush = |id: i64, paths: &mut Vec<String>, tsv: &mut String| {
        let video: Vec<&String> = paths.iter().filter(|p| is_video(p)).collect();
        torrents += 1;
        if video.is_empty() {
            no_video += 1;
            paths.clear();
            return;
        }
        let mut unknown = 0u32;
        let mut seen: BTreeMap<(u32, u32), Vec<&String>> = BTreeMap::new();
        for p in &video {
            files_total += 1;
            match EpisodeLabel::of_path(p) {
                EpisodeLabel::Unknown => {
                    unknown += 1;
                    files_unknown += 1;
                    tsv.push_str(&format!("{id}\t{}\n", p.replace('\t', " ")));
                }
                EpisodeLabel::Episode { season, number, .. } => seen.entry((season, number)).or_default().push(p),
                _ => {}
            }
        }
        if unknown == 0 {
            all_mapped += 1;
        } else if (unknown as usize) < video.len() {
            partly += 1;
        } else {
            none_mapped += 1;
        }
        if seen.values().any(|v| v.len() > 1) {
            dup_numbers += 1;
            for ((season, number), ps) in seen.iter().filter(|(_, v)| v.len() > 1) {
                for p in ps {
                    dups.push_str(&format!("{id}\t{season}\t{number}\t{}\n", p.replace('\t', " ")));
                }
            }
        }
        paths.clear();
    };
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).unwrap();
    for row in rows {
        let (id, path) = row.unwrap();
        if cur != Some(id) {
            if let Some(prev) = cur {
                flush(prev, &mut paths, &mut tsv);
            }
            cur = Some(id);
        }
        paths.push(path);
    }
    if let Some(prev) = cur {
        flush(prev, &mut paths, &mut tsv);
    }
    std::fs::write(out, tsv).unwrap();
    if let Some(dups_out) = env_path("DUPS_OUT") {
        std::fs::write(dups_out, dups).unwrap();
    }
    println!("torrents with file lists: {torrents} (no video file: {no_video})");
    println!("all video files mapped: {all_mapped}; partly unmapped: {partly}; none mapped: {none_mapped}");
    println!("torrents with duplicate (season, episode) numbers: {dup_numbers}");
    println!("video files: {files_total}, unmapped: {files_unknown}");
}

/// Running counts for `tosho_batch_check`, one `add` per release.
#[derive(Default)]
struct BatchTally {
    tsv: String,
    releases: u32,
    no_range: u32,
    checked: u32,
    verified_offset0: u32,
    verified_offset: u32,
    flagged: u32,
    missing: u32,
    out_of_range: u32,
    duplicates: u32,
    unread: u32,
}

impl BatchTally {
    fn add(&mut self, id: i64, title: &str, paths: &[String]) {
        self.releases += 1;
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let check = check_batch(title, &refs);
        if check.issues == [BatchIssue::NoTitleRange] {
            self.no_range += 1;
            return;
        }
        self.checked += 1;
        match (check.verified, check.offset) {
            (true, 0) => self.verified_offset0 += 1,
            (true, _) => self.verified_offset += 1,
            (false, _) => self.flagged += 1,
        }
        for issue in &check.issues {
            match issue {
                BatchIssue::Missing(_) => self.missing += 1,
                BatchIssue::OutOfRange(_) => self.out_of_range += 1,
                BatchIssue::Duplicates(_) => self.duplicates += 1,
                BatchIssue::Unread(_) => self.unread += 1,
                BatchIssue::NoTitleRange => {}
            }
        }
        self.tsv.push_str(&format!("{id}\t{}\t{}\t{:?}\t{}\n", check.verified, check.offset, check.issues, title.replace('\t', " ")));
    }
}

/// Runs `nyaa_client::check_batch` over every release of the dataset that has a stored file list,
/// using the release title and its file paths in listed order. Prints the verdict counts and writes
/// one TSV row per checked (titled-range) release to `BATCH_OUT`: id, verified, offset, issues, title.
///
/// ```text
/// TOSHO_DATASET=<tosho-dataset-*.db> BATCH_OUT=<out.tsv> \
///   cargo test -p nyaa-stream --lib tosho_batch_check -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs TOSHO_DATASET and BATCH_OUT"]
fn tosho_batch_check() {
    let db = env_path("TOSHO_DATASET").expect("TOSHO_DATASET");
    let out = env_path("BATCH_OUT").expect("BATCH_OUT");
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut stmt = conn.prepare("SELECT r.id, r.title, f.path FROM releases r JOIN release_files f ON f.nyaa_id = r.id ORDER BY r.id, f.idx").unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))).unwrap();

    let mut tally = BatchTally { tsv: String::from("id\tverified\toffset\tissues\ttitle\n"), ..Default::default() };
    let mut group: Option<(i64, String, Vec<String>)> = None;
    for row in rows {
        let (id, title, path) = row.unwrap();
        match &mut group {
            Some((gid, _, paths)) if *gid == id => paths.push(path),
            _ => {
                if let Some((gid, gtitle, paths)) = group.take() {
                    tally.add(gid, &gtitle, &paths);
                }
                group = Some((id, title, vec![path]));
            }
        }
    }
    if let Some((gid, gtitle, paths)) = group.take() {
        tally.add(gid, &gtitle, &paths);
    }

    std::fs::write(out, &tally.tsv).unwrap();
    println!("releases with file lists: {}", tally.releases);
    println!("no title range (NoTitleRange): {}", tally.no_range);
    println!("checked (title has a range): {}", tally.checked);
    println!("verified offset 0: {}; verified offset > 0: {}", tally.verified_offset0, tally.verified_offset);
    println!("flagged: {}", tally.flagged);
    println!("flagged by issue kind (a release may count in several): Missing {}, OutOfRange {}, Duplicates {}, Unread {}", tally.missing, tally.out_of_range, tally.duplicates, tally.unread);
}
