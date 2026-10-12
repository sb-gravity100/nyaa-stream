//! Permanent local database of every nyaa.si release the app has seen
//! (SQLite), keyed by nyaa.si's own release id - the number in
//! `/view/<id>` - so the same torrent reached through different queries,
//! pages or groups is one row.
//!
//! It answers three things before/instead of the network:
//! - a repeated query inside its freshness window is replayed from
//!   `search_hits` with no request at all;
//! - a stale repeat only pages until it reaches releases already stored
//!   (nyaa.si lists newest first), instead of walking every page again;
//! - `search_local` word-matches the stored titles the way nyaa.si's search
//!   does, for an instant first result and as the offline/rate-limited
//!   fallback for queries never run before.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::{NyaaResult, TorrentDetails};

/// nyaa.si's release id: `https://nyaa.si/view/2012345` -> 2012345.
pub fn release_id(view_url: &str) -> Option<i64> {
    let rest = view_url.split("/view/").nth(1)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Lowercase words with punctuation as separators, space-padded so a whole
/// word can be matched with `LIKE '% word %'`.
fn padded_words(text: &str) -> String {
    let words: Vec<String> = text
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    format!(" {} ", words.join(" "))
}

/// The `releases` columns an imported database must have.
const RELEASE_COLUMNS: &[&str] =
    &["id", "category", "title", "magnet", "torrent_url", "view_url", "size", "published", "seeders", "leechers", "first_seen", "updated_at"];

const PRAGMAS: &str = "PRAGMA journal_mode = WAL;
     PRAGMA synchronous = NORMAL;";

/// The tables of a release database (also the `releases` part of a AnimeTosho
/// dataset, see `tosho_build`).
pub(crate) const RELEASE_TABLES: &str = "CREATE TABLE IF NOT EXISTS releases (
         id INTEGER PRIMARY KEY,
         category TEXT NOT NULL,
         title TEXT NOT NULL,
         words TEXT NOT NULL,
         magnet TEXT NOT NULL,
         torrent_url TEXT NOT NULL,
         view_url TEXT NOT NULL,
         size TEXT NOT NULL,
         published TEXT NOT NULL,
         seeders INTEGER NOT NULL,
         leechers INTEGER NOT NULL,
         first_seen INTEGER NOT NULL,
         updated_at INTEGER NOT NULL
     );
     CREATE TABLE IF NOT EXISTS searches (
         key TEXT PRIMARY KEY,
         fetched_at INTEGER NOT NULL
     );
     CREATE TABLE IF NOT EXISTS search_hits (
         key TEXT NOT NULL,
         release_id INTEGER NOT NULL,
         PRIMARY KEY (key, release_id)
     ) WITHOUT ROWID;
     CREATE TABLE IF NOT EXISTS details (
         id INTEGER PRIMARY KEY,
         json TEXT NOT NULL,
         fetched_at INTEGER NOT NULL
     );";

/// The permanent AnimeTosho tables (see `tosho_build` for the dataset they
/// are copied from); replaced wholesale by `import_tosho`.
const TOSHO_TABLES: &str = "CREATE TABLE IF NOT EXISTS release_aid (
         nyaa_id INTEGER PRIMARY KEY,
         aid INTEGER NOT NULL,
         eid INTEGER NOT NULL,
         approved INTEGER NOT NULL
     );
     CREATE TABLE IF NOT EXISTS release_files (
         nyaa_id INTEGER NOT NULL,
         idx INTEGER NOT NULL,
         path TEXT NOT NULL,
         size INTEGER NOT NULL,
         PRIMARY KEY (nyaa_id, idx)
     ) WITHOUT ROWID;
     CREATE TABLE IF NOT EXISTS anidb_tvdb (
         anidb_id INTEGER PRIMARY KEY,
         tvdb_id INTEGER,
         season INTEGER,
         episode_offset INTEGER NOT NULL,
         mapping TEXT
     );
     CREATE TABLE IF NOT EXISTS stem_aid (
         stem TEXT PRIMARY KEY,
         aid INTEGER NOT NULL,
         votes INTEGER NOT NULL
     );
     CREATE TABLE IF NOT EXISTS tosho_meta (key TEXT PRIMARY KEY, value TEXT);";

/// Insert a release unless its id is already stored (dataset building). The
/// `words` column is left empty: `import` recomputes it.
pub(crate) const INSERT_RELEASE_IF_NEW: &str = "INSERT OR IGNORE INTO releases (id, category, title, words, magnet, torrent_url, view_url, size, published, seeders, leechers, first_seen, updated_at)
     VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7, ?8, 0, 0, ?9, ?9)";

