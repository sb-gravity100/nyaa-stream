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

use crate::NyaaResult;

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
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS releases (
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
             ) WITHOUT ROWID;",
        )?;
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
                let mut upsert = tx.prepare_cached(
                    "INSERT INTO releases (id, category, title, words, magnet, torrent_url, view_url, size, published, seeders, leechers, first_seen, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                     ON CONFLICT(id) DO UPDATE SET
                         title = excluded.title, words = excluded.words, magnet = excluded.magnet, torrent_url = excluded.torrent_url,
                         size = excluded.size, published = excluded.published, seeders = excluded.seeders,
                         leechers = excluded.leechers, updated_at = excluded.updated_at",
                )?;
                let mut hit = tx.prepare_cached("INSERT OR IGNORE INTO search_hits (key, release_id) VALUES (?1, ?2)")?;
                tx.execute("DELETE FROM search_hits WHERE key = ?1", params![key])?;
                for r in &results {
                    let Some(id) = release_id(&r.view_url) else { continue };
                    upsert.execute(params![
                        id, category, r.title, padded_words(&r.title), r.magnet, r.torrent_url, r.view_url, r.size, r.published,
                        r.seeders, r.leechers, stamp
                    ])?;
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
                "SELECT title, magnet, torrent_url, view_url, size, seeders, leechers, published FROM releases WHERE category = ?1",
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

fn row_to_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<NyaaResult> {
    Ok(NyaaResult {
        title: row.get(0)?,
        magnet: row.get(1)?,
        torrent_url: row.get(2)?,
        view_url: row.get(3)?,
        size: row.get(4)?,
        seeders: row.get(5)?,
        leechers: row.get(6)?,
        published: row.get(7)?,
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
        }
    }

    fn temp_db(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("nyaa-store-test-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn parses_release_ids() {
        assert_eq!(release_id("https://nyaa.si/view/2012345"), Some(2012345));
        assert_eq!(release_id("https://nyaa.si/view/99#comments"), Some(99));
        assert_eq!(release_id("https://nyaa.si/user/x"), None);
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
