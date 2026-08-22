use serde::Serialize;

const NYAA_BASE_URL: &str = "https://nyaa.si";

#[derive(Debug, Clone, Serialize)]
pub struct NyaaResult {
    pub title: String,
    pub magnet: String,
    pub torrent_url: String,
    pub size: String,
    pub seeders: u32,
    pub leechers: u32,
    pub published: String,
}

/// nyaa.si's search categories, passed as the `c` query param.
#[derive(Debug, Clone, Copy)]
pub enum Category {
    AnimeEnglishTranslated,
    AnimeRaw,
    AllAnime,
}

impl Category {
    fn code(self) -> &'static str {
        match self {
            Category::AnimeEnglishTranslated => "1_2",
            Category::AnimeRaw => "1_4",
            Category::AllAnime => "1_0",
        }
    }
}

pub struct NyaaClient {
    http: reqwest::Client,
}

impl Default for NyaaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NyaaClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Searches nyaa.si via its RSS feed, which is far more stable than
    /// scraping the HTML search page.
    pub async fn search(&self, query: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        let url = format!(
            "{NYAA_BASE_URL}/?page=rss&c={}&f=0&q={}",
            category.code(),
            urlencoding::encode(query)
        );
        let body = self.http.get(&url).send().await?.bytes().await?;
        let channel = rss::Channel::read_from(&body[..])?;

        let results = channel
            .items()
            .iter()
            .filter_map(|item| {
                let title = item.title()?.to_string();
                let extensions = item.extensions().get("nyaa")?;
                let get_ext = |name: &str| -> Option<String> {
                    extensions
                        .get(name)?
                        .first()
                        .and_then(|e| e.value().map(|v| v.to_string()))
                };
                let magnet = get_ext("magnetUrl").unwrap_or_default();
                let size = get_ext("size").unwrap_or_default();
                let seeders = get_ext("seeders").and_then(|s| s.parse().ok()).unwrap_or(0);
                let leechers = get_ext("leechers").and_then(|s| s.parse().ok()).unwrap_or(0);
                let torrent_url = item.enclosure().map(|e| e.url().to_string()).unwrap_or_default();
                let published = item.pub_date().unwrap_or_default().to_string();

                Some(NyaaResult {
                    title,
                    magnet,
                    torrent_url,
                    size,
                    seeders,
                    leechers,
                    published,
                })
            })
            .collect();

        Ok(results)
    }
}