const UPSERT: &str = "INSERT INTO releases (id, category, title, words, magnet, torrent_url, view_url, size, published, seeders, leechers, first_seen, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
     ON CONFLICT(id) DO UPDATE SET
         title = excluded.title, words = excluded.words, magnet = excluded.magnet, torrent_url = excluded.torrent_url,
         size = excluded.size, published = excluded.published, seeders = excluded.seeders,
         leechers = excluded.leechers, updated_at = excluded.updated_at";

/// Upserts one release; `None` when its view URL has no nyaa id.
fn upsert_release(stmt: &mut rusqlite::CachedStatement<'_>, category: &str, r: &NyaaResult, stamp: i64) -> rusqlite::Result<Option<i64>> {
    let Some(id) = release_id(&r.view_url) else { return Ok(None) };
    stmt.execute(params![id, category, r.title, padded_words(&r.title), r.magnet, r.torrent_url, r.view_url, r.size, r.published, r.seeders, r.leechers, stamp])?;
    Ok(Some(id))
}

/// What an import did.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct ImportStats {
    /// Releases in the imported file.
    pub total: u64,
    /// New to this database.
    pub added: u64,
    /// Already stored, replaced by a fresher imported row.
    pub updated: u64,
}

/// What an AnimeTosho dataset import did.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ToshoImportStats {
    /// The dataset's `releases`, merged like `Store::import`.
    pub releases: ImportStats,
    pub release_aid: u64,
    pub release_files: u64,
    pub anidb_tvdb: u64,
    /// Rows of the static stem mapper (0 for datasets built before it).
    pub stem_aid: u64,
    pub dump_date: String,
}

/// The imported AnimeTosho dataset (`Store::tosho_info`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ToshoInfo {
    /// Date of the AnimeTosho dump the dataset was built from, `YYYY-MM-DD`.
    pub dump_date: String,
    /// Unix seconds the dataset was imported.
    pub imported_at: i64,
    pub release_aid: u64,
    pub release_files: u64,
    pub anidb_tvdb: u64,
}

/// AnimeTosho's AniDB resolution of one nyaa.si release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ToshoRelease {
    /// AniDB anime id.
    pub aid: u32,
    /// AniDB episode id (0 = none/batch); releases of the same episode share it.
    pub eid: u32,
    /// `resolveapproved`: 0 = AnimeTosho's unconfirmed guess, 1/2/3 trusted.
    pub approved: u8,
}

/// AniDB -> TVDB mapping of one anime.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AnidbTvdb {
    pub tvdb_id: u32,
    /// TVDB season the entry maps to (`None` = no single season).
    pub season: Option<i32>,
    /// Added to AniDB episode numbers to get TVDB ones.
    pub episode_offset: i32,
    /// AnimeTosho's `mapping-list` (JSON text) for irregular mappings.
    pub mapping: Option<String>,
}

/// Writes `releases` (category code, release) into a release database at
/// `path` (created if missing) - for building importable datasets.
pub fn write_release_database(path: &Path, releases: &[(String, NyaaResult)]) -> anyhow::Result<usize> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch(PRAGMAS)?;
    conn.execute_batch(RELEASE_TABLES)?;
    let tx = conn.transaction()?;
    let stamp = now();
    let mut written = 0;
    {
        let mut upsert = tx.prepare_cached(UPSERT)?;
        for (category, r) in releases {
            if upsert_release(&mut upsert, category, r, stamp)?.is_some() {
                written += 1;
            }
        }
    }
    tx.commit()?;
    // A single self-contained file to hand around (no -wal/-shm sidecars).
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")?;
    tracing::info!(path = %path.display(), written, "release database written");
    Ok(written)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

/// A stored query result and how long ago it was fetched.
pub(crate) struct StoredSearch {
    pub results: Vec<NyaaResult>,
    pub age: Duration,
}

