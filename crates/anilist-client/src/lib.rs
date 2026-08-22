use serde::{Deserialize, Serialize};

const ANILIST_URL: &str = "https://graphql.anilist.co";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeMedia {
    pub id: i64,
    pub title: AnimeTitle,
    pub description: Option<String>,
    pub episodes: Option<i32>,
    #[serde(rename = "coverImage")]
    pub cover_image: CoverImage,
    #[serde(rename = "averageScore")]
    pub average_score: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverImage {
    pub large: Option<String>,
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: T,
}

#[derive(Deserialize)]
struct SearchData {
    #[serde(rename = "Page")]
    page: SearchPage,
}

#[derive(Deserialize)]
struct SearchPage {
    media: Vec<AnimeMedia>,
}

#[derive(Deserialize)]
struct MediaData {
    #[serde(rename = "Media")]
    media: AnimeMedia,
}

const SEARCH_QUERY: &str = r#"
query ($search: String, $perPage: Int) {
  Page(page: 1, perPage: $perPage) {
    media(search: $search, type: ANIME, sort: SEARCH_MATCH) {
      id
      title { romaji english native }
      description(asHtml: false)
      episodes
      coverImage { large extraLarge }
      averageScore
    }
  }
}
"#;

const MEDIA_BY_ID_QUERY: &str = r#"
query ($id: Int) {
  Media(id: $id, type: ANIME) {
    id
    title { romaji english native }
    description(asHtml: false)
    episodes
    coverImage { large extraLarge }
    averageScore
  }
}
"#;

pub struct AniListClient {
    http: reqwest::Client,
}

impl Default for AniListClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn search(&self, query: &str, per_page: i32) -> anyhow::Result<Vec<AnimeMedia>> {
        let body = serde_json::json!({
            "query": SEARCH_QUERY,
            "variables": { "search": query, "perPage": per_page }
        });
        let resp: GraphQlResponse<SearchData> = self
            .http
            .post(ANILIST_URL)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.data.page.media)
    }

    pub async fn get_by_id(&self, id: i64) -> anyhow::Result<AnimeMedia> {
        let body = serde_json::json!({
            "query": MEDIA_BY_ID_QUERY,
            "variables": { "id": id }
        });
        let resp: GraphQlResponse<MediaData> = self
            .http
            .post(ANILIST_URL)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.data.media)
    }
}
