//! Static stem -> AniDB id mapper (PLAN.md "AnimeTosho dataset", T9).
//!
//! A *stem* is the release's show name as `release-parse` reads it (season
//! marker and episode number are separate fields there, so they are already
//! out), normalised the way `title_match.rs` compares titles. The dataset
//! builder counts `stem -> aid` votes over the dump's releases and keeps only
//! the stems that one `aid` clearly owns; the app looks a release with no
//! dataset `aid` up in that table as an `approved = 0` hint.
//!
//! The table is trained offline from the dump alone and is never updated from
//! data collected later, so there is no feedback loop.

use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;

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

/// Vote thresholds: a stem is kept when one `aid` holds at least
/// `min_share` of its votes and has at least `min_releases` releases or
/// `min_groups` distinct groups.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainConfig {
    pub min_share: f64,
    pub min_releases: u32,
    pub min_groups: u32,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self { min_share: 0.90, min_releases: 3, min_groups: 2 }
    }
}

/// One dataset release as training or test data.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub stem: String,
    pub group: String,
    pub aid: u32,
    /// `resolveapproved`: 0 = AnimeTosho's unconfirmed guess, 1-3 trusted.
    pub approved: u8,
}

/// A trained stem: its `aid` and the number of releases that voted for it.
pub type StemTable = HashMap<String, (u32, u32)>;

#[derive(Default)]
struct Votes {
    releases: u32,
    groups: BTreeSet<String>,
}

/// Counts `stem -> aid` votes and keeps the stems that pass `config`.
/// Trusted releases (`approved` 1-3) always vote; a guess (`approved` 0)
/// votes only when every release of its group for that stem (guesses and
/// trusted alike) agrees on the same `aid`, so a group's own contradiction
/// never feeds the table. Conflicting stems are dropped, not guessed.
pub fn train(samples: &[Sample], config: TrainConfig) -> StemTable {
    let mut per_group: HashMap<(&str, &str), BTreeSet<u32>> = HashMap::new();
    for s in samples {
        per_group.entry((s.stem.as_str(), s.group.as_str())).or_default().insert(s.aid);
    }
    let mut votes: HashMap<&str, HashMap<u32, Votes>> = HashMap::new();
    for s in samples {
        if s.approved == 0 && per_group[&(s.stem.as_str(), s.group.as_str())].len() != 1 {
            continue;
        }
        let v = votes.entry(s.stem.as_str()).or_default().entry(s.aid).or_default();
        v.releases += 1;
        v.groups.insert(s.group.clone());
    }
    let mut table = StemTable::new();
    for (stem, by_aid) in votes {
        let total: u32 = by_aid.values().map(|v| v.releases).sum();
        let Some((aid, top)) = by_aid.iter().max_by_key(|(aid, v)| (v.releases, std::cmp::Reverse(**aid))) else { continue };
        let share = f64::from(top.releases) / f64::from(total);
        if share >= config.min_share && (top.releases >= config.min_releases || top.groups.len() as u32 >= config.min_groups) {
            table.insert(stem.to_string(), (*aid, top.releases));
        }
    }
    tracing::info!(samples = samples.len(), stems = table.len(), ?config, "stem table trained");
    table
}

/// Result of testing a table on releases with known `aid`s.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Evaluation {
    pub test: u64,
    /// Test releases whose stem the table knows.
    pub resolved: u64,
    pub correct: u64,
    /// Test releases whose `aid` also occurs in the training data (series
    /// the table could know; newer series have no stems by design).
    pub known_series: u64,
    pub known_series_resolved: u64,
}

impl Evaluation {
    pub fn coverage(&self) -> f64 {
        ratio(self.resolved, self.test)
    }
    pub fn precision(&self) -> f64 {
        ratio(self.correct, self.resolved)
    }
    /// Coverage among the test releases of series the training data holds.
    pub fn known_series_coverage(&self) -> f64 {
        ratio(self.known_series_resolved, self.known_series)
    }
}

