//! Builds the AnimeTosho dataset (`tosho-dataset-<date>.db`, see PLAN.md
//! "AnimeTosho dataset") from the MariaDB dump's SQL streams.
//!
//! Output schema (other code is written against it):
//! - `meta(key, value)`: `kind = 'tosho'`, `dump_date` (YYYY-MM-DD),
//!   `built_at` (unix seconds), row counts (`releases`, `release_aid`,
//!   `release_files`, `release_files_torrents`, `anidb_tvdb`, `stem_aid`).
//! - `release_aid(nyaa_id, aid, eid, approved)`: nyaa.si anime releases
//!   AnimeTosho resolved to AniDB ids; `approved` is `resolveapproved`
//!   (0 = AnimeTosho's unconfirmed guess, 1/2/3 trusted).
//! - `release_files(nyaa_id, idx, path, size)`: file lists of those
//!   releases that hold more than one file.
//! - `anidb_tvdb(anidb_id, tvdb_id, season, episode_offset, mapping)`.
//! - `stem_aid(stem, aid, votes)`: the static stem mapper trained on the
//!   dump's own releases (`stem.rs`, PLAN.md T9).
//! - `releases` (+ the empty query-cache tables): the same schema and row
//!   shape as `store.rs`, so `Store::import` merges it like any release
//!   database; `category` is nyaa.si's code (legacy `1_37`/`1_11`/`1_38`/`1_32`
//!   are mapped, empty = `1_2`). `words` is left empty (the import recomputes it); `updated_at`
//!   is the dump date, so a row the app scraped itself (live seeders) wins.
//!
//! The dump lists `toto_files` before `toto_toto`, so file rows are staged
//! in a scratch database next to the output and joined at the end.

use std::io::BufRead;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::stem::{self, TrainConfig};
use crate::store::{INSERT_RELEASE_IF_NEW, RELEASE_TABLES};
use crate::tosho::{Header, InsertReader, Value};

const DATASET_TABLES: &str = "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
     CREATE TABLE release_aid (
         nyaa_id INTEGER PRIMARY KEY,
         aid INTEGER NOT NULL,
         eid INTEGER NOT NULL,
         approved INTEGER NOT NULL
     );
     CREATE TABLE release_files (
         nyaa_id INTEGER NOT NULL,
         idx INTEGER NOT NULL,
         path TEXT NOT NULL,
         size INTEGER NOT NULL,
         PRIMARY KEY (nyaa_id, idx)
     ) WITHOUT ROWID;
     CREATE TABLE anidb_tvdb (
         anidb_id INTEGER PRIMARY KEY,
         tvdb_id INTEGER,
         season INTEGER,
         episode_offset INTEGER NOT NULL,
         mapping TEXT
     );
     CREATE TABLE stem_aid (
         stem TEXT PRIMARY KEY,
         aid INTEGER NOT NULL,
         votes INTEGER NOT NULL
     );";

/// Row counts of a finished build.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BuildStats {
    pub releases: u64,
    /// Of `releases`, how many came from the nyaa.si archive scrape.
    pub archive_releases: u64,
    pub release_aid: u64,
    pub release_files: u64,
    /// Releases that have a file list.
    pub release_files_torrents: u64,
    pub anidb_tvdb: u64,
    /// Rows of the trained `stem_aid` table.
    pub stem_aid: u64,
}

/// Writes the dataset into a new database file. Feed it `read_toto` (and
/// optionally `read_archive`), then `finish`.
pub struct ToshoBuilder {
    conn: Connection,
    out: PathBuf,
    stage: PathBuf,
    dump_date: String,
    dump_unix: i64,
    stats: BuildStats,
    stem_config: TrainConfig,
}

fn num(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        Value::Float(f) => *f as i64,
        _ => 0,
    }
}

fn column(header: &Header, name: &str) -> anyhow::Result<usize> {
    header.index(name).ok_or_else(|| anyhow::anyhow!("table {} has no column `{name}`", header.table))
}

/// Unix seconds of 00:00 UTC on `YYYY-MM-DD`.
pub fn parse_date(date: &str) -> anyhow::Result<i64> {
    let parts: Vec<i64> = date.split('-').map(|p| p.parse::<i64>()).collect::<Result<_, _>>().map_err(|_| anyhow::anyhow!("bad date {date:?}, want YYYY-MM-DD"))?;
    let [y, m, d] = parts[..] else { anyhow::bail!("bad date {date:?}, want YYYY-MM-DD") };
    anyhow::ensure!((1..=12).contains(&m) && (1..=31).contains(&d), "bad date {date:?}");
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok((era * 146097 + doe - 719468) * 86400)
}

