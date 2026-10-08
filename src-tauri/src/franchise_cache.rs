//! Disk cache for `AniListClient::franchise`: the walk costs a request per
//! prequel/sequel hop, and a franchise's shape rarely changes, so a week-old
//! copy is served without asking AniList and any copy is served when AniList
//! fails. Lives under the cache dir (`franchise/<anilist id>.json`).

use std::{path::PathBuf, time::Duration};

use anilist_client::{AniListClient, Franchise};

const FRESH_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub struct FranchiseCache {
    dir: PathBuf,
}

impl FranchiseCache {
    pub fn new(dir: PathBuf) -> Self {
        FranchiseCache { dir }
    }

    fn path(&self, anilist_id: i64) -> PathBuf {
        self.dir.join(format!("{anilist_id}.json"))
    }

    async fn read(&self, anilist_id: i64) -> Option<(Franchise, Duration)> {
        let path = self.path(anilist_id);
        let modified = tokio::fs::metadata(&path).await.ok()?.modified().ok()?;
        let franchise = serde_json::from_slice(&tokio::fs::read(&path).await.ok()?).ok()?;
        Some((franchise, modified.elapsed().unwrap_or_default()))
    }

    /// The cached franchise, however old; no request.
    pub async fn cached(&self, anilist_id: i64) -> Option<Franchise> {
        self.read(anilist_id).await.map(|(franchise, _)| franchise)
    }

    /// The franchise, fetched when there is no fresh copy. None when AniList
    /// fails and nothing is cached: callers match without sibling data then.
    pub async fn get(&self, anilist: &AniListClient, anilist_id: i64) -> Option<Franchise> {
        let cached = self.read(anilist_id).await;
        if let Some((franchise, age)) = &cached {
            if *age < FRESH_FOR {
                tracing::debug!(anilist_id, age_s = age.as_secs(), "franchise served from cache");
                return Some(franchise.clone());
            }
        }
        match anilist.franchise(anilist_id).await {
            Ok(franchise) => {
                tracing::info!(anilist_id, relatives = franchise.relatives.len(), "franchise fetched");
                let write = async {
                    tokio::fs::create_dir_all(&self.dir).await?;
                    tokio::fs::write(self.path(anilist_id), serde_json::to_vec(&franchise)?).await?;
                    anyhow::Ok(())
                };
                if let Err(err) = write.await {
                    tracing::warn!(anilist_id, %err, "failed to cache franchise");
                }
                Some(franchise)
            }
            Err(err) => {
                tracing::warn!(anilist_id, %err, stale = cached.is_some(), "franchise fetch failed");
                cached.map(|(franchise, _)| franchise)
            }
        }
    }
}
