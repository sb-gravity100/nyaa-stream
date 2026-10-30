//! Artwork lookups for AniList entries. Secrets are passed by the caller and
//! are deliberately excluded from Debug output, URLs, and tracing fields.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const MAP_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
const TMDB_API: &str = "https://api.themoviedb.org/3";
const FANART_API: &str = "https://webservice.fanart.tv/v3.2";
const ARM_API: &str = "https://arm.haglund.dev/api/v2";
const MAP_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const TMDB_TTL_SECS: u64 = 180 * 24 * 60 * 60;
const FANART_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const ARM_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const SEASON_LOGO_UPLOAD_MAX_DISTANCE_DAYS: i64 = 180;
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
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub backdrop: Option<String>,
    /// Fanart.tv's independent show background, used by the media-page layer.
    #[serde(default)]
    pub fanart_backdrop: Option<String>,
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
    pub tvdb_season: Option<i32>,
    pub simkl_id: Option<u64>,
    pub anidb_id: Option<u64>,
    pub kitsu_id: Option<u64>,
    pub myanimelist_id: Option<u64>,
    pub imdb_id: Option<String>,
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

/// arm-server v2 `/ids` fields available from an AniList query. TMDB and
/// TVDB are intentionally absent: those IDs are one-to-many in the API spec.
#[derive(Serialize, Deserialize, Default)]
struct ArmIds {
    anilist: Option<i64>,
    anidb: Option<u64>,
    kitsu: Option<u64>,
    myanimelist: Option<u64>,
    simkl: Option<u64>,
    imdb: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct TmdbIds { tv: Option<u64>, movie: Option<TmdbMovieIds> }
/// Fribb stores movie IDs as an array because a single AniList entry can map
/// to several TMDB films. Accept a scalar too for older/partial map versions.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TmdbMovieIds { One(u64), Many(Vec<u64>) }
impl TmdbMovieIds {
    fn first(&self) -> Option<u64> {
        match self { Self::One(id) => Some(*id), Self::Many(ids) => ids.first().copied() }
    }
}
#[derive(Serialize, Deserialize, Default)]
struct TmdbFindResponse {
    #[serde(default)] tv_results: Vec<TmdbFindItem>,
    #[serde(default)] movie_results: Vec<TmdbFindItem>,
}
#[derive(Serialize, Deserialize)]
struct TmdbFindItem { id: u64 }
#[derive(Serialize, Deserialize)]
struct SeasonMap { tmdb: Option<i32>, tvdb: Option<i32> }
#[derive(Serialize, Deserialize)]
struct EpisodeOffset { tmdb: Option<i32> }

#[derive(Serialize, Deserialize, Default)]
struct Images { #[serde(default)] backdrops: Vec<TmdbImage>, #[serde(default)] logos: Vec<TmdbImage>, #[serde(default)] stills: Vec<TmdbImage> }
#[derive(Serialize, Deserialize)]
struct TmdbImage { file_path: String, #[serde(default)] width: u32, #[serde(default)] vote_average: f32, iso_639_1: Option<String> }

#[derive(Serialize, Deserialize, Default)]
struct TmdbConfiguration { #[serde(default)] images: TmdbImageConfiguration }
#[derive(Serialize, Deserialize, Default)]
struct TmdbImageConfiguration {
    secure_base_url: String,
    #[serde(default)] still_sizes: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct TmdbSeasonDetails {
    #[serde(default)]
    episodes: Vec<TmdbSeasonEpisode>,
}

#[derive(Serialize, Deserialize)]
struct TmdbSeasonEpisode {
    episode_number: i32,
    still_path: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct FanartResponse {
    #[serde(default)] showbackground: Vec<FanartImage>,
    #[serde(default)] clearlogo: Vec<FanartImage>,
}
#[derive(Serialize, Deserialize)]
struct FanartImage {
    url: String,
    #[serde(default)] likes: String,
    #[serde(default)] season: Option<FanartSeason>,
    #[serde(default)] added: Option<String>,
    copyright: Option<String>,
    copyright_link: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FanartSeason { Number(i32), Label(String) }
impl FanartSeason {
    fn matches(&self, season: i32) -> bool {
        match self { Self::Number(number) => *number == season, Self::Label(label) => label.parse::<i32>().ok() == Some(season) }
    }
    fn is_general(&self) -> bool { matches!(self, Self::Label(label) if label.eq_ignore_ascii_case("all")) }
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