/// `YYYY-MM-DD HH:MM` (UTC), the way nyaa.si's listing shows dates.
fn format_published(unix: i64) -> String {
    let days = unix.div_euclid(86400);
    let secs = unix.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, secs % 3600 / 60)
}

/// nyaa.si's size column: `1.5 GiB`, `300.0 MiB`, `512 Bytes`.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} Bytes");
    }
    let mut value = bytes as f64;
    let mut unit = "KiB";
    for next in ["KiB", "MiB", "GiB", "TiB"] {
        value /= 1024.0;
        unit = next;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

/// AnimeTosho's `nyaa_cat` mixes nyaa.si codes with nyaa.se's legacy ones
/// (`1_37` English-translated, `1_11` non-English, `1_38` raw, `1_32` AMV);
/// the app searches with nyaa.si's, and an empty value means English-translated.
fn nyaa_si_category(cat: &str) -> String {
    match cat {
        "" | "1_37" => "1_2",
        "1_11" => "1_3",
        "1_38" => "1_4",
        "1_32" => "1_1",
        other => other,
    }
    .to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `magnet:?xt=urn:btih:<hex>&dn=<title>`.
fn magnet(hash: &[u8], title: &str) -> String {
    format!("magnet:?xt=urn:btih:{}&dn={}", hex(hash), urlencoding::encode(title))
}

/// A release row ready to insert.
struct ReleaseRow<'a> {
    id: i64,
    category: &'a str,
    title: &'a str,
    hash: &'a [u8],
    size: u64,
    created: i64,
}

fn insert_release(stmt: &mut rusqlite::Statement<'_>, updated: i64, r: &ReleaseRow<'_>) -> anyhow::Result<bool> {
    let n = stmt.execute(params![
        r.id,
        r.category,
        r.title,
        magnet(r.hash, r.title),
        format!("https://nyaa.si/download/{}.torrent", r.id),
        format!("https://nyaa.si/view/{}", r.id),
        format_size(r.size),
        format_published(r.created),
        updated,
    ])?;
    Ok(n > 0)
}

impl ToshoBuilder {
    /// Creates `out` (replacing any file there). `dump_date` is `YYYY-MM-DD`.
    pub fn create(out: &Path, dump_date: &str) -> anyhow::Result<Self> {
        tracing::info!(out = %out.display(), dump_date, "creating AnimeTosho dataset");
        let dump_unix = parse_date(dump_date)?;
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::remove_file(out);
        let stage = out.with_extension("stage.tmp");
        let _ = std::fs::remove_file(&stage);
        let conn = Connection::open(out)?;
        conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA cache_size = -200000; PRAGMA temp_store = FILE;")?;
        conn.execute_batch(RELEASE_TABLES)?;
        conn.execute_batch(DATASET_TABLES)?;
        conn.execute("ATTACH DATABASE ?1 AS stage", params![stage.to_string_lossy()])?;
        conn.execute_batch(
            "PRAGMA stage.journal_mode = OFF; PRAGMA stage.synchronous = OFF;
             CREATE TABLE stage.files (toto_id INTEGER NOT NULL, filename TEXT NOT NULL, size INTEGER NOT NULL);
             CREATE TABLE stage.wanted (toto_id INTEGER PRIMARY KEY, nyaa_id INTEGER NOT NULL);",
        )?;
        Ok(Self { conn, out: out.to_path_buf(), stage, dump_date: dump_date.to_string(), dump_unix, stats: BuildStats::default(), stem_config: TrainConfig::default() })
    }

    /// Vote thresholds for the `stem_aid` table written by `finish`.
    pub fn set_stem_config(&mut self, config: TrainConfig) {
        self.stem_config = config;
    }

    /// Reads `toto_repl.sql`: `toto_files`, `toto_toto` and `toto_anidb_tvdb`
    /// (every other statement is skipped unparsed).
    pub fn read_toto<R: BufRead>(&mut self, source: R) -> anyhow::Result<()> {
        tracing::info!("reading toto_repl.sql");
        let mut reader = InsertReader::new(source);
        self.conn.execute_batch("BEGIN")?;
        while let Some(header) = reader.next_insert()? {
            match header.table.as_str() {
                "toto_toto" => self.read_toto_table(&mut reader, &header)?,
                "toto_files" => self.stage_files(&mut reader, &header)?,
                "toto_anidb_tvdb" => self.read_anidb_tvdb(&mut reader, &header)?,
                other => {
                    tracing::debug!(table = other, "skipping statement");
                    reader.skip_statement()?;
                }
            }
        }
        self.conn.execute_batch("COMMIT")?;
        tracing::info!(?self.stats, "toto_repl.sql read");
        Ok(())
    }

    fn read_toto_table<R: BufRead>(&mut self, reader: &mut InsertReader<R>, h: &Header) -> anyhow::Result<()> {
        let (id, nyaa_id, subdom, name, btih, size, dateline, files, nyaa_cat, deleted, isdupe, aid, eid, resolve) = (
            column(h, "id")?,
            column(h, "nyaa_id")?,
            column(h, "nyaa_subdom")?,
            column(h, "name")?,
            column(h, "btih")?,
            column(h, "totalsize")?,
            column(h, "dateline")?,
            column(h, "torrentfiles")?,
            column(h, "nyaa_cat")?,
            column(h, "deleted")?,
            column(h, "isdupe")?,
            column(h, "aid")?,
            column(h, "eid")?,
            column(h, "resolveapproved")?,
        );
        let conn = &self.conn;
        let mut release = conn.prepare(INSERT_RELEASE_IF_NEW)?;
        let mut aid_stmt = conn.prepare("INSERT OR REPLACE INTO release_aid (nyaa_id, aid, eid, approved) VALUES (?1, ?2, ?3, ?4)")?;
        let mut wanted = conn.prepare("INSERT OR REPLACE INTO stage.wanted (toto_id, nyaa_id) VALUES (?1, ?2)")?;
        while let Some(row) = reader.next_row()? {
            let nid = num(&row[nyaa_id]);
            if nid <= 0 || row[subdom].as_bytes().is_some_and(|b| !b.is_empty()) || num(&row[deleted]) != 0 {
                continue;
            }
            if let Some(hash) = row[btih].as_bytes().filter(|b| b.len() == 20) {
                let title = row[name].text();
                let category = nyaa_si_category(&row[nyaa_cat].text());
                let r = ReleaseRow { id: nid, category: &category, title: &title, hash, size: num(&row[size]).max(0) as u64, created: num(&row[dateline]) };
                if insert_release(&mut release, self.dump_unix, &r)? {
                    self.stats.releases += 1;
                }
            } else {
                tracing::debug!(nyaa_id = nid, "release without a 20-byte btih, no release row");
            }
            let aid_value = num(&row[aid]);
            if aid_value > 0 && num(&row[isdupe]) == 0 {
                aid_stmt.execute(params![nid, aid_value, num(&row[eid]), num(&row[resolve])])?;
                self.stats.release_aid += 1;
                if num(&row[files]) > 1 {
                    wanted.execute(params![num(&row[id]), nid])?;
                }
            }
        }
        Ok(())
    }

    fn stage_files<R: BufRead>(&mut self, reader: &mut InsertReader<R>, h: &Header) -> anyhow::Result<()> {
        let (toto_id, filename, filesize) = (column(h, "toto_id")?, column(h, "filename")?, column(h, "filesize")?);
        let mut stmt = self.conn.prepare("INSERT INTO stage.files (toto_id, filename, size) VALUES (?1, ?2, ?3)")?;
        while let Some(row) = reader.next_row()? {
            stmt.execute(params![num(&row[toto_id]), row[filename].text(), num(&row[filesize])])?;
        }
        Ok(())
    }

    fn read_anidb_tvdb<R: BufRead>(&mut self, reader: &mut InsertReader<R>, h: &Header) -> anyhow::Result<()> {
        let (anidb, tvdb, season, offset, mapping) =
            (column(h, "anidbid")?, column(h, "tvdbid")?, column(h, "defaulttvdbseason")?, column(h, "episodeoffset")?, column(h, "mapping-list")?);
        let mut stmt = self.conn.prepare("INSERT OR REPLACE INTO anidb_tvdb (anidb_id, tvdb_id, season, episode_offset, mapping) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        while let Some(row) = reader.next_row()? {
            let season_value = match &row[season] {
                Value::Null => None,
                // A non-numeric season (AniDB-lists' "a" = absolute order) has no integer form.
                other => other.as_i64().or_else(|| other.text().trim().parse().ok()),
            };
            let mapping_value = match &row[mapping] {
                Value::Null => None,
                other => Some(other.text()).filter(|m| !m.is_empty()),
            };
            stmt.execute(params![num(&row[anidb]), num(&row[tvdb]), season_value, num(&row[offset]), mapping_value])?;
            self.stats.anidb_tvdb += 1;
        }
        Ok(())
    }

    /// Reads `arcscrape.sql`: nyaa.si archive rows (`nyaasi_torrents`) not
    /// already present, anime (main category 1) only, skipping hidden,
    /// deleted and banned entries.
    pub fn read_archive<R: BufRead>(&mut self, source: R) -> anyhow::Result<()> {
        tracing::info!("reading arcscrape.sql");
        const SKIP_FLAGS: i64 = 2 | 32 | 64; // HIDDEN, DELETED, BANNED
        let mut reader = InsertReader::new(source);
        self.conn.execute_batch("BEGIN")?;
        while let Some(header) = reader.next_insert()? {
            if header.table != "nyaasi_torrents" {
                reader.skip_statement()?;
                continue;
            }
            let (id, hash, name, size, flags, created, main, sub) = (
                column(&header, "id")?,
                column(&header, "info_hash")?,
                column(&header, "display_name")?,
                column(&header, "filesize")?,
                column(&header, "flags")?,
                column(&header, "created_time")?,
                column(&header, "main_category_id")?,
                column(&header, "sub_category_id")?,
            );
            let mut release = self.conn.prepare(INSERT_RELEASE_IF_NEW)?;
            while let Some(row) = reader.next_row()? {
                let nid = num(&row[id]);
                if nid <= 0 || num(&row[main]) != 1 || num(&row[flags]) & SKIP_FLAGS != 0 {
                    continue;
                }
                let Some(hash) = row[hash].as_bytes().filter(|b| b.len() == 20) else { continue };
                let category = format!("{}_{}", num(&row[main]), num(&row[sub]));
                let title = row[name].text();
                let r = ReleaseRow { id: nid, category: &category, title: &title, hash, size: num(&row[size]).max(0) as u64, created: num(&row[created]) };
                if insert_release(&mut release, self.dump_unix, &r)? {
                    self.stats.releases += 1;
                    self.stats.archive_releases += 1;
                }
            }
        }
        self.conn.execute_batch("COMMIT")?;
        tracing::info!(?self.stats, "arcscrape.sql read");
        Ok(())
    }

    /// Joins the staged file lists, writes `meta`, removes the scratch file
    /// and leaves a single self-contained database.
    pub fn finish(mut self) -> anyhow::Result<BuildStats> {
        tracing::info!("joining file lists");
        self.conn.execute_batch("BEGIN; CREATE INDEX stage.files_toto ON files (toto_id);")?;
        self.conn.execute(
            "INSERT OR IGNORE INTO release_files (nyaa_id, idx, path, size)
             SELECT nyaa_id, n - 1, filename, size FROM (
                 SELECT w.nyaa_id AS nyaa_id, f.filename AS filename, f.size AS size,
                        ROW_NUMBER() OVER (PARTITION BY w.nyaa_id ORDER BY f.rowid) AS n
                 FROM stage.files f JOIN stage.wanted w ON w.toto_id = f.toto_id
             ) ORDER BY nyaa_id, n",
            [],
        )?;
        self.stats.release_files = self.conn.query_row("SELECT COUNT(*) FROM release_files", [], |r| r.get::<_, i64>(0))? as u64;
        self.stats.release_files_torrents = self.conn.query_row("SELECT COUNT(DISTINCT nyaa_id) FROM release_files", [], |r| r.get::<_, i64>(0))? as u64;
        self.stats.release_aid = self.conn.query_row("SELECT COUNT(*) FROM release_aid", [], |r| r.get::<_, i64>(0))? as u64;
        self.stats.anidb_tvdb = self.conn.query_row("SELECT COUNT(*) FROM anidb_tvdb", [], |r| r.get::<_, i64>(0))? as u64;
        self.stats.releases = self.conn.query_row("SELECT COUNT(*) FROM releases", [], |r| r.get::<_, i64>(0))? as u64;
        let built_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let meta: Vec<(&str, String)> = vec![
            ("kind", "tosho".into()),
            ("dump_date", self.dump_date.clone()),
            ("built_at", built_at.to_string()),
            ("releases", self.stats.releases.to_string()),
            ("archive_releases", self.stats.archive_releases.to_string()),
            ("release_aid", self.stats.release_aid.to_string()),
            ("release_files", self.stats.release_files.to_string()),
            ("release_files_torrents", self.stats.release_files_torrents.to_string()),
            ("anidb_tvdb", self.stats.anidb_tvdb.to_string()),
        ];
        for (key, value) in meta {
            self.conn.execute("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)", params![key, value])?;
        }
        self.conn.execute_batch("COMMIT; DETACH DATABASE stage;")?;
        self.stats.stem_aid = stem::train_dataset(&self.conn, self.stem_config)?;
        self.conn.execute("INSERT OR REPLACE INTO meta (key, value) VALUES ('stem_aid', ?1)", params![self.stats.stem_aid.to_string()])?;
        let Self { conn, out, stage, stats, .. } = self;
        drop(conn);
        if let Err(err) = std::fs::remove_file(&stage) {
            tracing::warn!(stage = %stage.display(), %err, "could not remove the scratch database");
        }
        tracing::info!(out = %out.display(), ?stats, "AnimeTosho dataset written");
        Ok(stats)
    }
}

/// Builds a dataset from `toto_repl.sql` alone.
pub fn build<R: BufRead>(toto: R, out: &Path, dump_date: &str) -> anyhow::Result<BuildStats> {
    let mut builder = ToshoBuilder::create(out, dump_date)?;
    builder.read_toto(toto)?;
    builder.finish()
}

/// `tosho-dataset-<YYYYMMDD>.db`.
pub fn dataset_file_name(dump_date: &str) -> String {
    format!("tosho-dataset-{}.db", dump_date.replace('-', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTO: &str = "-- dump
INSERT INTO `toto_files` (`id`, `toto_id`, `filename`, `filesize`) VALUES (1,10,'Show/ep 01.mkv',1000),(2,10,'Show/ep 02.mkv',2000),(3,11,'single.mkv',5),(4,12,'x.mkv',1),(5,12,'y.mkv',2);
INSERT INTO `toto_toto` (`id`, `nyaa_id`, `nyaa_subdom`, `name`, `btih`, `totalsize`, `dateline`, `torrentfiles`, `nyaa_cat`, `deleted`, `isdupe`, `aid`, `eid`, `resolveapproved`) VALUES (10,100,'','[G] Show - Batch (it''s)',0x0102030405060708090a0b0c0d0e0f1011121314,3000,1700000000,2,'1_2',0,0,55,0,1),(11,101,'','[G] Show - 03',0x0102030405060708090a0b0c0d0e0f1011121315,5,1700000100,1,'',0,0,55,9,0),(12,102,'','dupe',0x0102030405060708090a0b0c0d0e0f1011121316,3,1700000200,2,'1_2',0,1,55,9,1),(13,103,'sukebei','adult',0x0102030405060708090a0b0c0d0e0f1011121317,3,1700000200,1,'1_2',0,0,56,9,1),(14,104,'','gone',0x0102030405060708090a0b0c0d0e0f1011121318,3,1700000200,1,'1_2',1,0,56,9,1),(15,0,'','tosho only',0x0102030405060708090a0b0c0d0e0f1011121319,3,1700000200,1,'1_2',0,0,56,9,1),(16,105,'','no aid',0x0102030405060708090a0b0c0d0e0f101112131a,2048,1700000300,1,'1_4',0,0,0,0,0);
INSERT INTO `toto_anidb_tvdb` (`anidbid`, `tvdbid`, `defaulttvdbseason`, `episodeoffset`, `tmdbids`, `mapping-list`) VALUES (55,300,2,-12,'',';1-1;'),(56,0,NULL,0,'',NULL);
INSERT INTO `toto_trackers` (`id`) VALUES (1),(2);
";

    const ARCHIVE: &str = "INSERT INTO `nyaasi_torrents` (`id`, `info_hash`, `display_name`, `filesize`, `flags`, `created_time`, `main_category_id`, `sub_category_id`) VALUES (100,0x0102030405060708090a0b0c0d0e0f1011121314,'already there',1,0,1,1,2),(200,0x0102030405060708090a0b0c0d0e0f1011121400,'archive anime',1048576,0,1700000000,1,2),(201,0x0102030405060708090a0b0c0d0e0f1011121401,'archive music',1,0,1700000000,2,1),(202,0x0102030405060708090a0b0c0d0e0f1011121402,'archive deleted',1,32,1700000000,1,2);
";

    fn temp(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("tosho-build-test-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn builds_the_dataset_schema() {
        let out = temp("schema");
        let mut b = ToshoBuilder::create(&out, "2026-05-09").unwrap();
        b.read_toto(TOTO.as_bytes()).unwrap();
        b.read_archive(ARCHIVE.as_bytes()).unwrap();
        let stats = b.finish().unwrap();
        assert!(!out.with_extension("stage.tmp").exists());
        assert_eq!(
            stats,
            BuildStats { releases: 5, archive_releases: 1, release_aid: 2, release_files: 2, release_files_torrents: 1, anidb_tvdb: 2, stem_aid: 0 }
        );

        let db = Connection::open(&out).unwrap();
        let meta = |k: &str| db.query_row("SELECT value FROM meta WHERE key = ?1", [k], |r| r.get::<_, String>(0)).unwrap();
        assert_eq!((meta("kind").as_str(), meta("dump_date").as_str(), meta("release_aid").as_str()), ("tosho", "2026-05-09", "2"));

        // Releases: sukebei, deleted and nyaa_id 0 are out; the archive adds anime only.
        let ids: Vec<i64> = db.prepare("SELECT id FROM releases ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(ids, vec![100, 101, 102, 105, 200]);
        let (title, magnet, view, size, published, category, updated): (String, String, String, String, String, String, i64) = db
            .query_row("SELECT title, magnet, view_url, size, published, category, updated_at FROM releases WHERE id = 100", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })
            .unwrap();
        assert_eq!(title, "[G] Show - Batch (it's)");
        assert_eq!(magnet, "magnet:?xt=urn:btih:0102030405060708090a0b0c0d0e0f1011121314&dn=%5BG%5D%20Show%20-%20Batch%20%28it%27s%29");
        assert_eq!((view.as_str(), size.as_str(), published.as_str(), category.as_str()), ("https://nyaa.si/view/100", "2.9 KiB", "2023-11-14 22:13", "1_2"));
        assert_eq!(updated, parse_date("2026-05-09").unwrap());
        let cat = |id: i64| db.query_row("SELECT category FROM releases WHERE id = ?1", [id], |r| r.get::<_, String>(0)).unwrap();
        assert_eq!((cat(101), cat(105), cat(200)), ("1_2".into(), "1_4".into(), "1_2".into()));
        // The archive did not overwrite the toto row (id 100): title unchanged above.

        // release_aid: the dupe (102) and sukebei/deleted rows are out; approved is kept.
        let aids: Vec<(i64, i64, i64, i64)> =
            db.prepare("SELECT nyaa_id, aid, eid, approved FROM release_aid ORDER BY nyaa_id").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(Result::unwrap).collect();
        assert_eq!(aids, vec![(100, 55, 0, 1), (101, 55, 9, 0)]);

        // release_files: only the multi-file release, in listed order.
        let files: Vec<(i64, i64, String, i64)> = db
            .prepare("SELECT nyaa_id, idx, path, size FROM release_files ORDER BY nyaa_id, idx")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(files, vec![(100, 0, "Show/ep 01.mkv".into(), 1000), (100, 1, "Show/ep 02.mkv".into(), 2000)]);

        let tvdb: (i64, Option<i64>, i64, Option<String>) =
            db.query_row("SELECT tvdb_id, season, episode_offset, mapping FROM anidb_tvdb WHERE anidb_id = 55", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
        assert_eq!(tvdb, (300, Some(2), -12, Some(";1-1;".into())));
        let null_row: (Option<i64>, Option<String>) = db.query_row("SELECT season, mapping FROM anidb_tvdb WHERE anidb_id = 56", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(null_row, (None, None));
    }

    #[test]
    fn build_without_archive_and_helpers() {
        let out = temp("plain");
        let stats = build(TOTO.as_bytes(), &out, "2026-05-09").unwrap();
        assert_eq!((stats.releases, stats.archive_releases), (4, 0));
        assert_eq!(dataset_file_name("2026-05-09"), "tosho-dataset-20260509.db");
        assert_eq!((nyaa_si_category("1_37").as_str(), nyaa_si_category("").as_str(), nyaa_si_category("1_4").as_str()), ("1_2", "1_2", "1_4"));
        assert_eq!(format_size(512), "512 Bytes");
        assert_eq!(format_size(1536 * 1024 * 1024), "1.5 GiB");
        assert_eq!(format_published(0), "1970-01-01 00:00");
        assert!(parse_date("2026-13-01").is_err());
    }
}