fn ratio(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

/// Looks every test sample up in `table` (training data `train_aids` only
/// tells which series were seen).
pub fn evaluate(table: &StemTable, train_aids: &BTreeSet<u32>, test: &[Sample]) -> Evaluation {
    let mut e = Evaluation::default();
    for s in test {
        e.test += 1;
        let known = train_aids.contains(&s.aid);
        e.known_series += u64::from(known);
        if let Some((aid, _)) = table.get(&s.stem) {
            e.resolved += 1;
            e.known_series_resolved += u64::from(known);
            e.correct += u64::from(*aid == s.aid);
        }
    }
    e
}

/// Dataset releases with an `aid` as `(published, sample)`, read from a built
/// dataset (`release_aid` joined with `releases`). Releases whose title has no
/// usable stem are skipped.
pub fn load_samples(conn: &Connection) -> anyhow::Result<Vec<(String, Sample)>> {
    let mut stmt = conn.prepare("SELECT r.title, r.published, a.aid, a.approved FROM release_aid a JOIN releases r ON r.id = a.nyaa_id")?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let title: String = row.get(0)?;
        let Some((stem, group)) = parse_stem(&title) else { continue };
        out.push((
            row.get::<_, String>(1)?,
            Sample { stem, group, aid: row.get::<_, i64>(2)?.max(0) as u32, approved: row.get::<_, i64>(3)?.clamp(0, 255) as u8 },
        ));
    }
    Ok(out)
}

