//! On-disk cache of nyaa.si responses, so a rate limit (HTTP 429) or outage
//! doesn't leave the app without sources for something it has seen before.
//!
//! Searches are fresh for `SEARCH_FRESH_FOR`; after that they're refetched,
//! but a failed refetch falls back to the cached copy whatever its age.
//! View-page details (submitter, batch status) don't change once a torrent
//! is uploaded, so they never expire.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// How long a cached search is used without asking nyaa.si again.
pub const SEARCH_FRESH_FOR: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    /// Unix seconds.
    fetched_at: u64,
    value: T,
}

pub(crate) struct Cached<T> {
    pub value: T,
    pub age: Duration,
}

#[derive(Clone)]
pub(crate) struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// File for `key` in `kind`'s folder, named by a stable FNV-1a hash (the
    /// key itself - a query or URL - isn't a safe file name).
    fn path(&self, kind: &str, key: &str) -> PathBuf {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in key.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.dir.join(kind).join(format!("{hash:016x}.json"))
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, kind: &str, key: &str) -> Option<Cached<T>> {
        let path = self.path(kind, key);
        let bytes = tokio::fs::read(&path).await.ok()?;
        match serde_json::from_slice::<Entry<T>>(&bytes) {
            Ok(entry) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                Some(Cached { value: entry.value, age: Duration::from_secs(now.saturating_sub(entry.fetched_at)) })
            }
            Err(err) => {
                tracing::warn!(?path, %err, "unreadable nyaa cache entry, ignoring");
                None
            }
        }
    }

    pub(crate) async fn put<T: Serialize>(&self, kind: &str, key: &str, value: &T) {
        let path = self.path(kind, key);
        let fetched_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let result = async {
            tokio::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).await?;
            let bytes = serde_json::to_vec(&Entry { fetched_at, value })?;
            // Write-then-rename so a crash never leaves a torn entry.
            let tmp = path.with_extension("tmp");
            tokio::fs::write(&tmp, bytes).await?;
            tokio::fs::rename(&tmp, &path).await?;
            anyhow::Ok(())
        }
        .await;
        if let Err(err) = result {
            tracing::warn!(?path, %err, "failed to write nyaa cache entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiskCache;

    #[tokio::test]
    async fn round_trips_and_reports_age() {
        let dir = std::env::temp_dir().join(format!("nyaa_cache_test_{}", std::process::id()));
        let cache = DiskCache::new(dir.clone());
        assert!(cache.get::<Vec<String>>("search", "1_2|frieren").await.is_none());
        cache.put("search", "1_2|frieren", &vec!["a".to_string()]).await;
        let hit = cache.get::<Vec<String>>("search", "1_2|frieren").await.unwrap();
        assert_eq!(hit.value, vec!["a".to_string()]);
        assert!(hit.age.as_secs() < 5);
        // Different kind or key: separate entries.
        assert!(cache.get::<Vec<String>>("details", "1_2|frieren").await.is_none());
        assert!(cache.get::<Vec<String>>("search", "1_2|frieren 2").await.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
