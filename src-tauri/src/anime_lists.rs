//! TVDB season numbers per AniList entry, from the Fribb/anime-lists
//! dataset (`anime-list-full.json`, `anilist_id` -> `season.tvdb`).
//!
//! Groups number seasons by TVDB (Demon Slayer's Swordsmith Village is `S3`
//! on Crunchyroll but `S4` for Salieri), so `title_match::ShowMatcher` adds
//! this number to the season set read from the entry's titles. The dataset's
//! repo has no license, so it is fetched at runtime, never bundled: the
//! 7.5 MB file is reduced to a small `{anilist_id: season}` map, cached on
//! disk and refreshed weekly. Lookups are in-memory and work offline once a
//! copy exists.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use serde::Deserialize;

const DATASET_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
/// A cached copy older than this is refreshed.
const REFRESH_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Whole-download limit: the 7.5 MB file shares the connection with a torrent
/// that may be saturating it (a 90 s limit failed in practice).
const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
struct Entry {
    anilist_id: Option<i64>,
    season: Option<EntrySeason>,
}

#[derive(Deserialize)]
struct EntrySeason {
    tvdb: Option<i64>,
}

/// Reduces the dataset to `anilist_id` -> TVDB season. Entries without both
/// are skipped, as are negative seasons (the dataset's "no season" value);
/// the first entry for an id wins.
fn parse_dataset(bytes: &[u8]) -> Result<HashMap<i64, u32>, serde_json::Error> {
    let entries: Vec<Entry> = serde_json::from_slice(bytes)?;
    let mut seasons = HashMap::new();
    for entry in entries {
        let (Some(id), Some(season)) = (entry.anilist_id, entry.season.and_then(|s| s.tvdb)) else { continue };
        if let Ok(season) = u32::try_from(season) {
            seasons.entry(id).or_insert(season);
        }
    }
    Ok(seasons)
}

#[derive(Clone)]
pub struct AnimeLists {
    seasons: Arc<RwLock<HashMap<i64, u32>>>,
    path: PathBuf,
}

impl AnimeLists {
    /// Loads the cached map at `path`, if any; call `refresh_if_stale` to
    /// fetch or renew it.
    pub fn load(path: PathBuf) -> Self {
        let seasons = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<HashMap<i64, u32>>(&bytes).ok())
            .unwrap_or_default();
        tracing::info!(?path, entries = seasons.len(), "anime-lists cache loaded");
        AnimeLists { seasons: Arc::new(RwLock::new(seasons)), path }
    }

    /// The TVDB season of an AniList entry, if the dataset has one.
    pub fn tvdb_season(&self, anilist_id: i64) -> Option<u32> {
        let season = self.seasons.read().ok()?.get(&anilist_id).copied();
        tracing::trace!(anilist_id, ?season, "tvdb season lookup");
        season
    }

    fn is_stale(&self) -> bool {
        let age = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        age.is_none_or(|age| age >= REFRESH_AFTER)
    }

    /// Downloads the dataset when there is no cached copy or it is over a
    /// week old. A failure keeps whatever copy is already loaded.
    pub async fn refresh_if_stale(&self) {
        if !self.is_stale() {
            tracing::debug!("anime-lists cache is fresh");
            return;
        }
        match self.refresh().await {
            Ok(entries) => tracing::info!(entries, "anime-lists refreshed"),
            Err(err) => tracing::warn!(err = %format!("{err:#}"), "anime-lists refresh failed, keeping the cached copy"),
        }
    }

    async fn refresh(&self) -> anyhow::Result<usize> {
        tracing::debug!(url = DATASET_URL, "fetching anime-lists");
        let client = reqwest::Client::builder().connect_timeout(CONNECT_TIMEOUT).timeout(FETCH_TIMEOUT).user_agent("nyaa-stream").build()?;
        let started = std::time::Instant::now();
        let bytes = client.get(DATASET_URL).send().await?.error_for_status()?.bytes().await.map_err(|err| anyhow::anyhow!("reading the body ({:?} in): {err:?}", started.elapsed()))?;
        tracing::debug!(bytes = bytes.len(), elapsed = ?started.elapsed(), "anime-lists downloaded");
        let seasons = tokio::task::spawn_blocking(move || parse_dataset(&bytes)).await??;
        if seasons.is_empty() {
            anyhow::bail!("dataset parsed to zero entries");
        }
        if let Some(dir) = self.path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec(&seasons)?).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        let entries = seasons.len();
        if let Ok(mut current) = self.seasons.write() {
            *current = seasons;
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_reduces_to_anilist_id_and_tvdb_season() {
        let json = br#"[
            {"type":"TV","anilist_id":290,"tvdb_id":72025,"season":{"tvdb":1,"tmdb":1}},
            {"type":"OVA","anilist_id":1225,"season":{"tvdb":2}},
            {"type":"TV","anilist_id":1225,"season":{"tvdb":9}},
            {"type":"TV","anilist_id":5,"season":{"tmdb":1}},
            {"type":"TV","anilist_id":6,"season":{"tvdb":-1}},
            {"type":"TV","anilist_id":7},
            {"type":"TV","season":{"tvdb":3}}
        ]"#;
        let seasons = parse_dataset(json).unwrap();
        assert_eq!(seasons.get(&290), Some(&1));
        assert_eq!(seasons.get(&1225), Some(&2));
        assert_eq!(seasons.len(), 2);
    }

    #[test]
    fn missing_cache_is_empty_and_stale() {
        let lists = AnimeLists::load(std::env::temp_dir().join("nyaa-stream-no-such-anime-lists.json"));
        assert_eq!(lists.tvdb_season(290), None);
        assert!(lists.is_stale());
    }
}
