//! Kitsu anime lookups used as a fallback when AniList is unavailable (rate
//! limited or down): text search and by-AniList-id. The app keys everything
//! by AniList id, so every result carries the AniList id Kitsu maps it to;
//! entries without such a mapping are dropped.

use serde::Serialize;
use serde_json::Value;

use crate::{KitsuClient, KITSU_BASE_URL};

/// Kitsu's page size cap.
const MAX_PAGE: u32 = 20;

/// One Kitsu anime, reduced to what the app displays.
#[derive(Debug, Clone, Serialize)]
pub struct KitsuAnime {
    pub anilist_id: i64,
    pub kitsu_id: i64,
    /// `titles.en` / `en_us`.
    pub title_english: Option<String>,
    /// `titles.en_jp` (romanized).
    pub title_romaji: Option<String>,
    /// `titles.ja_jp`.
    pub title_native: Option<String>,
    pub canonical_title: Option<String>,
    pub synopsis: Option<String>,
    pub episode_count: Option<i32>,
    /// Minutes per episode.
    pub episode_length: Option<i32>,
    /// `TV`, `movie`, `OVA`, `ONA`, `special`, `music`.
    pub subtype: Option<String>,
    /// `YYYY-MM-DD`.
    pub start_date: Option<String>,
    /// 0-100, Kitsu's average rating.
    pub average_rating: Option<f64>,
    pub poster_large: Option<String>,
    pub poster_original: Option<String>,
}

impl KitsuClient {
    /// Text search, most relevant first.
    pub async fn search_anime(&self, query: &str, limit: u32) -> anyhow::Result<Vec<KitsuAnime>> {
        tracing::debug!(query, limit, "Kitsu anime search");
        let resp = self
            .http
            .get(format!("{KITSU_BASE_URL}/anime"))
            .query(&[
                ("filter[text]", query.to_string()),
                ("page[limit]", limit.min(MAX_PAGE).to_string()),
                ("include", "mappings".to_string()),
            ])
            .send()
            .await?
            .error_for_status()
            .inspect_err(|err| tracing::error!(query, %err, "Kitsu anime search failed"))?;
        let doc: Value = resp.json().await?;
        let anilist_ids = anilist_ids_by_mapping(&doc);
        let results: Vec<KitsuAnime> = doc["data"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|anime| {
                let anilist_id = anime["relationships"]["mappings"]["data"]
                    .as_array()?
                    .iter()
                    .find_map(|m| m["id"].as_str().and_then(|id| anilist_ids.get(id)).copied())?;
                parse_anime(anime, anilist_id)
            })
            .collect();
        tracing::info!(query, count = results.len(), "Kitsu anime search succeeded");
        Ok(results)
    }

    /// The Kitsu entry mapped to an AniList id, if any.
    pub async fn anime_by_anilist_id(&self, anilist_id: i64) -> anyhow::Result<Option<KitsuAnime>> {
        tracing::debug!(anilist_id, "Kitsu anime by AniList id");
        let resp = self
            .http
            .get(format!("{KITSU_BASE_URL}/mappings"))
            .query(&[
                ("filter[externalSite]", "anilist/anime".to_string()),
                ("filter[externalId]", anilist_id.to_string()),
                ("include", "item".to_string()),
            ])
            .send()
            .await?
            .error_for_status()
            .inspect_err(|err| tracing::error!(anilist_id, %err, "Kitsu mapping lookup failed"))?;
        let doc: Value = resp.json().await?;
        let anime = doc["included"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["type"] == "anime"))
            .and_then(|anime| parse_anime(anime, anilist_id));
        tracing::info!(anilist_id, found = anime.is_some(), "Kitsu anime by AniList id done");
        Ok(anime)
    }
}

/// Mapping resource id -> AniList id, from a response's `included`.
fn anilist_ids_by_mapping(doc: &Value) -> std::collections::HashMap<String, i64> {
    doc["included"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|item| item["type"] == "mappings" && item["attributes"]["externalSite"] == "anilist/anime")
        .filter_map(|item| {
            let id = item["id"].as_str()?.to_string();
            let anilist_id = item["attributes"]["externalId"].as_str()?.parse().ok()?;
            Some((id, anilist_id))
        })
        .collect()
}

fn parse_anime(anime: &Value, anilist_id: i64) -> Option<KitsuAnime> {
    let a = &anime["attributes"];
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let int = |v: &Value| v.as_i64().map(|n| n as i32);
    Some(KitsuAnime {
        anilist_id,
        kitsu_id: anime["id"].as_str()?.parse().ok()?,
        title_english: text(&a["titles"]["en"]).or_else(|| text(&a["titles"]["en_us"])),
        title_romaji: text(&a["titles"]["en_jp"]),
        title_native: text(&a["titles"]["ja_jp"]),
        canonical_title: text(&a["canonicalTitle"]),
        synopsis: text(&a["synopsis"]),
        episode_count: int(&a["episodeCount"]),
        episode_length: int(&a["episodeLength"]),
        subtype: text(&a["subtype"]),
        start_date: text(&a["startDate"]),
        average_rating: a["averageRating"].as_str().and_then(|r| r.parse().ok()),
        poster_large: text(&a["posterImage"]["large"]),
        poster_original: text(&a["posterImage"]["original"]),
    })
}
