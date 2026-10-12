//! TVDB season numbers and AniDB ids per AniList entry, from the
//! Fribb/anime-lists dataset (`anime-list-full.json`: `anilist_id` ->
//! `season.tvdb` and `anilist_id` -> `anidb_id`).
//!
//! Groups number seasons by TVDB (Demon Slayer's Swordsmith Village is `S3`
//! on Crunchyroll but `S4` for Salieri), so `title_match::ShowMatcher` adds
//! this number to the season set read from the entry's titles. The AniDB ids
//! feed the AnimeTosho dataset join, which keys on AniDB. The dataset's
//! repo has no license, so it is fetched at runtime, never bundled: the
//! 7.5 MB file is reduced to small `{anilist_id: season}` and
//! `{anilist_id: [anidb_id]}` maps, cached on disk and refreshed weekly.
//! Lookups are in-memory and work offline once a copy exists.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize};

const DATASET_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
/// A cached copy older than this is refreshed.
const REFRESH_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Whole-download limit: the 7.5 MB file shares the connection with a torrent
/// that may be saturating it (a 90 s limit failed in practice).
const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Version of the cached reduced file. v1 was a bare `{anilist_id: season}`
/// map; v2 adds the AniList -> AniDB id sets.
const CACHE_VERSION: u32 = 2;

/// `anidb_id` is a number in the dataset today; a list is accepted too.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(i64),
    Many(Vec<i64>),
}

fn lenient_aids<'de, D: Deserializer<'de>>(d: D) -> Result<Option<OneOrMany>, D::Error> {
    // A malformed value must not sink the whole 40k-entry file.
    Ok(Option::<serde_json::Value>::deserialize(d)?.and_then(|v| serde_json::from_value(v).ok()))
}

#[derive(Deserialize)]
struct Entry {
    anilist_id: Option<i64>,
    #[serde(default, deserialize_with = "lenient_aids")]
    anidb_id: Option<OneOrMany>,
    season: Option<EntrySeason>,
}

#[derive(Deserialize)]
struct EntrySeason {
    tvdb: Option<i64>,
}

/// The reduced dataset, as cached on disk.
#[derive(Default, Serialize, Deserialize)]
struct Reduced {
    version: u32,
    /// `anilist_id` -> TVDB season.
    seasons: HashMap<i64, u32>,
    /// `anilist_id` -> sorted, deduped AniDB ids.
    anidb: HashMap<u32, Vec<u32>>,
}

/// Reduces the dataset to `anilist_id` -> TVDB season and `anilist_id` ->
/// AniDB ids. Season: entries without both ids are skipped, as are negative
/// seasons (the dataset's "no season" value); the first entry for an id wins.
/// AniDB: every entry's id is collected, since AniList -> AniDB is one-to-many.
fn parse_dataset(bytes: &[u8]) -> Result<Reduced, serde_json::Error> {
    let entries: Vec<Entry> = serde_json::from_slice(bytes)?;
    let mut out = Reduced { version: CACHE_VERSION, ..Default::default() };
    for entry in entries {
        let Some(id) = entry.anilist_id else { continue };
        if let Some(season) = entry.season.and_then(|s| s.tvdb).and_then(|s| u32::try_from(s).ok()) {
            out.seasons.entry(id).or_insert(season);
        }
        let aids = match entry.anidb_id {
            Some(OneOrMany::One(a)) => vec![a],
            Some(OneOrMany::Many(v)) => v,
            None => continue,
        };
        if let Ok(anilist) = u32::try_from(id) {
            let slot = out.anidb.entry(anilist).or_default();
            slot.extend(aids.into_iter().filter_map(|a| u32::try_from(a).ok().filter(|a| *a > 0)));
        }
    }
    for aids in out.anidb.values_mut() {
        aids.sort_unstable();
        aids.dedup();
    }
    out.anidb.retain(|_, v| !v.is_empty());
    Ok(out)
}