    /// Get TMDB artwork and a separate fanart.tv show background for the media
    /// page's subdued image layer. Kitsu/AniList/poster fallback remains the app's responsibility.
    pub async fn get_artwork(&self, anilist_id: i64, season_start_date: Option<&str>) -> Result<Artwork> {
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
        if let (Some(key), Some(tvdb_id)) = (self.credentials.fanart_api_key.as_deref().filter(|s| !s.trim().is_empty()), ids.tvdb_id) {
            match self.fanart_images(tvdb_id, key).await {
                Ok(data) => {
                    let mut used_images = Vec::new();
                    if let Some(image) = best_fanart(&data.showbackground) {
                        artwork.fanart_backdrop = Some(image.url.clone());
                        if artwork.backdrop.is_none() {
                            artwork.backdrop = Some(image.url.clone());
                        }
                        used_images.push(image);
                    }
                    if let Some(image) = choose_fanart_logo(&data.clearlogo, ids.tvdb_season, season_start_date) {
                        tracing::debug!(anilist_id, season = ?ids.tvdb_season, season_start_date = ?season_start_date, "using season-specific fanart.tv logo");
                        artwork.logo = Some(image.url.clone());
                        used_images.push(image);
                    } else if artwork.logo.is_none() {
                        if let Some(image) = best_general_fanart_logo(&data.clearlogo) {
                            artwork.logo = Some(image.url.clone());
                            used_images.push(image);
                        }
                    }
                    if !used_images.is_empty() {
                        if artwork.source.as_deref() == Some("tmdb") {
                            artwork.source = Some("tmdb+fanart.tv".into());
                            let tmdb_attribution = artwork.attribution.take().unwrap_or_else(|| TMDB_ATTRIBUTION.into());
                            artwork.attribution = Some(format!("{tmdb_attribution} · {}", fanart_attribution(&used_images)));
                        } else {
                            artwork.source = Some("fanart.tv".into());
                            artwork.attribution = Some(fanart_attribution(&used_images));
                        }
                    }
                }
                Err(err) => tracing::warn!(anilist_id, provider = "fanart.tv", error = %err, "artwork provider failed"),
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
        let configuration = self.tmdb_configuration(token).await?;
        let season_key = format!("tmdb-season-{anilist_id}-{season}");
        let season_details: TmdbSeasonDetails = self.cached_json(&season_key, TMDB_TTL_SECS, || async {
            self.tmdb_get(&format!("/tv/{tv_id}/season/{season}"), token).await
        }).await?;
        if let Some(path) = season_details.episodes.iter()
            .find(|entry| i64::from(entry.episode_number) == episode)
            .and_then(|entry| entry.still_path.as_deref())
            .filter(|path| !path.is_empty())
        {
            return Ok(Some(EpisodeStill { url: tmdb_thumbnail_url(path, &configuration.images), source: "tmdb" }));
        }

        let key = format!("tmdb-episode-images-{anilist_id}-{season}-{episode}");
        let images: Images = self.cached_json(&key, TMDB_TTL_SECS, || async {
            self.tmdb_get_images(&format!("/tv/{tv_id}/season/{season}/episode/{episode}/images"), token).await
        }).await?;
        Ok(choose_still(&images.stills, &configuration.images).map(|url| EpisodeStill { url, source: "tmdb" }))
    }

    pub async fn ids(&self, anilist_id: i64) -> Result<AnimeIds> {
        let rows = match self.mapping().await {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!(anilist_id, error = %err, "Fribb artwork ID map unavailable; trying arm-server fallback");
                Vec::new()
            }
        };
        let mut found: Vec<_> = rows.into_iter().filter(|row| row.anilist_id == Some(anilist_id)).collect();
        // Prefer entries with explicit seasonal coordinates for split-cour mapping.
        found.sort_by_key(|row| row.season.as_ref().and_then(|s| s.tmdb.or(s.tvdb)).is_none());
        let row = found.into_iter().next();
        let row_tmdb = row.as_ref().and_then(|r| r.themoviedb_id.as_ref());
        let arm_needed = row_tmdb.and_then(|ids| ids.tv.or_else(|| ids.movie.as_ref().and_then(TmdbMovieIds::first))).is_none();
        let arm = if arm_needed {
            match self.arm_ids(anilist_id).await {
                Ok(ids) => ids,
                Err(err) => {
                    tracing::warn!(anilist_id, error = %err, "arm-server artwork ID fallback failed");
                    ArmIds::default()
                }
            }
        } else { ArmIds::default() };
        let tmdb = row.as_ref().and_then(|r| r.themoviedb_id.as_ref());
        let mut tmdb_tv_id = tmdb.and_then(|id| id.tv);
        let mut tmdb_movie_id = tmdb.and_then(|id| id.movie.as_ref()).and_then(TmdbMovieIds::first);
        if tmdb_tv_id.is_none() && tmdb_movie_id.is_none() {
            if let (Some(imdb_id), Some(token)) = (
                arm.imdb.as_deref().filter(|id| valid_imdb_id(id)),
                self.credentials.tmdb_read_access_token.as_deref().filter(|s| !s.trim().is_empty()),
            ) {
                match self.tmdb_find_ids(imdb_id, token).await {
                    Ok((tv_id, movie_id)) => {
                        tmdb_tv_id = tv_id;
                        tmdb_movie_id = movie_id;
                    }
                    Err(err) => tracing::warn!(anilist_id, provider = "tmdb", error = %err, "IMDb artwork ID fallback failed"),
                }
            }
        }
        Ok(AnimeIds {
            tmdb_tv_id,
            tmdb_movie_id,
            tmdb_season: row.as_ref().and_then(|r| r.season.as_ref()).and_then(|s| s.tmdb),
            tmdb_episode_offset: row.as_ref().and_then(|r| r.episode_offset.as_ref()).and_then(|o| o.tmdb),
            tvdb_id: row.as_ref().and_then(|r| r.tvdb_id),
            tvdb_season: row.as_ref().and_then(|r| r.season.as_ref()).and_then(|s| s.tvdb),
            simkl_id: row.as_ref().and_then(|r| r.simkl_id).or(arm.simkl),
            anidb_id: arm.anidb,
            kitsu_id: arm.kitsu,
            myanimelist_id: arm.myanimelist,
            imdb_id: arm.imdb,
        })
    }

    async fn arm_ids(&self, anilist_id: i64) -> Result<ArmIds> {
        let key = format!("arm-ids-anilist-{anilist_id}");
        self.cached_json(&key, ARM_TTL_SECS, || async {
            tracing::debug!(anilist_id, "requesting arm-server ID fallback");
            let id = anilist_id.to_string();
            let value: serde_json::Value = self.client.get(format!("{ARM_API}/ids"))
                .query(&[("source", "anilist"), ("id", id.as_str()), ("include", "anilist,anidb,kitsu,myanimelist,simkl,imdb")])
                .send().await?.error_for_status()?.json().await?;
            let relations = match value {
                serde_json::Value::Null => Vec::new(),
                serde_json::Value::Array(items) => items,
                item => vec![item],
            };
            for relation in relations.into_iter().filter(|v| !v.is_null()) {
                let ids: ArmIds = serde_json::from_value(relation)?;
                if ids.anilist == Some(anilist_id) { return Ok(ids); }
            }
            Ok(ArmIds::default())
        }).await
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
        self.cached_json(&key, TMDB_TTL_SECS, || async { self.tmdb_get_images(&format!("/{kind}/{id}/images"), token).await }).await
    }

    async fn tmdb_configuration(&self, token: &str) -> Result<TmdbConfiguration> {
        self.cached_json("tmdb-configuration", TMDB_TTL_SECS, || async { self.tmdb_get("/configuration", token).await }).await
    }

    async fn tmdb_find_ids(&self, imdb_id: &str, token: &str) -> Result<(Option<u64>, Option<u64>)> {
        let key = format!("tmdb-find-{imdb_id}");
        self.cached_json(&key, TMDB_TTL_SECS, || async {
            tracing::debug!(imdb_id, "requesting TMDB ID by external IMDb ID");
            Ok(self.client.get(format!("{TMDB_API}/find/{imdb_id}"))
                .bearer_auth(token)
                .query(&[("external_source", "imdb_id")])
                .send().await?.error_for_status()?.json::<TmdbFindResponse>().await?)
        }).await.map(|response| unambiguous_tmdb_ids(&response))
    }

    async fn tmdb_get<T: for<'de> Deserialize<'de>>(&self, path: &str, token: &str) -> Result<T> {
        tracing::debug!(path, "requesting TMDB metadata");
        Ok(self.client.get(format!("{TMDB_API}{path}")).bearer_auth(token).send().await?.error_for_status()?.json().await?)
    }

    async fn tmdb_get_images<T: for<'de> Deserialize<'de>>(&self, path: &str, token: &str) -> Result<T> {
        tracing::debug!(path, "requesting TMDB images");
        Ok(self.client.get(format!("{TMDB_API}{path}")).bearer_auth(token).query(&[("include_image_language", "en-US,null")]).send().await?.error_for_status()?.json().await?)
    }

    async fn fanart_images(&self, tvdb_id: u64, key: &str) -> Result<FanartResponse> {
        let cache_key = format!("fanart-tv-season-logos-{tvdb_id}");
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
fn valid_imdb_id(value: &str) -> bool {
    value.len() <= 50
        && value.strip_prefix("tt").is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
}
fn unambiguous_tmdb_ids(response: &TmdbFindResponse) -> (Option<u64>, Option<u64>) {
    match (response.tv_results.as_slice(), response.movie_results.as_slice()) {
        ([tv], []) => (Some(tv.id), None),
        ([], [movie]) => (None, Some(movie.id)),
        _ => (None, None),
    }
}
fn choose_logo(images: &[TmdbImage]) -> Option<String> {
    images.iter().max_by(|a,b| a.vote_average.total_cmp(&b.vote_average)).map(|i| tmdb_image_url(&i.file_path, "w500"))
}
fn choose_still(images: &[TmdbImage], configuration: &TmdbImageConfiguration) -> Option<String> {
    images.iter().filter(|image| !image.file_path.is_empty())
        .max_by(|a, b| a.vote_average.total_cmp(&b.vote_average).then(a.width.cmp(&b.width)))
        .map(|image| tmdb_thumbnail_url(&image.file_path, configuration))
}
fn tmdb_thumbnail_url(path: &str, configuration: &TmdbImageConfiguration) -> String {
    let base = if configuration.secure_base_url.is_empty() { "https://image.tmdb.org/t/p/" } else { &configuration.secure_base_url };
    let size = ["w500", "w780", "w300", "w185", "original"].into_iter()
        .find(|size| configuration.still_sizes.iter().any(|candidate| candidate.as_str() == *size))
        .unwrap_or("w500");
    format!("{}/{size}{path}", base.trim_end_matches('/'))
}
fn tmdb_image_url(path: &str, size: &str) -> String { format!("https://image.tmdb.org/t/p/{size}{path}") }
fn best_fanart(images: &[FanartImage]) -> Option<&FanartImage> {
    images.iter().max_by_key(|i| i.likes.parse::<u32>().unwrap_or(0))
}
fn best_season_fanart_logo(images: &[FanartImage], season: Option<i32>) -> Option<&FanartImage> {
    let season = season?;
    images.iter()
        .filter(|image| image.season.as_ref().is_some_and(|value| value.matches(season)))
        .max_by_key(|image| image.likes.parse::<u32>().unwrap_or(0))
}
fn choose_fanart_logo<'a>(images: &'a [FanartImage], season: Option<i32>, season_start_date: Option<&str>) -> Option<&'a FanartImage> {
    best_season_fanart_logo(images, season).or_else(|| closest_fanart_logo(images, season_start_date))
}
fn best_general_fanart_logo(images: &[FanartImage]) -> Option<&FanartImage> {
    images.iter()
        .filter(|image| image.season.as_ref().is_none_or(FanartSeason::is_general))
        .max_by_key(|image| image.likes.parse::<u32>().unwrap_or(0))
}
fn closest_fanart_logo<'a>(images: &'a [FanartImage], season_start_date: Option<&str>) -> Option<&'a FanartImage> {
    let target_day = season_start_date.and_then(date_ordinal)?;
    images.iter()
        .filter_map(|image| {
            if !image.season.as_ref().is_none_or(FanartSeason::is_general) { return None; }
            let uploaded_day = image.added.as_deref().and_then(date_ordinal)?;
            let distance = (uploaded_day - target_day).abs();
            (distance <= SEASON_LOGO_UPLOAD_MAX_DISTANCE_DAYS).then_some((image, distance))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(image, _)| image)
}
fn date_ordinal(value: &str) -> Option<i64> {
    let date = value.get(..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) { return None; }
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imdb_ids_are_validated_before_becoming_tmdb_paths() {
        assert!(valid_imdb_id("tt1234567"));
        assert!(!valid_imdb_id("1234567"));
        assert!(!valid_imdb_id("tt"));
        assert!(!valid_imdb_id("tt12/3"));
        assert!(!valid_imdb_id(&format!("tt{}", "1".repeat(49))));
    }

    #[test]
    fn tmdb_find_accepts_only_one_unambiguous_media_result() {
        let tv = TmdbFindResponse { tv_results: vec![TmdbFindItem { id: 42 }], ..Default::default() };
        assert_eq!(unambiguous_tmdb_ids(&tv), (Some(42), None));

        let movie = TmdbFindResponse { movie_results: vec![TmdbFindItem { id: 84 }], ..Default::default() };
        assert_eq!(unambiguous_tmdb_ids(&movie), (None, Some(84)));

        let ambiguous = TmdbFindResponse {
            tv_results: vec![TmdbFindItem { id: 42 }],
            movie_results: vec![TmdbFindItem { id: 84 }],
        };
        assert_eq!(unambiguous_tmdb_ids(&ambiguous), (None, None));
    }

    #[test]
    fn fribb_tmdb_movie_ids_accept_single_and_multiple_ids() {
        let multiple: TmdbIds = serde_json::from_str(r#"{"tv":123,"movie":[456,789]}"#).unwrap();
        assert_eq!(multiple.movie.as_ref().and_then(TmdbMovieIds::first), Some(456));
        let single: TmdbIds = serde_json::from_str(r#"{"movie":456}"#).unwrap();
        assert_eq!(single.movie.as_ref().and_then(TmdbMovieIds::first), Some(456));
    }

    #[test]
    fn tmdb_episode_still_uses_a_thumbnail_size() {
        let details: TmdbSeasonDetails = serde_json::from_str(r#"{"episodes":[{"episode_number":7,"still_path":"/episode-still.jpg"}]}"#).unwrap();
        let path = details.episodes.iter().find(|entry| entry.episode_number == 7).and_then(|entry| entry.still_path.as_deref()).unwrap();
        let configuration = TmdbImageConfiguration {
            secure_base_url: "https://image.tmdb.org/t/p/".into(),
            still_sizes: vec!["w92".into(), "w300".into(), "w500".into(), "w780".into(), "original".into()],
        };
        assert_eq!(tmdb_thumbnail_url(path, &configuration), "https://image.tmdb.org/t/p/w500/episode-still.jpg");
    }

    #[test]
    fn tmdb_still_url_uses_a_supported_fallback_size() {
        let configuration = TmdbImageConfiguration {
            secure_base_url: "https://images.test/p/".into(),
            still_sizes: vec!["w92".into(), "w300".into(), "original".into()],
        };
        assert_eq!(tmdb_thumbnail_url("/still.jpg", &configuration), "https://images.test/p/w300/still.jpg");
    }

    #[test]
    fn fanart_logo_prefers_the_mapped_season_then_general_logo() {
        let images: Vec<FanartImage> = serde_json::from_str(r#"[
            {"url":"generic.png","likes":"100"},
            {"url":"season-one.png","season":"1","likes":"500"},
            {"url":"season-two.png","season":2,"likes":"1","added":"2020-01-01 00:00:00"},
            {"url":"all-seasons.png","season":"all","likes":"50"}
        ]"#).unwrap();
        assert_eq!(choose_fanart_logo(&images, Some(2), Some("2020-01-01")).map(|image| image.url.as_str()), Some("season-two.png"));
        assert!(best_season_fanart_logo(&images, None).is_none());
        assert_eq!(best_general_fanart_logo(&images).map(|image| image.url.as_str()), Some("generic.png"));
    }

    #[test]
    fn fribb_season_map_retains_both_provider_season_numbers() {
        let season: SeasonMap = serde_json::from_str(r#"{"tmdb":3,"tvdb":2}"#).unwrap();
        assert_eq!(season.tmdb, Some(3));
        assert_eq!(season.tvdb, Some(2));
    }

    #[test]
    fn fanart_logo_uploads_use_nearest_date_within_six_months() {
        let images: Vec<FanartImage> = serde_json::from_str(r#"[
            {"url":"near-before.png","added":"2026-03-20 00:00:00"},
            {"url":"near-after.png","added":"2026-04-15 00:00:00"},
            {"url":"other-season.png","season":"1","added":"2026-03-15 00:00:00"},
            {"url":"too-old.png","added":"2025-09-01 00:00:00"}
        ]"#).unwrap();
        assert_eq!(closest_fanart_logo(&images, Some("2026-04-01")).map(|image| image.url.as_str()), Some("near-before.png"));
        assert!(closest_fanart_logo(&images, Some("2020-04-01")).is_none());
    }

    #[test]
    fn fanart_backdrop_serializes_camel_case_and_old_cache_entries_still_load() {
        let artwork: Artwork = serde_json::from_str(r#"{"backdrop":"tmdb.jpg","logo":null,"source":"tmdb","attribution":"TMDB"}"#).unwrap();
        assert_eq!(artwork.fanart_backdrop, None);
        let artwork = Artwork { fanart_backdrop: Some("fanart.jpg".into()), ..artwork };
        let json = serde_json::to_value(artwork).unwrap();
        assert_eq!(json["fanartBackdrop"], "fanart.jpg");
    }
}