#[derive(Clone)]
pub(crate) struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub(crate) fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(PRAGMAS)?;
        conn.execute_batch(RELEASE_TABLES)?;
        conn.execute_batch(TOSHO_TABLES)?;
        tracing::info!(path = %path.display(), "nyaa release database opened");
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Runs `work` on a blocking thread with the connection.
    async fn run<T: Send + 'static>(&self, work: impl FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static) -> anyhow::Result<T> {
        let conn = self.conn.clone();
        Ok(tokio::task::spawn_blocking(move || work(&mut conn.lock().unwrap())).await??)
    }

    /// Stores `results` (upserting each release by id) and records them as
    /// the answer to query `key`.
    pub(crate) async fn save_search(&self, key: &str, category: &str, results: &[NyaaResult]) -> anyhow::Result<()> {
        let (key, category, results) = (key.to_string(), category.to_string(), results.to_vec());
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let stamp = now();
            {
                let mut upsert = tx.prepare_cached(UPSERT)?;
                let mut hit = tx.prepare_cached("INSERT OR IGNORE INTO search_hits (key, release_id) VALUES (?1, ?2)")?;
                tx.execute("DELETE FROM search_hits WHERE key = ?1", params![key])?;
                for r in &results {
                    let Some(id) = upsert_release(&mut upsert, &category, r, stamp)? else { continue };
                    hit.execute(params![key, id])?;
                }
            }
            tx.execute("INSERT INTO searches (key, fetched_at) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET fetched_at = excluded.fetched_at", params![key, stamp])?;
            tx.commit()
        })
        .await
    }

    /// The stored answer to query `key`, if it was ever fetched.
    pub(crate) async fn search(&self, key: &str) -> anyhow::Result<Option<StoredSearch>> {
        let key = key.to_string();
        self.run(move |conn| {
            let Some(fetched_at): Option<i64> = conn
                .query_row("SELECT fetched_at FROM searches WHERE key = ?1", params![key], |row| row.get(0))
                .map(Some)
                .or_else(|err| if matches!(err, rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(err) })?
            else {
                return Ok(None);
            };
            let mut stmt = conn.prepare_cached(
                "SELECT r.title, r.magnet, r.torrent_url, r.view_url, r.size, r.seeders, r.leechers, r.published
                 FROM search_hits h JOIN releases r ON r.id = h.release_id
                 WHERE h.key = ?1 ORDER BY r.id DESC",
            )?;
            let results = stmt.query_map(params![key], row_to_result)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(Some(StoredSearch { results, age: Duration::from_secs((now() - fetched_at).max(0) as u64) }))
        })
        .await
    }

    /// Stored releases in `category` whose titles contain every one of
    /// `words` as a whole word (nyaa.si's own AND matching), newest first.
    pub(crate) async fn search_local(&self, category: &str, query: &str, limit: usize) -> anyhow::Result<Vec<NyaaResult>> {
        let words: Vec<String> = padded_words(query).split_whitespace().map(str::to_string).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let category = category.to_string();
        self.run(move |conn| {
            let mut sql = String::from(
                // "1_0" is nyaa.si's "all anime": every 1_x subcategory, not an equal match.
                "SELECT title, magnet, torrent_url, view_url, size, seeders, leechers, published FROM releases WHERE (category = ?1 OR (?1 = '1_0' AND category IN ('1_1', '1_2', '1_3', '1_4')))",
            );
            for i in 0..words.len() {
                sql.push_str(&format!(" AND words LIKE ?{}", i + 2));
            }
            sql.push_str(" ORDER BY id DESC LIMIT ");
            sql.push_str(&limit.to_string());
            let mut values: Vec<String> = vec![category];
            values.extend(words.iter().map(|w| format!("% {w} %")));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), row_to_result)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .await
    }

    /// Imports another release database's `releases` (see PLAN.md
    /// "Importing a nyaa.si database"): upsert by nyaa id, a stored row wins
    /// unless the imported one is fresher, `words` recomputed. Query caches
    /// (`searches`, `search_hits`) aren't imported.
    pub(crate) async fn import(&self, path: &Path) -> anyhow::Result<ImportStats> {
        let path = path.to_path_buf();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<ImportStats> {
            let mut conn = conn.lock().unwrap();
            conn.execute("ATTACH DATABASE ?1 AS src", params![path.to_string_lossy()])?;
            let result = (|| -> anyhow::Result<ImportStats> {
                check_release_columns(&conn)?;
                let tx = conn.transaction()?;
                let stats = merge_releases(&tx)?;
                tx.commit()?;
                Ok(stats)
            })();
            let _ = conn.execute("DETACH DATABASE src", []);
            result
        })
        .await?
    }

    /// Imports an AnimeTosho dataset (`tosho_build`; PLAN.md "AnimeTosho
    /// dataset"): its `releases` merge like `import`, and `release_aid`,
    /// `release_files`, `anidb_tvdb` replace the stored ones wholesale, all in
    /// one transaction. Anything but a dataset (`meta.kind = 'tosho'`) is refused.
    pub(crate) async fn import_tosho(&self, path: &Path) -> anyhow::Result<ToshoImportStats> {
        let path = path.to_path_buf();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<ToshoImportStats> {
            let mut conn = conn.lock().unwrap();
            conn.execute("ATTACH DATABASE ?1 AS src", params![path.to_string_lossy()])?;
            let result = (|| -> anyhow::Result<ToshoImportStats> {
                let tables: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM src.sqlite_master WHERE type = 'table' AND name IN ('meta', 'release_aid', 'release_files', 'anidb_tvdb', 'releases')",
                    [],
                    |row| row.get(0),
                )?;
                if tables != 5 {
                    anyhow::bail!("not an AnimeTosho dataset (expected the tables meta, release_aid, release_files, anidb_tvdb and releases)");
                }
                let kind: Option<String> = conn.query_row("SELECT value FROM src.meta WHERE key = 'kind'", [], |row| row.get(0)).ok();
                if kind.as_deref() != Some("tosho") {
                    anyhow::bail!("not an AnimeTosho dataset (meta.kind is {:?}, expected \"tosho\")", kind.unwrap_or_default());
                }
                check_release_columns(&conn)?;
                let dump_date: String = conn.query_row("SELECT value FROM src.meta WHERE key = 'dump_date'", [], |row| row.get(0)).unwrap_or_default();
                let tx = conn.transaction()?;
                let releases = merge_releases(&tx)?;
                tx.execute_batch(
                    "DELETE FROM main.release_aid;
                     INSERT INTO main.release_aid (nyaa_id, aid, eid, approved) SELECT nyaa_id, aid, eid, approved FROM src.release_aid;
                     DELETE FROM main.release_files;
                     INSERT INTO main.release_files (nyaa_id, idx, path, size) SELECT nyaa_id, idx, path, size FROM src.release_files ORDER BY nyaa_id, idx;
                     DELETE FROM main.anidb_tvdb;
                     INSERT INTO main.anidb_tvdb (anidb_id, tvdb_id, season, episode_offset, mapping) SELECT anidb_id, tvdb_id, season, episode_offset, mapping FROM src.anidb_tvdb;
                     DELETE FROM main.stem_aid;",
                )?;
                // Older datasets have no stem table: they import, with no stems.
                let has_stems: i64 = tx.query_row("SELECT COUNT(*) FROM src.sqlite_master WHERE type = 'table' AND name = 'stem_aid'", [], |row| row.get(0))?;
                if has_stems > 0 {
                    tx.execute_batch("INSERT INTO main.stem_aid (stem, aid, votes) SELECT stem, aid, votes FROM src.stem_aid;")?;
                } else {
                    tracing::info!("dataset has no stem_aid table, stems cleared");
                }
                let count = |table: &str| -> rusqlite::Result<u64> { tx.query_row(&format!("SELECT COUNT(*) FROM main.{table}"), [], |row| row.get::<_, i64>(0)).map(|n| n.max(0) as u64) };
                let stats = ToshoImportStats { releases, release_aid: count("release_aid")?, release_files: count("release_files")?, anidb_tvdb: count("anidb_tvdb")?, stem_aid: count("stem_aid")?, dump_date };
                tx.execute_batch("DELETE FROM main.tosho_meta;")?;
                {
                    let mut set = tx.prepare("INSERT OR REPLACE INTO main.tosho_meta (key, value) VALUES (?1, ?2)")?;
                    for (key, value) in [
                        ("dump_date", stats.dump_date.clone()),
                        ("imported_at", now().to_string()),
                        ("release_aid", stats.release_aid.to_string()),
                        ("release_files", stats.release_files.to_string()),
                        ("anidb_tvdb", stats.anidb_tvdb.to_string()),
                        ("stem_aid", stats.stem_aid.to_string()),
                    ] {
                        set.execute(params![key, value])?;
                    }
                }
                tx.commit()?;
                Ok(stats)
            })();
            let _ = conn.execute("DETACH DATABASE src", []);
            result
        })
        .await?
    }

    /// The imported dataset's date and counts, `None` before any import.
    pub(crate) async fn tosho_info(&self) -> anyhow::Result<Option<ToshoInfo>> {
        self.run(|conn| {
            let mut stmt = conn.prepare_cached("SELECT key, value FROM tosho_meta")?;
            let rows: Vec<(String, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            let get = |key: &str| rows.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
            let Some(dump_date) = get("dump_date") else { return Ok(None) };
            let number = |key: &str| get(key).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            Ok(Some(ToshoInfo {
                dump_date: dump_date.to_string(),
                imported_at: get("imported_at").and_then(|v| v.parse().ok()).unwrap_or(0),
                release_aid: number("release_aid"),
                release_files: number("release_files"),
                anidb_tvdb: number("anidb_tvdb"),
            }))
        })
        .await
    }

    /// AnimeTosho's AniDB resolution of nyaa.si release `nyaa_id`, if the
    /// imported dataset knows it.
    pub(crate) async fn aid_of(&self, nyaa_id: i64) -> anyhow::Result<Option<ToshoRelease>> {
        self.run(move |conn| match conn.prepare_cached("SELECT aid, eid, approved FROM release_aid WHERE nyaa_id = ?1")?.query_row(params![nyaa_id], row_to_tosho_release) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        })
        .await
    }

    /// `aid_of` for many releases at once (ids the dataset doesn't know are absent).
    pub(crate) async fn aids_of(&self, nyaa_ids: &[i64]) -> anyhow::Result<std::collections::HashMap<i64, ToshoRelease>> {
        let ids = nyaa_ids.to_vec();
        self.run(move |conn| {
            let mut stmt = conn.prepare_cached("SELECT aid, eid, approved FROM release_aid WHERE nyaa_id = ?1")?;
            let mut found = std::collections::HashMap::new();
            for id in ids {
                match stmt.query_row(params![id], row_to_tosho_release) {
                    Ok(r) => {
                        found.insert(id, r);
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(err) => return Err(err),
                }
            }
            Ok(found)
        })
        .await
    }

    /// The AniDB id the static stem mapper gives `stem` (`stem::stem_of`), if
    /// the imported dataset's `stem_aid` holds it.
    pub(crate) async fn aid_for_stem(&self, stem: &str) -> anyhow::Result<Option<u32>> {
        let stem = stem.to_string();
        self.run(move |conn| match conn.prepare_cached("SELECT aid FROM stem_aid WHERE stem = ?1")?.query_row(params![stem], |row| row.get::<_, i64>(0)) {
            Ok(aid) => Ok(Some(aid.max(0) as u32)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        })
        .await
    }

    /// `aid_for_stem` for many stems (unknown stems are absent from the map).
    pub(crate) async fn aids_for_stems(&self, stems: &[String]) -> anyhow::Result<std::collections::HashMap<String, u32>> {
        let stems = stems.to_vec();
        self.run(move |conn| {
            let mut stmt = conn.prepare_cached("SELECT aid FROM stem_aid WHERE stem = ?1")?;
            let mut found = std::collections::HashMap::new();
            for stem in stems {
                match stmt.query_row(params![stem], |row| row.get::<_, i64>(0)) {
                    Ok(aid) => {
                        found.insert(stem, aid.max(0) as u32);
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(err) => return Err(err),
                }
            }
            Ok(found)
        })
        .await
    }

    /// The file list of release `nyaa_id` from the dataset (multi-file
    /// releases only; empty when unknown), with per-video season/episode.
    pub(crate) async fn release_files(&self, nyaa_id: i64) -> anyhow::Result<Vec<crate::TorrentFile>> {
        let raw: Vec<(String, u64)> = self
            .run(move |conn| {
                let mut stmt = conn.prepare_cached("SELECT path, size FROM release_files WHERE nyaa_id = ?1 ORDER BY idx")?;
                let rows = stmt.query_map(params![nyaa_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?.max(0) as u64)))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .await?;
        Ok(crate::files::build_files(raw).0)
    }

    /// The AniDB -> TVDB mapping of anime `aid`, if the dataset has one.
    pub(crate) async fn anidb_tvdb(&self, aid: u32) -> anyhow::Result<Option<AnidbTvdb>> {
        self.run(move |conn| {
            let row = conn.prepare_cached("SELECT tvdb_id, season, episode_offset, mapping FROM anidb_tvdb WHERE anidb_id = ?1")?.query_row(params![aid], |row| {
                Ok(AnidbTvdb {
                    tvdb_id: row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u32,
                    season: row.get::<_, Option<i64>>(1)?.map(|n| n as i32),
                    episode_offset: row.get::<_, i64>(2)? as i32,
                    mapping: row.get(3)?,
                })
            });
            match row {
                Ok(r) => Ok(Some(r)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(err) => Err(err),
            }
        })
        .await
    }

    /// The scraped view page of release `id`, if it was ever fetched. A row
    /// that no longer deserializes is treated as missing (refetched).
    pub(crate) async fn details(&self, id: i64) -> anyhow::Result<Option<TorrentDetails>> {
        let json: Option<String> = self
            .run(move |conn| match conn.query_row("SELECT json FROM details WHERE id = ?1", params![id], |row| row.get(0)) {
                Ok(json) => Ok(Some(json)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(err) => Err(err),
            })
            .await?;
        Ok(json.and_then(|json| match serde_json::from_str(&json) {
            Ok(details) => Some(details),
            Err(err) => {
                tracing::warn!(id, %err, "unreadable stored torrent details, ignoring");
                None
            }
        }))
    }

    /// Stores the scraped view page of release `id` (replacing any earlier one).
    pub(crate) async fn save_details(&self, id: i64, details: &TorrentDetails) -> anyhow::Result<()> {
        let json = serde_json::to_string(details)?;
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO details (id, json, fetched_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json, fetched_at = excluded.fetched_at",
                params![id, json, now()],
            )
            .map(|_| ())
        })
        .await
    }

    /// Ids among `ids` already stored.
    pub(crate) async fn known_ids(&self, ids: &[i64]) -> anyhow::Result<HashSet<i64>> {
        let ids = ids.to_vec();
        self.run(move |conn| {
            let mut stmt = conn.prepare_cached("SELECT 1 FROM releases WHERE id = ?1")?;
            let mut known = HashSet::new();
            for id in ids {
                if stmt.exists(params![id])? {
                    known.insert(id);
                }
            }
            Ok(known)
        })
        .await
    }
}

fn row_to_tosho_release(row: &rusqlite::Row<'_>) -> rusqlite::Result<ToshoRelease> {
    Ok(ToshoRelease { aid: row.get::<_, i64>(0)?.max(0) as u32, eid: row.get::<_, i64>(1)?.max(0) as u32, approved: row.get::<_, i64>(2)?.clamp(0, 255) as u8 })
}

/// Errors unless the attached `src` database has a `releases` table with
/// every column an import needs.
fn check_release_columns(conn: &Connection) -> anyhow::Result<()> {
    let columns: Vec<String> = conn.prepare("SELECT name FROM pragma_table_info('releases', 'src')")?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
    let missing: Vec<&str> = RELEASE_COLUMNS.iter().copied().filter(|c| !columns.iter().any(|have| have == c)).collect();
    if !missing.is_empty() {
        anyhow::bail!("not a nyaa-stream release database (no `releases` table with {})", missing.join(", "));
    }
    Ok(())
}

/// Merges `src.releases` into `main.releases` inside `tx`: upsert by nyaa id,
/// a stored row wins unless the imported one is fresher, `words` recomputed.
fn merge_releases(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<ImportStats> {
    let total: i64 = tx.query_row("SELECT COUNT(*) FROM src.releases", [], |row| row.get(0))?;
    let before: i64 = tx.query_row("SELECT COUNT(*) FROM main.releases", [], |row| row.get(0))?;
    let changed = tx.execute(
        "INSERT INTO main.releases (id, category, title, words, magnet, torrent_url, view_url, size, published, seeders, leechers, first_seen, updated_at)
         SELECT id, category, title, '', magnet, torrent_url, view_url, size, published, seeders, leechers, first_seen, updated_at
         FROM src.releases WHERE id IS NOT NULL
         ON CONFLICT(id) DO UPDATE SET
             title = excluded.title, words = '', magnet = excluded.magnet, torrent_url = excluded.torrent_url,
             size = excluded.size, published = excluded.published, seeders = excluded.seeders,
             leechers = excluded.leechers, updated_at = excluded.updated_at
         WHERE excluded.updated_at > main.releases.updated_at",
        [],
    )? as i64;
    let after: i64 = tx.query_row("SELECT COUNT(*) FROM main.releases", [], |row| row.get(0))?;
    {
        let titles: Vec<(i64, String)> = tx.prepare("SELECT id, title FROM main.releases WHERE words = ''")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut set = tx.prepare("UPDATE main.releases SET words = ?2 WHERE id = ?1")?;
        for (id, title) in titles {
            set.execute(params![id, padded_words(&title)])?;
        }
    }
    let added = (after - before).max(0);
    Ok(ImportStats { total: total.max(0) as u64, added: added as u64, updated: (changed - added).max(0) as u64 })
}

fn row_to_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<NyaaResult> {
    let title: String = row.get(0)?;
    Ok(NyaaResult {
        label: crate::EpisodeLabel::of(&title),
        title,
        magnet: row.get(1)?,
        torrent_url: row.get(2)?,
        view_url: row.get(3)?,
        size: row.get(4)?,
        seeders: row.get(5)?,
        leechers: row.get(6)?,
        published: row.get(7)?,
        episode_group: None,
        aid_trusted: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(id: i64, title: &str, seeders: u32) -> NyaaResult {
        NyaaResult {
            title: title.to_string(),
            magnet: format!("magnet:?xt={id}"),
            torrent_url: format!("https://nyaa.si/download/{id}.torrent"),
            view_url: format!("https://nyaa.si/view/{id}"),
            size: "1 GiB".into(),
            seeders,
            leechers: 0,
            published: "2026-01-01".into(),
            label: Default::default(),
            episode_group: None,
            aid_trusted: None,
        }
    }

    #[tokio::test]
    async fn imports_another_database() {
        let src = temp_db("import-src");
        let _ = std::fs::remove_file(&src);
        write_release_database(&src, &[("1_2".into(), release(1, "[SubsPlease] Frieren - 01 (1080p)", 50)), ("1_2".into(), release(2, "[SubsPlease] Frieren - 02 (1080p)", 40))]).unwrap();
        let store = Store::open(&temp_db("import-dst")).unwrap();
        store.save_search("frieren", "1_2", &[release(2, "[SubsPlease] Frieren - 02 (1080p)", 99)]).await.unwrap();
        let stats = store.import(&src).await.unwrap();
        assert_eq!((stats.total, stats.added), (2, 1));
        let found = store.search_local("1_2", "frieren 01", 10).await.unwrap();
        assert_eq!(found.len(), 1);
        // Not a release database
        let bad = temp_db("import-bad");
        let _ = std::fs::remove_file(&bad);
        rusqlite::Connection::open(&bad).unwrap().execute_batch("CREATE TABLE other (x)").unwrap();
        assert!(store.import(&bad).await.is_err());
    }

    const TOSHO_SQL: &str = "INSERT INTO `toto_files` (`id`, `toto_id`, `filename`, `filesize`) VALUES (1,10,'S/Show - 01.mkv',1000),(2,10,'S/Show - 02.mkv',2000);
INSERT INTO `toto_toto` (`id`, `nyaa_id`, `nyaa_subdom`, `name`, `btih`, `totalsize`, `dateline`, `torrentfiles`, `nyaa_cat`, `deleted`, `isdupe`, `aid`, `eid`, `resolveapproved`) VALUES (10,100,'','[G] Show - Batch',0x0102030405060708090a0b0c0d0e0f1011121314,3000,1700000000,2,'1_37',0,0,55,7,3),(11,101,'','[G] Show - 03',0x0102030405060708090a0b0c0d0e0f1011121315,5,1700000100,1,'',0,0,55,9,0);
INSERT INTO `toto_anidb_tvdb` (`anidbid`, `tvdbid`, `defaulttvdbseason`, `episodeoffset`, `mapping-list`) VALUES (55,300,2,-12,'[]');
";

    #[tokio::test]
    async fn imports_a_tosho_dataset() {
        let src = temp_db("tosho-src");
        crate::tosho_build::build(TOSHO_SQL.as_bytes(), &src, "2026-05-09").unwrap();
        let store = Store::open(&temp_db("tosho-dst")).unwrap();
        assert!(store.tosho_info().await.unwrap().is_none());
        // A stored scraped row (fresher than the dump) wins over the dataset's.
        store.save_search("q", "1_2", &[release(101, "[G] Show - 03 scraped", 40)]).await.unwrap();
        let stats = store.import_tosho(&src).await.unwrap();
        assert_eq!((stats.releases.total, stats.releases.added, stats.release_aid, stats.release_files, stats.anidb_tvdb), (2, 1, 2, 2, 1));
        assert_eq!(stats.dump_date, "2026-05-09");
        assert_eq!(stats.stem_aid, 0);
        assert_eq!(store.aid_for_stem("show").await.unwrap(), None);
        assert_eq!(store.aid_of(100).await.unwrap(), Some(ToshoRelease { aid: 55, eid: 7, approved: 3 }));
        assert_eq!(store.aid_of(999).await.unwrap(), None);
        assert_eq!(store.aids_of(&[100, 101, 999]).await.unwrap().len(), 2);
        let files = store.release_files(100).await.unwrap();
        assert_eq!(
            files.iter().map(|f| (f.path.as_str(), f.size, f.index, f.episode)).collect::<Vec<_>>(),
            vec![("S/Show - 01.mkv", 1000, 0, Some(1)), ("S/Show - 02.mkv", 2000, 1, Some(2))]
        );
        assert!(store.release_files(101).await.unwrap().is_empty());
        assert_eq!(store.anidb_tvdb(55).await.unwrap(), Some(AnidbTvdb { tvdb_id: 300, season: Some(2), episode_offset: -12, mapping: Some("[]".into()) }));
        assert!(store.anidb_tvdb(1).await.unwrap().is_none());
        let info = store.tosho_info().await.unwrap().unwrap();
        assert_eq!((info.dump_date.as_str(), info.release_aid, info.release_files, info.anidb_tvdb), ("2026-05-09", 2, 2, 1));
        // The dataset's release merged in under the app's category code; the stored one kept its title.
        let found = store.search_local("1_2", "show", 10).await.unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|r| r.title == "[G] Show - 03 scraped"));
        // Re-importing replaces rather than appends.
        let again = store.import_tosho(&src).await.unwrap();
        assert_eq!((again.release_aid, again.release_files, again.releases.added), (2, 2, 0));
        // A plain release database or an unrelated file is refused.
        let plain = temp_db("tosho-plain");
        write_release_database(&plain, &[("1_2".into(), release(1, "x", 1))]).unwrap();
        assert!(store.import_tosho(&plain).await.unwrap_err().to_string().contains("AnimeTosho"));
        let bad = temp_db("tosho-bad");
        rusqlite::Connection::open(&bad).unwrap().execute_batch("CREATE TABLE meta (key TEXT, value TEXT); CREATE TABLE other (x)").unwrap();
        assert!(store.import_tosho(&bad).await.is_err());
    }

    #[tokio::test]
    async fn imports_stems_and_clears_them_for_older_datasets() {
        let src = temp_db("stem-src");
        crate::tosho_build::build(TOSHO_SQL.as_bytes(), &src, "2026-05-09").unwrap();
        rusqlite::Connection::open(&src).unwrap().execute_batch("INSERT INTO stem_aid VALUES ('frieren', 17617, 40), ('one piece', 69, 900);").unwrap();
        let store = Store::open(&temp_db("stem-dst")).unwrap();
        let stats = store.import_tosho(&src).await.unwrap();
        assert_eq!(stats.stem_aid, 2);
        assert_eq!(store.aid_for_stem("frieren").await.unwrap(), Some(17617));
        assert_eq!(store.aid_for_stem("unknown show").await.unwrap(), None);
        let found = store.aids_for_stems(&["one piece".into(), "frieren".into(), "nope".into()]).await.unwrap();
        assert_eq!((found.len(), found["one piece"], found["frieren"]), (2, 69, 17617));
        // Re-importing replaces: a dataset without the table (built before T9) clears the stems.
        rusqlite::Connection::open(&src).unwrap().execute_batch("DROP TABLE stem_aid;").unwrap();
        let old = store.import_tosho(&src).await.unwrap();
        assert_eq!(old.stem_aid, 0);
        assert!(store.aids_for_stems(&["frieren".into()]).await.unwrap().is_empty());
    }

    fn temp_db(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("nyaa-store-test-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[tokio::test]
    async fn stores_details_by_release_id() {
        let store = Store::open(&temp_db("details")).unwrap();
        assert!(store.details(7).await.unwrap().is_none());
        let details = TorrentDetails {
            submitter: "Reza".into(),
            is_batch: true,
            file_count: 1,
            files: vec![crate::TorrentFile { path: "Season 2/a S02E01.mkv".into(), size: 5, index: 0, season: Some(2), episode: Some(1) }],
            seasons: vec![2],
        };
        store.save_details(7, &details).await.unwrap();
        let back = store.details(7).await.unwrap().unwrap();
        assert_eq!((back.submitter.as_str(), back.seasons.clone(), back.files.len()), ("Reza", vec![2], 1));
        assert_eq!(back.files[0].season, Some(2));
        assert!(store.details(8).await.unwrap().is_none());
    }

    #[test]
    fn old_details_json_still_deserializes() {
        let old: TorrentDetails = serde_json::from_str(r#"{"submitter":"x","is_batch":false,"file_count":1}"#).unwrap();
        assert!(old.files.is_empty() && old.seasons.is_empty());
    }

    #[test]
    fn parses_release_ids() {
        assert_eq!(release_id("https://nyaa.si/view/2012345"), Some(2012345));
        assert_eq!(release_id("https://nyaa.si/view/99#comments"), Some(99));
        assert_eq!(release_id("https://nyaa.si/user/x"), None);
    }

    #[tokio::test]
    async fn all_anime_category_covers_every_anime_subcategory() {
        let store = Store::open(&temp_db("all-anime")).unwrap();
        store.save_search("1_2|k", "1_2", &[release(1, "[A] Kaiju - 01", 1)]).await.unwrap();
        store.save_search("1_4|k", "1_4", &[release(2, "[B] Kaiju - 01 raw", 1)]).await.unwrap();
        store.save_search("1_3|k", "1_3", &[release(3, "[C] Kaiju - 01 non-english", 1)]).await.unwrap();
        store.save_search("3_1|k", "3_1", &[release(4, "[D] Kaiju - 01 manga", 1)]).await.unwrap();
        assert_eq!(store.search_local("1_0", "kaiju", 50).await.unwrap().len(), 3, "1_0 = all anime subcategories, not manga");
        assert_eq!(store.search_local("1_2", "kaiju", 50).await.unwrap().len(), 1, "a concrete category still matches by equality");
    }

    #[tokio::test]
    async fn dedupes_by_id_and_answers_local_searches() {
        let store = Store::open(&temp_db("dedupe")).unwrap();
        store.save_search("1_2|fate", "1_2", &[release(2, "[A] Fate strange Fake - 02", 5), release(1, "[A] Fate strange Fake - 01", 3)]).await.unwrap();
        // A second query returns release 2 again, with new seeders, plus another show.
        store.save_search("1_2|fake", "1_2", &[release(2, "[A] Fate strange Fake - 02", 9), release(3, "[B] Other Fake Show - 01", 1)]).await.unwrap();

        let fate = store.search_local("1_2", "Fate/strange Fake", 50).await.unwrap();
        assert_eq!(fate.len(), 2, "release 2 is stored once");
        assert_eq!(fate[0].seeders, 9, "and carries its latest seeders");
        assert_eq!(store.search_local("1_2", "fake", 50).await.unwrap().len(), 3);
        assert_eq!(store.search_local("1_2", "fak", 50).await.unwrap().len(), 0, "whole words only");

        let stored = store.search("1_2|fate").await.unwrap().unwrap();
        assert_eq!(stored.results.len(), 2);
        assert!(store.search("1_2|missing").await.unwrap().is_none());
        assert_eq!(store.known_ids(&[1, 2, 3, 4]).await.unwrap().len(), 3);
    }
}