/// The in-memory form: the reduced data plus the reverse AniDB index.
#[derive(Default)]
struct Data {
    seasons: HashMap<i64, u32>,
    anidb: HashMap<u32, Vec<u32>>,
    by_anidb: HashMap<u32, Vec<u32>>,
}

impl Data {
    fn from_reduced(r: Reduced) -> Self {
        let mut by_anidb: HashMap<u32, Vec<u32>> = HashMap::new();
        for (anilist, aids) in &r.anidb {
            for aid in aids {
                by_anidb.entry(*aid).or_default().push(*anilist);
            }
        }
        for v in by_anidb.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        Data { seasons: r.seasons, anidb: r.anidb, by_anidb }
    }
}

/// Reads a cached file. `None` for missing, unreadable or pre-v2 content,
/// which all mean "refresh".
fn read_cache(path: &std::path::Path) -> Option<Reduced> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice::<Reduced>(&bytes) {
        Ok(r) if r.version == CACHE_VERSION => Some(r),
        Ok(r) => {
            tracing::info!(version = r.version, "anime-lists cache has another format version, refreshing");
            None
        }
        Err(err) => {
            tracing::info!(%err, "anime-lists cache is in the old or an unreadable format, refreshing");
            None
        }
    }
}

#[derive(Clone)]
pub struct AnimeLists {
    data: Arc<RwLock<Data>>,
    path: PathBuf,
    /// The cached file was missing or in an old format: refresh regardless
    /// of its age.
    format_stale: bool,
}

impl AnimeLists {
    /// Loads the cached map at `path`, if any; call `refresh_if_stale` to
    /// fetch or renew it. A cache without the AniDB field (the old format)
    /// loads empty and is refreshed on the next `refresh_if_stale`.
    pub fn load(path: PathBuf) -> Self {
        let cached = read_cache(&path);
        let format_stale = cached.is_none();
        let data = cached.map(Data::from_reduced).unwrap_or_default();
        tracing::info!(?path, seasons = data.seasons.len(), anilist_with_anidb = data.anidb.len(), format_stale, "anime-lists cache loaded");
        AnimeLists { data: Arc::new(RwLock::new(data)), path, format_stale }
    }

    /// The TVDB season of an AniList entry, if the dataset has one.
    pub fn tvdb_season(&self, anilist_id: i64) -> Option<u32> {
        let season = self.data.read().ok()?.seasons.get(&anilist_id).copied();
        tracing::trace!(anilist_id, ?season, "tvdb season lookup");
        season
    }

    /// The AniDB ids of an AniList entry, sorted and deduped; empty when
    /// unknown (AniList -> AniDB is one-to-many).
    pub fn anidb_ids(&self, anilist_id: u32) -> Vec<u32> {
        let ids = self.data.read().ok().and_then(|d| d.anidb.get(&anilist_id).cloned()).unwrap_or_default();
        tracing::trace!(anilist_id, ?ids, "anidb ids lookup");
        ids
    }

    /// The AniList entries that map to an AniDB id, sorted and deduped; empty
    /// when unknown.
    pub fn anilist_ids_for_anidb(&self, aid: u32) -> Vec<u32> {
        let ids = self.data.read().ok().and_then(|d| d.by_anidb.get(&aid).cloned()).unwrap_or_default();
        tracing::trace!(aid, ?ids, "anilist ids lookup");
        ids
    }

    fn is_stale(&self) -> bool {
        if self.format_stale {
            return true;
        }
        let age = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        age.is_none_or(|age| age >= REFRESH_AFTER)
    }

