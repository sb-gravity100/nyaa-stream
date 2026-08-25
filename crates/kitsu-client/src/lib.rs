use std::collections::HashMap;

use serde::{Deserialize, Serialize};

const KITSU_BASE_URL: &str = "https://kitsu.io/api/edge";
// Kitsu's max page size; episodes are fetched sorted by number ascending and
// capped at this many pages so a very long-running show (e.g. One Piece,
// 1000+ episodes) degrades gracefully instead of paging forever - matches
// the same graceful-degradation tradeoff the AniList streamingEpisodes path
// already had.
const EPISODES_PER_PAGE: u32 = 20;
const MAX_EPISODE_PAGES: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitsuMetadata {
    /// Wide (3360x800) banner image, meant for full-bleed backdrops - not to
    /// be confused with Kitsu's `posterImage`, which is a portrait poster
    /// like AniList's coverImage and unsuitable for a backdrop layer.
    pub background: Option<String>,
    /// episode number -> thumbnail url
    pub episode_thumbnails: HashMap<i32, String>,
}

#[derive(Deserialize)]
struct JsonApiList<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct JsonApiDoc<T> {
    data: T,
}

#[derive(Deserialize)]
struct MappingResource {
    relationships: MappingRelationships,
}

#[derive(Deserialize)]
struct MappingRelationships {
    item: MappingItemRelationship,
}

#[derive(Deserialize)]
struct MappingItemRelationship {
    data: Option<MappingItemRef>,
}

#[derive(Deserialize)]
struct MappingItemRef {
    id: String,
}

#[derive(Deserialize)]
struct AnimeResource {
    attributes: AnimeAttributes,
}

#[derive(Deserialize)]
struct AnimeAttributes {
    #[serde(rename = "coverImage")]
    cover_image: Option<KitsuCoverImage>,
}

#[derive(Deserialize)]
struct KitsuCoverImage {
    large: Option<String>,
}

#[derive(Deserialize)]
struct EpisodeResource {
    attributes: EpisodeAttributes,
}

#[derive(Deserialize)]
struct EpisodeAttributes {
    number: Option<i32>,
    thumbnail: Option<KitsuEpisodeThumbnail>,
}

#[derive(Deserialize)]
struct KitsuEpisodeThumbnail {
    original: Option<String>,
}

pub struct KitsuClient {
    http: reqwest::Client,
}

impl Default for KitsuClient {
    fn default() -> Self {
        Self::new()
    }
}

impl KitsuClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Resolves an AniList anime id to a Kitsu anime id via Kitsu's
    /// crowdsourced external-site mapping table. Verified live that a
    /// single AniList id can have multiple (sometimes conflicting) mapping
    /// rows - e.g. a main show and a spinoff both mapped to the same
    /// AniList id - so this takes the first row Kitsu returns, which in
    /// practice (verified against Re:Zero S4) is the earliest-created,
    /// canonical one.
    async fn resolve_kitsu_id(&self, anilist_id: i64) -> anyhow::Result<Option<i64>> {
        tracing::debug!(anilist_id, "resolving AniList id to Kitsu id");
        let resp = self
            .http
            .get(format!("{KITSU_BASE_URL}/mappings"))
            .query(&[
                ("filter[externalSite]", "anilist/anime".to_string()),
                ("filter[externalId]", anilist_id.to_string()),
                // Kitsu omits the relationship's resource-identifier
                // ("data") linkage unless the related resource is
                // explicitly requested - verified live: without this, every
                // mapping's `relationships.item.data` is simply absent, not
                // null, so resolution silently finds nothing.
                ("include", "item".to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;
        let parsed: JsonApiList<MappingResource> = resp.json().await?;
        let kitsu_id = parsed
            .data
            .into_iter()
            .find_map(|m| m.relationships.item.data.map(|r| r.id))
            .and_then(|id| id.parse::<i64>().ok());
        tracing::debug!(anilist_id, ?kitsu_id, "resolved Kitsu id");
        Ok(kitsu_id)
    }

    async fn get_background(&self, kitsu_id: i64) -> anyhow::Result<Option<String>> {
        let resp = self
            .http
            .get(format!("{KITSU_BASE_URL}/anime/{kitsu_id}"))
            .send()
            .await?
            .error_for_status()?;
        let parsed: JsonApiDoc<AnimeResource> = resp.json().await?;
        Ok(parsed
            .data
            .attributes
            .cover_image
            .and_then(|c| c.large))
    }

    async fn get_episode_thumbnails(&self, kitsu_id: i64) -> anyhow::Result<HashMap<i32, String>> {
        let mut thumbnails = HashMap::new();
        for page in 0..MAX_EPISODE_PAGES {
            let resp = self
                .http
                .get(format!("{KITSU_BASE_URL}/anime/{kitsu_id}/episodes"))
                .query(&[
                    ("page[limit]", EPISODES_PER_PAGE.to_string()),
                    ("page[offset]", (page * EPISODES_PER_PAGE).to_string()),
                    ("sort", "number".to_string()),
                ])
                .send()
                .await?
                .error_for_status()?;
            let parsed: JsonApiList<EpisodeResource> = resp.json().await?;
            let page_len = parsed.data.len();
            for ep in parsed.data {
                if let (Some(number), Some(thumb)) = (
                    ep.attributes.number,
                    ep.attributes.thumbnail.and_then(|t| t.original),
                ) {
                    thumbnails.insert(number, thumb);
                }
            }
            if page_len < EPISODES_PER_PAGE as usize {
                break;
            }
        }
        Ok(thumbnails)
    }

    /// Fetches everything the frontend needs for one anime in a single
    /// call: the wide backdrop image plus a per-episode thumbnail map.
    /// Returns `Ok(None)` when Kitsu has no mapping for this AniList id at
    /// all (rather than an error) - the frontend must still fall back to
    /// AniList's own poster art in that case.
    pub async fn get_metadata(&self, anilist_id: i64) -> anyhow::Result<Option<KitsuMetadata>> {
        let Some(kitsu_id) = self.resolve_kitsu_id(anilist_id).await? else {
            tracing::debug!(anilist_id, "no Kitsu mapping found");
            return Ok(None);
        };

        let (background, episode_thumbnails) =
            tokio::try_join!(self.get_background(kitsu_id), self.get_episode_thumbnails(kitsu_id))?;

        tracing::info!(
            anilist_id,
            kitsu_id,
            has_background = background.is_some(),
            thumbnail_count = episode_thumbnails.len(),
            "fetched Kitsu metadata"
        );

        Ok(Some(KitsuMetadata {
            background,
            episode_thumbnails,
        }))
    }
}
