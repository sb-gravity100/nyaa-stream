//! Artwork lookups for AniList entries. Secrets are passed by the caller and
//! are deliberately excluded from Debug output, URLs, and tracing fields.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const MAP_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
const TMDB_API: &str = "https://api.themoviedb.org/3";
const FANART_API: &str = "https://webservice.fanart.tv/v3.2";
const MAP_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const TMDB_TTL_SECS: u64 = 180 * 24 * 60 * 60;
const FANART_TTL_SECS: u64 = 7 * 24 * 60 * 60;
pub const TMDB_API_SETTINGS_URL: &str = "https://www.themoviedb.org/settings/api";
pub const FANART_API_KEY_URL: &str = "https://fanart.tv/get-an-api-key/";
pub const TMDB_ATTRIBUTION: &str = "This product uses the TMDB API but is not endorsed or certified by TMDB.";

/// Optional user-entered credentials. Never derive Debug: values are secrets.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentials {
    pub tmdb_read_access_token: Option<String>,
    pub fanart_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artwork {
    pub backdrop: Option<String>,
    pub logo: Option<String>,
    pub source: Option<String>,
    pub attribution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeStill {
    pub url: String,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnimeIds {
    pub tmdb_tv_id: Option<u64>,
    pub tmdb_movie_id: Option<u64>,
    pub tmdb_season: Option<i32>,
    pub tmdb_episode_offset: Option<i32>,
    pub tvdb_id: Option<u64>,
    pub simkl_id: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct Timed<T> { fetched_at: u64, value: T }

#[derive(Serialize, Deserialize)]
struct MapRow {
    anilist_id: Option<i64>,
    themoviedb_id: Option<TmdbIds>,
    tvdb_id: Option<u64>,
    simkl_id: Option<u64>,
    season: Option<SeasonMap>,
    episode_offset: Option<EpisodeOffset>,
}
#[derive(Serialize, Deserialize)]
struct TmdbIds { tv: Option<u64>, movie: Option<u64> }
#[derive(Serialize, Deserialize)]
struct SeasonMap { tmdb: Option<i32> }
#[derive(Serialize, Deserialize)]
struct EpisodeOffset { tmdb: Option<i32> }

#[derive(Serialize, Deserialize, Default)]
struct Images { #[serde(default)] backdrops: Vec<TmdbImage>, #[serde(default)] logos: Vec<TmdbImage>, #[serde(default)] stills: Vec<TmdbImage> }
#[derive(Serialize, Deserialize)]
struct TmdbImage { file_path: String, #[serde(default)] width: u32, #[serde(default)] vote_average: f32, iso_639_1: Option<String> }

#[derive(Serialize, Deserialize, Default)]
struct FanartResponse {
    #[serde(default)] showbackground: Vec<FanartImage>,
    #[serde(default)] clearlogo: Vec<FanartImage>,
}
#[derive(Serialize, Deserialize)]
struct FanartImage {
    url: String,
    #[serde(default)] likes: String,
    copyright: Option<String>,
    copyright_link: Option<String>,
}

pub struct ArtworkClient {
    client: reqwest::Client,
    cache_dir: PathBuf,
    credentials: ProviderCredentials,
}

impl ArtworkClient {
    pub fn new(cache_dir: impl Into<PathBuf>, credentials: ProviderCredentials) -> Result<Self> {
        let client = reqwest::Client::builder().user_agent(concat!("nyaa-stream/", env!("CARGO_PKG_VERSION"))).build()?;
        Ok(Self { client, cache_dir: cache_dir.into(), credentials })
    }

    /// Get the first usable backdrop/logo. TMDB is preferred, then fanart.tv.
    /// Kitsu/AniList/poster fallback remains the app's responsibility.
    pub async fn get_artwork(&self, anilist_id: i64) -> Result<Artwork> {
        tracing::debug!(anilist_id, "artwork lookup started");
        let ids = self.ids(anilist_id).await?;
        let mut artwork = Artwork::default();
        if let Some(token) = self.credentials.tmdb_read_access_token.as_deref().filter(|s| !s.trim().is_empty()) {
            if let Some((kind, id)) = ids.tmdb_tv_id.map(|id| ("tv", id)).or(ids.tmdb_movie_id.map(|id| ("movie", id))) {
                match self.tmdb_images(kind, id, token).await {
                    Ok(images) => {
                        artwork.backdrop = choose_backdrop(&images.backdrops);
                        artwork.logo = choose_logo(&images.logos);
                        if artwork.backdrop.is_some() || artwork.logo.is_some() {
                            artwork.source = Some("tmdb".into());
                            artwork.attribution = Some(TMDB_ATTRIBUTION.into());
                        }
                    }
                    Err(err) => tracing::warn!(anilist_id, provider = "tmdb", error = %err, "artwork provider failed"),
                }
            }
        }
        if artwork.backdrop.is_none() || artwork.logo.is_none() {
            if let (Some(key), Some(tvdb_id)) = (self.credentials.fanart_api_key.as_deref().filter(|s| !s.trim().is_empty()), ids.tvdb_id) {
                match self.fanart_images(tvdb_id, key).await {
                    Ok(data) => {
                        let mut used_images = Vec::new();
                        if artwork.backdrop.is_none() {
                            if let Some(image) = best_fanart(&data.showbackground) {
                                artwork.backdrop = Some(image.url.clone());
                                used_images.push(image);
                            }
                        }
                        if artwork.logo.is_none() {
                            if let Some(image) = best_fanart(&data.clearlogo) {
                                artwork.logo = Some(image.url.clone());
                                used_images.push(image);
                            }
                        }
                        if !used_images.is_empty() {
                            if artwork.source.as_deref() == Some("tmdb") {
                                artwork.source = Some("tmdb+fanart.tv".into());
                                artwork.attribution = Some(format!("{} · {}", TMDB_ATTRIBUTION, fanart_attribution(&used_images)));
                            } else {
                                artwork.source = Some("fanart.tv".into());
                                artwork.attribution = Some(fanart_attribution(&used_images));
                            }
                        }
                    }
                    Err(err) => tracing::warn!(anilist_id, provider = "fanart.tv", error = %err, "artwork provider failed"),
                }
            }
        }
        tracing::debug!(anilist_id, source = ?artwork.source, "artwork lookup finished");
        Ok(artwork)
    }

    /// Get an episode still, correcting AniList numbering by the dataset's TMDB offset.
    pub async fn get_episode_still(&self, anilist_id: i64, episode: u32) -> Result<Option<EpisodeStill>> {
        let ids = self.ids(anilist_id).await?;
        let Some(tv_id) = ids.tmdb_tv_id else { return Ok(None) };
        let Some(season) = ids.tmdb_season else { return Ok(None) };
        let episode = i64::from(episode) + i64::from(ids.tmdb_episode_offset.unwrap_or(0));
        if episode < 1 || episode > i64::from(u32::MAX) { return Ok(None); }
        let Some(token) = self.credentials.tmdb_read_access_token.as_deref().filter(|s| !s.trim().is_empty()) else { return Ok(None) };
        let key = format!("tmdb-episode-{anilist_id}-{season}-{episode}");
        let images: Images = self.cached_json(&key, TMDB_TTL_SECS, || async {
            self.tmdb_get(&format!("/tv/{tv_id}/season/{season}/episode/{episode}/images"), token).await
        }).await?;
        Ok(choose_backdrop(&images.stills).map(|url| EpisodeStill { url, source: "tmdb" }))
    }

    pub async fn ids(&self, anilist_id: i64) -> Result<AnimeIds> {
        let rows = self.mapping().await?;
        let mut found: Vec<_> = rows.into_iter().filter(|row| row.anilist_id == Some(anilist_id)).collect();
        // Prefer entries with explicit seasonal coordinates for split-cour mapping.
        found.sort_by_key(|row| row.season.as_ref().and_then(|s| s.tmdb).is_none());
        let row = found.into_iter().next().ok_or_else(|| anyhow!("AniList id has no artwork ID mapping"))?;
        let tmdb = row.themoviedb_id.unwrap_or(TmdbIds { tv: None, movie: None });
        Ok(AnimeIds { tmdb_tv_id: tmdb.tv, tmdb_movie_id: tmdb.movie, tmdb_season: row.season.and_then(|s| s.tmdb), tmdb_episode_offset: row.episode_offset.and_then(|o| o.tmdb), tvdb_id: row.tvdb_id, simkl_id: row.simkl_id })
    }

    async fn mapping(&self) -> Result<Vec<MapRow>> {
        let key = "fribb-anime-list-full";
        self.cached_json(key, MAP_TTL_SECS, || async {
            tracing::debug!(url = MAP_URL, "refreshing artwork ID map");
            self.client.get(MAP_URL).send().await?.error_for_status()?.json::<Vec<MapRow>>().await.map_err(Into::into)
        }).await
    }

    async fn tmdb_images(&self, kind: &str, id: u64, token: &str) -> Result<Images> {
        let key = format!("tmdb-images-{kind}-{id}");
        self.cached_json(&key, TMDB_TTL_SECS, || async { self.tmdb_get(&format!("/{kind}/{id}/images"), token).await }).await
    }

    async fn tmdb_get<T: for<'de> Deserialize<'de>>(&self, path: &str, token: &str) -> Result<T> {
        tracing::debug!(path, "requesting TMDB metadata");
        Ok(self.client.get(format!("{TMDB_API}{path}")).bearer_auth(token).query(&[("include_image_language", "en,null")]).send().await?.error_for_status()?.json().await?)
    }

    async fn fanart_images(&self, tvdb_id: u64, key: &str) -> Result<FanartResponse> {
        let cache_key = format!("fanart-tv-{tvdb_id}");
        self.cached_json(&cache_key, FANART_TTL_SECS, || async {
            tracing::debug!(tvdb_id, "requesting fanart.tv metadata");
            Ok(self.client.get(format!("{FANART_API}/tv/{tvdb_id}")).header("api-key", key).send().await?.error_for_status()?.json().await?)
        }).await
    }

    async fn cached_json<T, F, Fut>(&self, key: &str, ttl: u64, fetch: F) -> Result<T>
    where T: Serialize + for<'de> Deserialize<'de>, F: FnOnce() -> Fut, Fut: std::future::Future<Output = Result<T>> {
        let path = self.cache_dir.join("nyaa-stream").join("artwork").join(format!("{key}.json"));
        if let Some(value) = read_fresh::<T>(&path, ttl).await { return Ok(value); }
        match fetch().await {
            Ok(value) => {
                if let Err(err) = write_timed(&path, &value).await { tracing::warn!(cache_key = key, error = %err, "could not persist artwork cache"); }
                Ok(value)
            }
            Err(err) => {
                if let Some(stale) = read_stale::<T>(&path).await { tracing::warn!(cache_key = key, error = %err, "using stale artwork cache"); Ok(stale) }
                else { Err(err) }
            }
        }
    }
}

fn choose_backdrop(images: &[TmdbImage]) -> Option<String> {
    images.iter().filter(|i| i.iso_639_1.is_none() && i.width >= 1920)
        .max_by(|a,b| a.vote_average.total_cmp(&b.vote_average))
        .or_else(|| images.iter().filter(|i| i.width >= 1920).max_by(|a,b| a.vote_average.total_cmp(&b.vote_average)))
        .or_else(|| images.iter().max_by(|a,b| a.vote_average.total_cmp(&b.vote_average)))
        .map(|i| tmdb_image_url(&i.file_path, "w1280"))
}
fn choose_logo(images: &[TmdbImage]) -> Option<String> {
    images.iter().max_by(|a,b| a.vote_average.total_cmp(&b.vote_average)).map(|i| tmdb_image_url(&i.file_path, "w500"))
}
fn tmdb_image_url(path: &str, size: &str) -> String { format!("https://image.tmdb.org/t/p/{size}{path}") }
fn best_fanart(images: &[FanartImage]) -> Option<&FanartImage> {
    images.iter().max_by_key(|i| i.likes.parse::<u32>().unwrap_or(0))
}
fn fanart_attribution(images: &[&FanartImage]) -> String {
    let details: Vec<_> = images.iter().filter_map(|image| {
        match (image.copyright.as_deref().filter(|v| !v.is_empty()), image.copyright_link.as_deref().filter(|v| !v.is_empty())) {
            (Some(owner), Some(link)) => Some(format!("{owner} ({link})")),
            (Some(owner), None) => Some(owner.to_owned()),
            _ => None,
        }
    }).collect();
    if details.is_empty() { "fanart.tv".into() }
    else { format!("fanart.tv · {}", details.join(" · ")) }
}

async fn read_fresh<T: for<'de> Deserialize<'de>>(path: &Path, ttl: u64) -> Option<T> {
    let cached: Timed<T> = read(path).await?;
    (now().saturating_sub(cached.fetched_at) < ttl).then_some(cached.value)
}
async fn read_stale<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> { read::<Timed<T>>(path).await.map(|v| v.value) }
async fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> { serde_json::from_slice(&tokio::fs::read(path).await.ok()?).ok() }
async fn write_timed<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    tokio::fs::create_dir_all(path.parent().context("cache path has no parent")?).await?;
    let bytes = serde_json::to_vec(&Timed { fetched_at: now(), value })?;
    tokio::fs::write(path, bytes).await?;
    Ok(())
}
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
