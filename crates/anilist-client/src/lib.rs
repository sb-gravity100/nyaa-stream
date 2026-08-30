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
    pub format: Option<String>,
    pub season: Option<String>,
    #[serde(rename = "seasonYear")]
    pub season_year: Option<i32>,
    /// Typical per-episode runtime in minutes, per AniList. Used as a
    /// player duration estimate that doesn't depend on scanning the video
    /// file itself - see PLAN.md's Known gaps on why `ffprobe`-derived
    /// duration is unreliable for a freshly-downloading torrent.
    pub duration: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverImage {
    pub large: Option<String>,
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
}

/// One episode's airing record, for building a cross-anime "latest
/// episodes" feed. Sourced from AniList's `Page.airingSchedules`, which
/// (verified live) supports filtering by a list of media ids and an
/// airing-time range in one batched request — exactly what a library-based
/// feed needs, rather than one request per saved anime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiringEntry {
    pub episode: i32,
    #[serde(rename = "airingAt")]
    pub airing_at: i64,
    pub media: AiringMedia,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiringMedia {
    pub id: i64,
    pub title: AnimeTitle,
    #[serde(rename = "coverImage")]
    pub cover_image: CoverImage,
    pub duration: Option<i32>,
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

#[derive(Deserialize)]
struct AiringSchedulesData {
    #[serde(rename = "Page")]
    page: AiringSchedulesPage,
}

#[derive(Deserialize)]
struct AiringSchedulesPage {
    #[serde(rename = "airingSchedules")]
    airing_schedules: Vec<AiringEntry>,
}

// Sorted by release date, newest first, not AniList's default text-match
// relevance: `search` already restricts results to relevant titles, and
// sort just orders that set — verified live that a "Frieren" search still
// returns exactly the 6 relevant Frieren entries, just ordered newest
// season first (2027) down to the original (2023) instead of relevance
// order (which put the original season first regardless of how many newer
// seasons exist).
const SEARCH_QUERY: &str = r#"
query ($search: String, $perPage: Int) {
  Page(page: 1, perPage: $perPage) {
    media(search: $search, type: ANIME, sort: START_DATE_DESC) {
      id
      title { romaji english native }
      description(asHtml: false)
      episodes
      coverImage { large extraLarge }
      averageScore
      format
      season
      seasonYear
      duration
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
    format
    season
    seasonYear
    duration
  }
}
"#;

/// Capped at 50 (AniList's practical max for this connection) and sorted
/// newest-first, so if a library is large enough to exceed it, the results
/// that get dropped are the oldest ones — the correct end to truncate for
/// a "latest episodes" feed.
const AIRING_SCHEDULES_QUERY: &str = r#"
query ($ids: [Int], $from: Int, $to: Int, $perPage: Int) {
  Page(perPage: $perPage) {
    airingSchedules(mediaId_in: $ids, airingAt_greater: $from, airingAt_lesser: $to, sort: TIME_DESC) {
      episode
      airingAt
      media { id title { romaji english native } coverImage { large extraLarge } duration }
    }
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
        tracing::debug!(query, per_page, url = ANILIST_URL, "sending AniList search request");
        let body = serde_json::json!({
            "query": SEARCH_QUERY,
            "variables": { "search": query, "perPage": per_page }
        });
        let resp = match self.http.post(ANILIST_URL).json(&body).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(query, %err, "AniList request failed to send");
                return Err(err.into());
            }
        };
        let resp = match resp.error_for_status() {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(query, %err, "AniList returned an error status");
                return Err(err.into());
            }
        };
        let parsed: GraphQlResponse<SearchData> = match resp.json().await {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::error!(query, %err, "failed to parse AniList response body");
                return Err(err.into());
            }
        };
        tracing::debug!(query, count = parsed.data.page.media.len(), "AniList search returned results");
        Ok(parsed.data.page.media)
    }

    pub async fn get_by_id(&self, id: i64) -> anyhow::Result<AnimeMedia> {
        tracing::debug!(id, url = ANILIST_URL, "sending AniList get_by_id request");
        let body = serde_json::json!({
            "query": MEDIA_BY_ID_QUERY,
            "variables": { "id": id }
        });
        let resp = match self.http.post(ANILIST_URL).json(&body).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(id, %err, "AniList get_by_id request failed to send");
                return Err(err.into());
            }
        };
        let resp = match resp.error_for_status() {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(id, %err, "AniList get_by_id returned an error status");
                return Err(err.into());
            }
        };
        let parsed: GraphQlResponse<MediaData> = match resp.json().await {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::error!(id, %err, "failed to parse AniList get_by_id response body");
                return Err(err.into());
            }
        };
        Ok(parsed.data.media)
    }

    /// Fetches recent episode airing records for a set of AniList media ids
    /// within `[from, to]` (unix seconds), in one batched request. Used to
    /// build a "latest episodes" feed for a saved library without one
    /// request per anime.
    pub async fn get_recent_airing_episodes(
        &self,
        media_ids: &[i64],
        from: i64,
        to: i64,
    ) -> anyhow::Result<Vec<AiringEntry>> {
        if media_ids.is_empty() {
            return Ok(Vec::new());
        }
        tracing::debug!(?media_ids, from, to, url = ANILIST_URL, "sending AniList airingSchedules request");
        let body = serde_json::json!({
            "query": AIRING_SCHEDULES_QUERY,
            "variables": { "ids": media_ids, "from": from, "to": to, "perPage": 50 }
        });
        let resp = match self.http.post(ANILIST_URL).json(&body).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(%err, "AniList airingSchedules request failed to send");
                return Err(err.into());
            }
        };
        let resp = match resp.error_for_status() {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(%err, "AniList airingSchedules returned an error status");
                return Err(err.into());
            }
        };
        let parsed: GraphQlResponse<AiringSchedulesData> = match resp.json().await {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::error!(%err, "failed to parse AniList airingSchedules response body");
                return Err(err.into());
            }
        };
        tracing::debug!(count = parsed.data.page.airing_schedules.len(), "AniList airingSchedules returned results");
        Ok(parsed.data.page.airing_schedules)
    }
}
