use scraper::{Html, Selector};
use serde::Serialize;

const NYAA_BASE_URL: &str = "https://nyaa.si";

#[derive(Debug, Clone, Serialize)]
pub struct NyaaResult {
    pub title: String,
    pub magnet: String,
    pub torrent_url: String,
    pub view_url: String,
    pub size: String,
    pub seeders: u32,
    pub leechers: u32,
    pub published: String,
}

/// Ground-truth batch/submitter info scraped from a torrent's nyaa.si view
/// page (`/view/{id}`), which the RSS search feed doesn't carry. A release
/// is a batch if its file list contains more than one video file — verified
/// against nyaa.si's real markup: a single-episode release's
/// `.torrent-file-list` has exactly one `<li><i class="fa-file">` entry,
/// while a season batch has one per episode (optionally nested under a
/// `<li><a class="folder">` wrapper).
#[derive(Debug, Clone, Serialize)]
pub struct TorrentDetails {
    pub submitter: String,
    pub is_batch: bool,
    pub file_count: usize,
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

/// Normalizes "smart" Unicode punctuation that AniList titles use into the
/// ASCII form nyaa.si's search tokenizer expects. Empirically verified
/// against nyaa.si's live RSS search: a title containing a curly apostrophe
/// (U+2019, e.g. AniList's "Frieren: Beyond Journey's End") drops from 75
/// matches to 1, because nyaa's tokenizer doesn't recognize it as a word
/// character the way it does the ASCII apostrophe. Curly double quotes,
/// em/en dashes, and colons were verified NOT to need normalization (nyaa's
/// tokenizer already treats them as word separators or ignores them), so
/// they're included here only as a cheap defensive normalization, not
/// because they were observed to break matching.
fn sanitize_query(query: &str) -> String {
    let normalized: String = query
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{FF07}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201F}' | '\u{FF02}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            _ => c,
        })
        .collect();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Rows per page of nyaa.si's HTML search results (fixed by nyaa.si itself).
/// Used to detect the last page: a page returning fewer rows than this
/// means there's nothing more to fetch.
const RESULTS_PER_PAGE: usize = 75;

/// Cap on how many result pages a single search will fetch (up to 375
/// results). Bounded deliberately: some long-running shows have hundreds of
/// scattered releases, and unbounded pagination would mean unbounded
/// requests to nyaa.si for a single search.
const MAX_SEARCH_PAGES: u32 = 5;

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

    /// Searches nyaa.si, paginating through the HTML search results page.
    ///
    /// This used to use nyaa.si's RSS feed, which is simpler to parse but
    /// was verified to silently ignore the `p=` (page) query param and
    /// always return only the newest 75 results — for a long-running show
    /// with hundreds of scattered releases across many fansub groups, that
    /// silently dropped every result older than the most recent 75,
    /// including entire early-season episodes. The HTML search page does
    /// support real pagination (confirmed via its own pagination controls),
    /// so this scrapes that instead.
    pub async fn search(&self, query: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        let sanitized = sanitize_query(query);
        if sanitized != query {
            tracing::debug!(query, sanitized, "sanitized smart punctuation for nyaa.si search");
        }

        let mut all_results = Vec::new();
        for page in 1..=MAX_SEARCH_PAGES {
            let page_results = self.search_page(&sanitized, category, page).await?;
            let page_count = page_results.len();
            all_results.extend(page_results);
            if page_count < RESULTS_PER_PAGE {
                break;
            }
        }

        tracing::debug!(query = sanitized, count = all_results.len(), "nyaa.si search returned results");
        Ok(all_results)
    }

    async fn search_page(&self, sanitized_query: &str, category: Category, page: u32) -> anyhow::Result<Vec<NyaaResult>> {
        let url = format!(
            "{NYAA_BASE_URL}/?f=0&c={}&q={}&p={page}",
            category.code(),
            urlencoding::encode(sanitized_query)
        );
        tracing::debug!(query = sanitized_query, url, page, "fetching nyaa.si search page");

        let body = match self.http.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(err) => {
                    tracing::error!(query = sanitized_query, page, %err, "failed to read nyaa.si search page body");
                    return Err(err.into());
                }
            },
            Err(err) => {
                tracing::error!(query = sanitized_query, page, %err, "nyaa.si search page request failed to send");
                return Err(err.into());
            }
        };

        let document = Html::parse_document(&body);

        // Unwraps are safe: these are fixed, compile-time-valid CSS selectors.
        let row_selector = Selector::parse("table.torrent-list tbody tr").unwrap();
        let title_link_selector = Selector::parse("td[colspan='2'] > a:not(.comments)").unwrap();
        let magnet_selector = Selector::parse("a[href^='magnet:']").unwrap();
        let download_selector = Selector::parse("a[href^='/download/']").unwrap();
        let text_center_selector = Selector::parse("td.text-center").unwrap();

        let mut results = Vec::new();
        for row in document.select(&row_selector) {
            let Some(title_el) = row.select(&title_link_selector).next() else {
                continue;
            };
            let Some(view_href) = title_el.value().attr("href") else {
                continue;
            };
            let title = title_el.text().collect::<String>();
            let view_url = format!("{NYAA_BASE_URL}{view_href}");

            let magnet = row
                .select(&magnet_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            let torrent_url = row
                .select(&download_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .map(|href| format!("{NYAA_BASE_URL}{href}"))
                .unwrap_or_default();

            // Order fixed by nyaa.si's table markup: [magnet/download
            // icons, size, date, seeders, leechers, downloads].
            let text_cells: Vec<_> = row.select(&text_center_selector).collect();
            let cell_text = |i: usize| text_cells.get(i).map(|el| el.text().collect::<String>().trim().to_string());
            let size = cell_text(1).unwrap_or_default();
            let published = cell_text(2).unwrap_or_default();
            let seeders = cell_text(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let leechers = cell_text(4).and_then(|s| s.parse().ok()).unwrap_or(0);

            results.push(NyaaResult {
                title,
                magnet,
                torrent_url,
                view_url,
                size,
                seeders,
                leechers,
                published,
            });
        }

        tracing::debug!(query = sanitized_query, page, count = results.len(), "parsed nyaa.si search page");
        Ok(results)
    }

    /// Fetches and parses a torrent's view page to determine its real
    /// submitter and batch status (see `TorrentDetails` doc comment).
    pub async fn fetch_details(&self, view_url: &str) -> anyhow::Result<TorrentDetails> {
        tracing::debug!(view_url, "fetching nyaa.si torrent view page");

        let body = match self.http.get(view_url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(err) => {
                    tracing::error!(view_url, %err, "failed to read nyaa.si view page body");
                    return Err(err.into());
                }
            },
            Err(err) => {
                tracing::error!(view_url, %err, "nyaa.si view page request failed to send");
                return Err(err.into());
            }
        };

        let document = Html::parse_document(&body);

        // Unwraps are safe: these are fixed, compile-time-valid CSS selectors.
        let submitter_selector = Selector::parse("div.panel-body a[href^='/user/']").unwrap();
        let file_selector = Selector::parse(".torrent-file-list i.fa-file").unwrap();

        let submitter = document
            .select(&submitter_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Anonymous".to_string());

        let file_count = document.select(&file_selector).count();
        let is_batch = file_count > 1;

        tracing::debug!(view_url, submitter, file_count, is_batch, "parsed nyaa.si view page");
        Ok(TorrentDetails {
            submitter,
            is_batch,
            file_count,
        })
    }
}