    /// Downloads the dataset when there is no cached copy, it is in an old
    /// format, or it is over a week old. A failure keeps whatever copy is
    /// already loaded.
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
        let reduced = tokio::task::spawn_blocking(move || parse_dataset(&bytes)).await??;
        if reduced.seasons.is_empty() {
            anyhow::bail!("dataset parsed to zero entries");
        }
        if let Some(dir) = self.path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec(&reduced)?).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        let entries = reduced.seasons.len();
        tracing::info!(seasons = entries, anilist_with_anidb = reduced.anidb.len(), "anime-lists reduced and cached");
        if let Ok(mut current) = self.data.write() {
            *current = Data::from_reduced(reduced);
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
            {"type":"TV","anilist_id":290,"anidb_id":1,"tvdb_id":72025,"season":{"tvdb":1,"tmdb":1}},
            {"type":"OVA","anilist_id":1225,"season":{"tvdb":2}},
            {"type":"TV","anilist_id":1225,"season":{"tvdb":9}},
            {"type":"TV","anilist_id":5,"season":{"tmdb":1}},
            {"type":"TV","anilist_id":6,"season":{"tvdb":-1}},
            {"type":"TV","anilist_id":7},
            {"type":"TV","season":{"tvdb":3}}
        ]"#;
        let r = parse_dataset(json).unwrap();
        assert_eq!(r.seasons.get(&290), Some(&1));
        assert_eq!(r.seasons.get(&1225), Some(&2));
        assert_eq!(r.seasons.len(), 2);
    }

    const SAMPLE: &str = r#"[
        {"anilist_id":100,"anidb_id":30,"season":{"tvdb":1}},
        {"anilist_id":100,"anidb_id":10},
        {"anilist_id":100,"anidb_id":30},
        {"anilist_id":200,"anidb_id":10},
        {"anilist_id":300,"anidb_id":[7,5,7]},
        {"anilist_id":400},
        {"anilist_id":500,"anidb_id":"oops"},
        {"anilist_id":600,"anidb_id":-3},
        {"anidb_id":99}
    ]"#;

    #[test]
    fn anidb_ids_are_one_to_many_sorted_and_deduped() {
        let data = Data::from_reduced(parse_dataset(SAMPLE.as_bytes()).unwrap());
        assert_eq!(data.anidb.get(&100), Some(&vec![10, 30]));
        assert_eq!(data.anidb.get(&300), Some(&vec![5, 7]));
        for missing in [400, 500, 600] {
            assert!(!data.anidb.contains_key(&missing), "{missing}");
        }
        assert_eq!(data.by_anidb.get(&10), Some(&vec![100, 200]));
        assert_eq!(data.by_anidb.get(&30), Some(&vec![100]));
        assert_eq!(data.by_anidb.get(&99), None);
    }

    fn temp_cache(name: &str, content: Option<&str>) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nyaa-stream-test-{}-{name}.json", std::process::id()));
        match content {
            Some(c) => std::fs::write(&path, c).unwrap(),
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        path
    }

    #[test]
    fn new_cache_round_trips_through_the_public_api() {
        let path = temp_cache("v2", Some(&serde_json::to_string(&parse_dataset(SAMPLE.as_bytes()).unwrap()).unwrap()));
        let lists = AnimeLists::load(path.clone());
        assert!(!lists.is_stale());
        assert_eq!(lists.tvdb_season(100), Some(1));
        assert_eq!(lists.anidb_ids(100), vec![10, 30]);
        assert_eq!(lists.anidb_ids(999), Vec::<u32>::new());
        assert_eq!(lists.anilist_ids_for_anidb(10), vec![100, 200]);
        assert_eq!(lists.anilist_ids_for_anidb(999), Vec::<u32>::new());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn old_format_cache_loads_empty_and_forces_refresh() {
        let path = temp_cache("v1", Some(r#"{"290":1,"1225":2}"#));
        let lists = AnimeLists::load(path.clone());
        assert_eq!(lists.tvdb_season(290), None);
        assert!(lists.anidb_ids(290).is_empty());
        assert!(lists.is_stale(), "a fresh-by-mtime old-format file must still refresh");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_cache_is_empty_and_stale() {
        let lists = AnimeLists::load(temp_cache("missing", None));
        assert_eq!(lists.tvdb_season(290), None);
        assert!(lists.is_stale());
    }
}