/// Writes `table` into the dataset's `stem_aid` (replacing its rows).
pub fn write_table(conn: &Connection, table: &StemTable) -> anyhow::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS stem_aid (stem TEXT PRIMARY KEY, aid INTEGER NOT NULL, votes INTEGER NOT NULL); DELETE FROM stem_aid; BEGIN")?;
    {
        let mut stmt = conn.prepare("INSERT INTO stem_aid (stem, aid, votes) VALUES (?1, ?2, ?3)")?;
        let mut rows: Vec<_> = table.iter().collect();
        rows.sort();
        for (stem, (aid, votes)) in rows {
            stmt.execute(rusqlite::params![stem, aid, votes])?;
        }
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// Trains on the dataset's own releases and writes `stem_aid`; returns the row count.
pub fn train_dataset(conn: &Connection, config: TrainConfig) -> anyhow::Result<u64> {
    let samples: Vec<Sample> = load_samples(conn)?.into_iter().map(|(_, s)| s).collect();
    let table = train(&samples, config);
    write_table(conn, &table)?;
    Ok(table.len() as u64)
}

/// Holdout: trains on releases published before `cutoff` (`YYYY-MM-DD`) and
/// evaluates on those from `cutoff` on that AnimeTosho trusts (`approved` 1-3,
/// the ground truth). Writes nothing.
pub fn holdout(conn: &Connection, cutoff: &str, config: TrainConfig) -> anyhow::Result<(usize, Evaluation)> {
    let all = load_samples(conn)?;
    let (before, after): (Vec<_>, Vec<_>) = all.into_iter().partition(|(published, _)| published.as_str() < cutoff);
    let train_set: Vec<Sample> = before.into_iter().map(|(_, s)| s).collect();
    let test: Vec<Sample> = after.into_iter().map(|(_, s)| s).filter(|s| s.approved >= 1).collect();
    let table = train(&train_set, config);
    let aids: BTreeSet<u32> = train_set.iter().map(|s| s.aid).collect();
    let eval = evaluate(&table, &aids, &test);
    tracing::info!(cutoff, train = train_set.len(), stems = table.len(), ?eval, "stem holdout evaluated");
    Ok((table.len(), eval))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(stem: &str, group: &str, aid: u32, approved: u8) -> Sample {
        Sample { stem: stem.into(), group: group.into(), aid, approved }
    }

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

    #[test]
    fn keeps_only_stems_one_aid_owns() {
        let mut s = Vec::new();
        for i in 0..4 {
            s.push(sample("owned", "g1", 10, 1 + (i % 3) as u8));
        }
        // 2 of 10 votes elsewhere: share 80% < 90%, dropped.
        for _ in 0..8 {
            s.push(sample("contested", "g1", 20, 1));
        }
        for _ in 0..2 {
            s.push(sample("contested", "g2", 21, 1));
        }
        // Too few releases and a single group.
        s.push(sample("rare", "g1", 30, 1));
        s.push(sample("rare", "g1", 30, 1));
        // Two releases from two groups pass on groups.
        s.push(sample("pair", "g1", 40, 3));
        s.push(sample("pair", "g2", 40, 3));
        let t = train(&s, TrainConfig::default());
        assert_eq!(t.get("owned"), Some(&(10, 4)));
        assert_eq!(t.get("pair"), Some(&(40, 2)));
        assert!(!t.contains_key("contested") && !t.contains_key("rare"));
    }

    #[test]
    fn guesses_vote_only_when_their_group_is_consistent() {
        let mut s = Vec::new();
        // g1 contradicts itself on "mixed": its guess does not vote, the trusted rows do.
        s.extend([sample("mixed", "g1", 5, 1), sample("mixed", "g1", 5, 1), sample("mixed", "g1", 5, 1), sample("mixed", "g1", 6, 0)]);
        // g2 is unanimous: its guesses vote.
        s.extend([sample("guessy", "g2", 7, 0), sample("guessy", "g2", 7, 0), sample("guessy", "g2", 7, 0)]);
        let t = train(&s, TrainConfig::default());
        assert_eq!(t.get("mixed"), Some(&(5, 3)));
        assert_eq!(t.get("guessy"), Some(&(7, 3)));
    }

    #[test]
    fn evaluation_counts_coverage_and_precision() {
        let table: StemTable = [("a".to_string(), (1, 9)), ("b".to_string(), (2, 9))].into_iter().collect();
        let seen: BTreeSet<u32> = [1, 2].into_iter().collect();
        let test = vec![sample("a", "", 1, 1), sample("b", "", 3, 1), sample("zzz", "", 1, 1), sample("new", "", 99, 1)];
        let e = evaluate(&table, &seen, &test);
        assert_eq!((e.test, e.resolved, e.correct, e.known_series, e.known_series_resolved), (4, 2, 1, 2, 1));
        assert!((e.coverage() - 0.5).abs() < 1e-9 && (e.precision() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn dataset_round_trip_and_holdout() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE releases (id INTEGER PRIMARY KEY, title TEXT, published TEXT);
             CREATE TABLE release_aid (nyaa_id INTEGER PRIMARY KEY, aid INTEGER, eid INTEGER, approved INTEGER);",
        )
        .unwrap();
        for i in 0..6 {
            let (date, n) = if i < 4 { ("2024-05-01 10:00", i) } else { ("2025-03-01 10:00", i) };
            conn.execute("INSERT INTO releases VALUES (?1, ?2, ?3)", rusqlite::params![i, format!("[SubsPlease] Sousou no Frieren - {:02} (1080p)", n + 1), date]).unwrap();
            conn.execute("INSERT INTO release_aid VALUES (?1, 17617, 0, 1)", [i]).unwrap();
        }
        let (stems, eval) = holdout(&conn, "2025-01-01", TrainConfig::default()).unwrap();
        assert_eq!((stems, eval.test, eval.resolved, eval.correct), (1, 2, 2, 2));
        assert_eq!(train_dataset(&conn, TrainConfig::default()).unwrap(), 1);
        let row: (String, i64, i64) = conn.query_row("SELECT stem, aid, votes FROM stem_aid", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(row, ("sousou no frieren".to_string(), 17617, 6));
    }
}
