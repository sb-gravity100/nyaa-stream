use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

mod label;
mod batch_check;
mod cache;
mod fansubbers;
mod file_name;
mod files;
mod peer_season;
pub mod stem;
mod store;
mod throttle;
pub mod sevenz_stream;
pub mod tosho_pipeline;
pub mod tosho;
pub mod tosho_build;
use cache::DiskCache;
use store::Store;
use throttle::Throttle;

pub use batch_check::{check_batch, BatchCheck, BatchIssue};
pub use fansubbers::group_tag;
pub use files::TorrentFile;
pub use label::EpisodeLabel;
pub use peer_season::{peer_seasons, PeerRelease};
pub use stem::stem_of;
pub use store::{release_id, write_release_database, AnidbTvdb, ImportStats, ToshoImportStats, ToshoInfo, ToshoRelease};

const NYAA_BASE_URL: &str = "https://nyaa.si";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyaaResult {
    pub title: String,
    pub magnet: String,
    pub torrent_url: String,
    pub view_url: String,
    pub size: String,
    pub seeders: u32,
    pub leechers: u32,
    pub published: String,
    /// What the title says it holds (episode / batch / movie / ...), parsed
    /// once here so the frontend doesn't re-parse titles.
    #[serde(default)]
    pub label: EpisodeLabel,
    /// AnimeTosho's AniDB episode id (`eid`) when the dataset knows the
    /// release: releases sharing it are the same episode. Filled by the
    /// show search commands, `None` otherwise.
    #[serde(default)]
    pub episode_group: Option<u32>,
    /// `Some(true)` when the dataset's trusted AniDB id (`approved` 1-3)
    /// put the release on the show it was searched for: the frontend's
    /// season filters then do not second-guess it (its title may name
    /// another season's number, e.g. Erai-raws' "2nd Season" for a split
    /// cour). `None` otherwise.
    #[serde(default)]
    pub aid_trusted: Option<bool>,
    /// Ambiguous unmarked season label, kept for diagnostics and legacy
    /// result paths; the current title matcher rejects these by default.
    #[serde(default)]
    pub season_ambiguous: Option<bool>,
}

/// Ground-truth batch/submitter info scraped from a torrent's nyaa.si view
/// page (`/view/{id}`), which the RSS search feed doesn't carry. A release
/// is a batch if its file list contains more than one video file — verified
/// against nyaa.si's real markup: a single-episode release's
/// `.torrent-file-list` has exactly one `<li><i class="fa-file">` entry,
/// while a season batch has one per episode (optionally nested under a
/// `<li><a class="folder">` wrapper).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentDetails {
    pub submitter: String,
    pub is_batch: bool,
    pub file_count: usize,
    /// Every file in the torrent, in listed order, with per-video season and
    /// episode (see `files`). Empty for details cached before this existed.
    #[serde(default)]
    pub files: Vec<TorrentFile>,
    /// Union of the video files' seasons, ascending.
    #[serde(default)]
    pub seasons: Vec<u32>,
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

/// `fetched` (fresh) followed by whatever of `stored` it doesn't repeat,
/// newest release id first - nyaa.si's own order.
fn merge_newest_first(fetched: Vec<NyaaResult>, stored: Vec<NyaaResult>) -> Vec<NyaaResult> {
    let mut seen: std::collections::HashSet<i64> = fetched.iter().filter_map(|r| release_id(&r.view_url)).collect();
    let mut merged = fetched;
    merged.extend(stored.into_iter().filter(|r| release_id(&r.view_url).is_none_or(|id| seen.insert(id))));
    merged.sort_by_key(|r| std::cmp::Reverse(release_id(&r.view_url).unwrap_or(0)));
    merged
}

/// The path a search runs under: the whole site, or one user's uploads.
fn user_path(user: Option<&str>) -> String {
    match user {
        Some(user) => format!("/user/{}", urlencoding::encode(user)),
        None => "/".to_string(),
    }
}

/// Sort sent with every search page: most seeded first. nyaa.si returns at
/// most 1,000 results per query, so newest first (its default) never reached a
/// long show's early seasons.
const SEARCH_SORT: &str = "s=seeders&o=desc";

/// Suffix of the search cache / database key for `SEARCH_SORT`: answers
/// fetched in another order must not be replayed as this one.
const SORT_KEY: &str = "seeders";

/// One search result page of nyaa.si, sorted by `SEARCH_SORT`.
fn search_url(user: Option<&str>, sanitized_query: &str, category: Category, page: u32) -> String {
    format!(
        "{NYAA_BASE_URL}{}?f=0&c={}&q={}&{SEARCH_SORT}&p={page}",
        user_path(user),
        category.code(),
        urlencoding::encode(sanitized_query)
    )
}

/// Rows per page of nyaa.si's HTML search results (fixed by nyaa.si itself).
/// Used to detect the last page: a page returning fewer rows than this
/// means there's nothing more to fetch.
const RESULTS_PER_PAGE: usize = 75;

/// Cap on how many result pages a single search will fetch (up to 1500
/// results). Bounded deliberately rather than unbounded: still avoids truly
/// unbounded requests to nyaa.si for a pathological query. The loop below
/// already stops as soon as a page comes back short, so this only costs
/// anything on searches that actually need it - and the old cap of 5 (375
/// results) was verified live to be far too low: nyaa.si returns 949 total
/// results for "That Time I Got Reincarnated as a Slime" (4 seasons, movies,
/// specials, years of overlapping fansub groups), so the old cap silently
/// dropped roughly 60% of that show's real sources, including most of its
/// earlier seasons.
const MAX_SEARCH_PAGES: u32 = 20;

pub struct NyaaClient {
    http: reqwest::Client,
    /// See `cache` - None for an uncached client (dev tools).
    cache: Option<DiskCache>,
    store: Option<Store>,
    throttle: std::sync::Arc<Throttle>,
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
            cache: None,
            store: None,
            throttle: std::sync::Arc::new(Throttle::new()),
        }
    }

    /// A client that caches responses under `dir` and falls back to them
    /// when nyaa.si fails or rate-limits (see `cache`).
    pub fn with_cache(dir: std::path::PathBuf) -> Self {
        Self {
            http: reqwest::Client::new(),
            cache: Some(DiskCache::new(dir)),
            store: None,
            throttle: std::sync::Arc::new(Throttle::new()),
        }
    }

    /// A client with the disk cache under `cache_dir` plus the permanent
    /// release database at `db_path` (see `store`). If the database can't be
    /// opened the client still works from the disk cache alone.
    pub fn with_storage(cache_dir: std::path::PathBuf, db_path: &std::path::Path) -> Self {
        let mut client = Self::with_cache(cache_dir);
        match Store::open(db_path) {
            Ok(store) => client.store = Some(store),
            Err(err) => tracing::error!(%err, path = %db_path.display(), "couldn't open the nyaa release database"),
        }
        client
    }

    /// Imports a nyaa.si release database file (see `Store::import`).
    pub async fn import_database(&self, path: &std::path::Path) -> anyhow::Result<ImportStats> {
        let Some(store) = &self.store else { anyhow::bail!("the release database isn't open") };
        tracing::info!(path = %path.display(), "importing nyaa release database");
        let stats = store.import(path).await?;
        tracing::info!(?stats, "nyaa release database imported");
        Ok(stats)
    }

    /// Imports an AnimeTosho dataset file (see `Store::import_tosho`).
    pub async fn import_tosho(&self, path: &std::path::Path) -> anyhow::Result<ToshoImportStats> {
        let Some(store) = &self.store else { anyhow::bail!("the release database isn't open") };
        tracing::info!(path = %path.display(), "importing AnimeTosho dataset");
        let stats = store.import_tosho(path).await.inspect_err(|err| tracing::error!(path = %path.display(), %err, "AnimeTosho dataset import failed"))?;
        tracing::info!(?stats, "AnimeTosho dataset imported");
        Ok(stats)
    }

    /// Date and counts of the imported AnimeTosho dataset; `None` before an import.
    pub async fn tosho_info(&self) -> Option<ToshoInfo> {
        let store = self.store.as_ref()?;
        store.tosho_info().await.unwrap_or_else(|err| {
            tracing::warn!(%err, "reading the AnimeTosho dataset info failed");
            None
        })
    }

    /// AnimeTosho's AniDB resolution of nyaa.si release `nyaa_id` (see `Store::aid_of`).
    pub async fn aid_of(&self, nyaa_id: i64) -> Option<ToshoRelease> {
        let store = self.store.as_ref()?;
        store.aid_of(nyaa_id).await.unwrap_or_else(|err| {
            tracing::warn!(nyaa_id, %err, "AniDB lookup failed");
            None
        })
    }

    /// `aid_of` for many releases (unknown ids are absent from the map).
    pub async fn aids_of(&self, nyaa_ids: &[i64]) -> std::collections::HashMap<i64, ToshoRelease> {
        let Some(store) = &self.store else { return Default::default() };
        store.aids_of(nyaa_ids).await.unwrap_or_else(|err| {
            tracing::warn!(count = nyaa_ids.len(), %err, "AniDB batch lookup failed");
            Default::default()
        })
    }

    /// The AniDB id the static stem mapper gives `stem` (see `stem::stem_of`);
    /// `None` when the stem is unknown or no dataset with stems is imported.
    pub async fn aid_for_stem(&self, stem: &str) -> Option<u32> {
        let store = self.store.as_ref()?;
        store.aid_for_stem(stem).await.unwrap_or_else(|err| {
            tracing::warn!(stem, %err, "stem lookup failed");
            None
        })
    }

    /// `aid_for_stem` for many stems (unknown stems are absent from the map).
    pub async fn aids_for_stems(&self, stems: &[String]) -> std::collections::HashMap<String, u32> {
        let Some(store) = &self.store else { return Default::default() };
        store.aids_for_stems(stems).await.unwrap_or_else(|err| {
            tracing::warn!(count = stems.len(), %err, "stem batch lookup failed");
            Default::default()
        })
    }

    /// The dataset's file list of nyaa.si release `nyaa_id` (multi-file
    /// releases only; empty when unknown).
    pub async fn release_files(&self, nyaa_id: i64) -> Vec<TorrentFile> {
        let Some(store) = &self.store else { return Vec::new() };
        store.release_files(nyaa_id).await.unwrap_or_else(|err| {
            tracing::warn!(nyaa_id, %err, "reading the dataset file list failed");
            Vec::new()
        })
    }

    /// The dataset's AniDB -> TVDB mapping of anime `aid`.
    pub async fn anidb_tvdb(&self, aid: u32) -> Option<AnidbTvdb> {
        let store = self.store.as_ref()?;
        store.anidb_tvdb(aid).await.unwrap_or_else(|err| {
            tracing::warn!(aid, %err, "AniDB -> TVDB lookup failed");
            None
        })
    }

    /// Releases already in the local database whose titles contain every word
    /// of `query` - instant, no request. Empty without a database.
    pub async fn search_local(&self, query: &str, category: Category) -> Vec<NyaaResult> {
        let Some(store) = &self.store else { return Vec::new() };
        match store.search_local(category.code(), &sanitize_query(query), 1500).await {
            Ok(results) => {
                tracing::debug!(query, count = results.len(), "local release search");
                results
            }
            Err(err) => {
                tracing::warn!(query, %err, "local release search failed");
                Vec::new()
            }
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
        let results = self.search_scoped(None, query, category).await?;
        self.record_groups(&results).await;
        Ok(results)
    }

    /// Like `search`, but only inside one uploader's releases
    /// (`/user/<user>?q=`).
    pub async fn search_user(&self, user: &str, query: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        self.search_scoped(Some(user), query, category).await
    }

    async fn search_scoped(&self, user: Option<&str>, query: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        let sanitized = sanitize_query(query);
        if sanitized != query {
            tracing::debug!(query, sanitized, "sanitized smart punctuation for nyaa.si search");
        }
        let key = match user {
            Some(user) => format!("{}|user:{}|{sanitized}|{SORT_KEY}", category.code(), user.to_lowercase()),
            None => format!("{}|{sanitized}|{SORT_KEY}", category.code()),
        };
        if let Some(store) = &self.store {
            return self.search_with_store(store, &key, user, &sanitized, category).await;
        }
        let Some(cache) = &self.cache else {
            return self.search_uncached(user, &sanitized, category).await;
        };
        let cached = cache.get::<Vec<NyaaResult>>("search", &key).await;
        if let Some(hit) = &cached {
            if hit.age < cache::SEARCH_FRESH_FOR {
                tracing::debug!(query = sanitized, age_s = hit.age.as_secs(), count = hit.value.len(), "nyaa.si search served from cache");
                return Ok(hit.value.clone());
            }
        }
        match self.search_uncached(user, &sanitized, category).await {
            Ok(results) => {
                cache.put("search", &key, &results).await;
                Ok(results)
            }
            Err(err) => match cached {
                Some(stale) => {
                    tracing::warn!(query = sanitized, %err, age_s = stale.age.as_secs(), "nyaa.si search failed, serving cached results");
                    Ok(stale.value)
                }
                None => Err(err),
            },
        }
    }

    /// The database-backed search: replay a fresh stored answer; otherwise
    /// fetch every page (the results are seeders-descending, so a page of
    /// already stored releases says nothing about the pages after it), merge
    /// with the stored answer, save; and if the fetch fails fall back to the
    /// stored answer or a local word match.
    async fn search_with_store(&self, store: &Store, key: &str, user: Option<&str>, sanitized: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        let prior = store.search(key).await.unwrap_or_else(|err| {
            tracing::warn!(%err, "reading the stored search failed");
            None
        });
        if let Some(prior) = &prior {
            if prior.age < cache::SEARCH_FRESH_FOR {
                tracing::debug!(query = sanitized, age_s = prior.age.as_secs(), count = prior.results.len(), "nyaa.si search served from the local database");
                return Ok(prior.results.clone());
            }
        }
        let prior_results = prior.map(|p| p.results);
        match self.search_uncached(user, sanitized, category).await {
            Ok(fetched) => {
                let merged = merge_newest_first(fetched, prior_results.unwrap_or_default());
                if let Err(err) = store.save_search(key, category.code(), &merged).await {
                    tracing::error!(%err, "saving the search to the local database failed");
                }
                Ok(merged)
            }
            Err(err) => {
                if let Some(prior) = prior_results {
                    tracing::warn!(query = sanitized, %err, count = prior.len(), "nyaa.si search failed, serving the stored answer");
                    return Ok(prior);
                }
                let local = self.search_local(sanitized, category).await;
                if local.is_empty() {
                    return Err(err);
                }
                tracing::warn!(query = sanitized, %err, count = local.len(), "nyaa.si search failed, serving a local word match");
                Ok(local)
            }
        }
    }

    /// Fetches result pages until a short one or `MAX_SEARCH_PAGES`. No early
    /// stop at already stored releases: that only works newest first, and the
    /// pages are seeders-descending (PLAN.md "Torrent source" -> Sort).
    async fn search_uncached(&self, user: Option<&str>, sanitized: &str, category: Category) -> anyhow::Result<Vec<NyaaResult>> {
        let sanitized = sanitized.to_string();

        let mut all_results = Vec::new();
        for page in 1..=MAX_SEARCH_PAGES {
            let page_results = self.search_page(user, &sanitized, category, page).await?;
            let page_count = page_results.len();
            all_results.extend(page_results);
            if page_count < RESULTS_PER_PAGE {
                break;
            }
        }

        tracing::debug!(query = sanitized, count = all_results.len(), "nyaa.si search returned results");
        Ok(all_results)
    }

    async fn search_page(&self, user: Option<&str>, sanitized_query: &str, category: Category, page: u32) -> anyhow::Result<Vec<NyaaResult>> {
        let url = search_url(user, sanitized_query, category, page);
        tracing::debug!(query = sanitized_query, url, page, "fetching nyaa.si search page");

        // A rate-limited (429) or failing page parses to zero rows - it must
        // be an error, not "no results", or it would be cached as such.
        let body = match self.throttle.get(&self.http, &url).await {
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
                magnet,
                torrent_url,
                view_url,
                size,
                seeders,
                leechers,
                published,
                episode_group: None,
                aid_trusted: None,
                season_ambiguous: None,
                label: EpisodeLabel::of(&title),
                title,
            });
        }

        tracing::debug!(query = sanitized_query, page, count = results.len(), "parsed nyaa.si search page");
        Ok(results)
    }

    /// Counts the group tags in `results` for `popular_fansubbers`.
    async fn record_groups(&self, results: &[NyaaResult]) {
        let Some(cache) = &self.cache else { return };
        let mut counts = cache.get::<fansubbers::GroupCounts>("fansubbers", "counts").await.map(|hit| hit.value).unwrap_or_default();
        counts.add(results);
        cache.put("fansubbers", "counts", &counts).await;
    }

    /// Popular fansub group names: a built-in seed list plus the tags seen
    /// most in real searches - for Settings' suggestions.
    pub async fn popular_fansubbers(&self, limit: usize) -> Vec<String> {
        let counts = match &self.cache {
            Some(cache) => cache.get::<fansubbers::GroupCounts>("fansubbers", "counts").await.map(|hit| hit.value).unwrap_or_default(),
            None => fansubbers::GroupCounts::default(),
        };
        counts.popular(limit)
    }

    /// The nyaa.si account behind a group tag, if known: cached, else probed
    /// (`/user/<tag>` existing means the tag *is* the account name). `None`
    /// means unknown - a tag whose uploads sit under another account name is
    /// learned by `learn_fansubber`.
    pub async fn resolve_fansubber(&self, tag: &str) -> Option<String> {
        let key = tag.to_lowercase();
        let cached = match &self.cache {
            Some(cache) => cache.get::<Option<String>>("fansubber-user", &key).await,
            None => None,
        };
        if let Some(hit) = &cached {
            let fresh_for = if hit.value.is_some() { fansubbers::RESOLVED_FRESH_FOR } else { fansubbers::UNRESOLVED_FRESH_FOR };
            if hit.age < fresh_for {
                tracing::debug!(tag, user = ?hit.value, "fansubber account served from cache");
                return hit.value.clone();
            }
        }
        let url = format!("{NYAA_BASE_URL}{}", user_path(Some(tag)));
        let user = match self.throttle.get(&self.http, &url).await {
            Ok(_) => Some(tag.to_string()),
            Err(err) => {
                let status = err.downcast_ref::<reqwest::Error>().and_then(reqwest::Error::status);
                if status != Some(reqwest::StatusCode::NOT_FOUND) {
                    // Rate limited/offline: unknown for now, don't cache it.
                    tracing::warn!(tag, %err, "couldn't probe fansubber account");
                    return cached.and_then(|hit| hit.value);
                }
                None
            }
        };
        tracing::info!(tag, ?user, "fansubber account resolved");
        if let Some(cache) = &self.cache {
            cache.put("fansubber-user", &key, &user).await;
        }
        user
    }

    /// Learns the account behind `tag` from one of its releases in
    /// `results` (its view page names the real submitter), for groups whose
    /// tag isn't their account name.
    pub async fn learn_fansubber(&self, tag: &str, results: &[NyaaResult]) {
        let Some(release) = results.iter().find(|r| group_tag(&r.title).is_some_and(|t| t.eq_ignore_ascii_case(tag))) else {
            return;
        };
        match self.fetch_details(&release.view_url).await {
            Ok(details) if details.submitter != "Anonymous" => {
                tracing::info!(tag, user = details.submitter, "learned fansubber account from a release page");
                if let Some(cache) = &self.cache {
                    cache.put("fansubber-user", &tag.to_lowercase(), &Some(details.submitter)).await;
                }
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(tag, %err, "couldn't learn fansubber account"),
        }
    }

    /// Fetches and parses a torrent's view page to determine its real
    /// submitter and batch status (see `TorrentDetails` doc comment).
    pub async fn fetch_details(&self, view_url: &str) -> anyhow::Result<TorrentDetails> {
        // A torrent's submitter and file list never change after upload, so
        // the release database keeps each scrape for good.
        let id = release_id(view_url);
        if let (Some(store), Some(id)) = (&self.store, id) {
            match store.details(id).await {
                Ok(Some(details)) => {
                    tracing::debug!(view_url, id, "nyaa.si torrent details served from the release database");
                    return Ok(details);
                }
                Ok(None) => tracing::debug!(view_url, id, "torrent details not in the release database"),
                Err(err) => tracing::warn!(view_url, %err, "reading stored torrent details failed"),
            }
        }
        if let Some(cache) = &self.cache {
            if let Some(hit) = cache.get::<TorrentDetails>("details", view_url).await {
                // Entries cached before the file list existed have none: refetch.
                if hit.value.file_count == 0 || !hit.value.files.is_empty() {
                    tracing::debug!(view_url, "nyaa.si torrent details served from cache");
                    if let (Some(store), Some(id)) = (&self.store, id) {
                        if let Err(err) = store.save_details(id, &hit.value).await {
                            tracing::warn!(view_url, %err, "storing torrent details failed");
                        }
                    }
                    return Ok(hit.value);
                }
                tracing::debug!(view_url, "cached torrent details predate the file list, refetching");
            }
        }
        // The imported AnimeTosho dataset knows many multi-file releases' file lists:
        // answer from it without a request. Read-only - never written to `details`,
        // so a later scrape (which also knows the submitter) can still replace it.
        if let Some(id) = id {
            if let Some(details) = self.dataset_details(id).await {
                tracing::debug!(view_url, id, files = details.files.len(), "torrent details built from the AnimeTosho dataset");
                return Ok(details);
            }
        }
        let details = self.fetch_details_uncached(view_url).await?;
        if let Some(cache) = &self.cache {
            cache.put("details", view_url, &details).await;
        }
        if let (Some(store), Some(id)) = (&self.store, id) {
            match store.save_details(id, &details).await {
                Ok(()) => tracing::debug!(view_url, id, files = details.files.len(), "torrent details stored"),
                Err(err) => tracing::warn!(view_url, %err, "storing torrent details failed"),
            }
        }
        Ok(details)
    }

    /// `TorrentDetails` built from the dataset's file list of release `id`
    /// (`None` when the dataset has none). The dataset doesn't carry the
    /// submitter, so it reads "Anonymous".
    async fn dataset_details(&self, id: i64) -> Option<TorrentDetails> {
        let files = self.release_files(id).await;
        if files.is_empty() {
            return None;
        }
        let mut seasons: Vec<u32> = files.iter().filter_map(|f| f.season).collect();
        seasons.sort_unstable();
        seasons.dedup();
        let file_count = files.len();
        Some(TorrentDetails { submitter: "Anonymous".to_string(), is_batch: file_count > 1, file_count, files, seasons })
    }

    async fn fetch_details_uncached(&self, view_url: &str) -> anyhow::Result<TorrentDetails> {
        tracing::debug!(view_url, "fetching nyaa.si torrent view page");

        let body = match self.throttle.get(&self.http, view_url).await {
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

        let (files, seasons) = files::build_files(files::scrape_files(&document));
        tracing::debug!(view_url, submitter, file_count, is_batch, listed = files.len(), ?seasons, "parsed nyaa.si view page");
        Ok(TorrentDetails {
            submitter,
            is_batch,
            file_count,
            files,
            seasons,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQL: &str = "INSERT INTO `toto_files` (`id`, `toto_id`, `filename`, `filesize`) VALUES (1,10,'Show/Season 1/Show - 01.mkv',1000),(2,10,'Show/Season 2/Show - 01.mkv',2000),(3,10,'Show/cover.jpg',5);
INSERT INTO `toto_toto` (`id`, `nyaa_id`, `nyaa_subdom`, `name`, `btih`, `totalsize`, `dateline`, `torrentfiles`, `nyaa_cat`, `deleted`, `isdupe`, `aid`, `eid`, `resolveapproved`) VALUES (10,100,'','[G] Show - Batch',0x0102030405060708090a0b0c0d0e0f1011121314,3000,1700000000,3,'1_37',0,0,55,7,3),(12,200,'','[G] Show - Other',0x0102030405060708090a0b0c0d0e0f1011121316,3000,1700000000,3,'1_37',0,0,55,7,3);
";

    #[test]
    fn search_pages_are_sorted_by_seeders() {
        assert_eq!(
            search_url(None, "Tensei shitara Slime Datta Ken", Category::AllAnime, 3),
            "https://nyaa.si/?f=0&c=1_0&q=Tensei%20shitara%20Slime%20Datta%20Ken&s=seeders&o=desc&p=3"
        );
        assert_eq!(
            search_url(Some("Erai-raws"), "slime", Category::AnimeEnglishTranslated, 1),
            "https://nyaa.si/user/Erai-raws?f=0&c=1_2&q=slime&s=seeders&o=desc&p=1"
        );
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("nyaa-client-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[tokio::test]
    async fn details_come_from_the_dataset_without_a_request() {
        let src = temp("details-src.db");
        tosho_build::build(SQL.as_bytes(), &src, "2026-05-09").unwrap();
        let client = NyaaClient::with_storage(std::env::temp_dir().join(format!("nyaa-client-test-cache-{}", std::process::id())), &temp("details-dst.db"));
        client.import_tosho(&src).await.unwrap();

        // Unknown to the store, known to the dataset: built offline.
        let details = client.fetch_details("https://nyaa.si/view/100").await.unwrap();
        assert_eq!((details.is_batch, details.file_count, details.seasons.clone()), (true, 3, vec![1, 2]));
        assert_eq!(details.files[1].path, "Show/Season 2/Show - 01.mkv");
        assert_eq!((details.files[0].season, details.files[0].episode), (Some(1), Some(1)));
        assert_eq!(details.files[2].season, None);
        // Not written to the permanent details table.
        assert!(client.store.as_ref().unwrap().details(100).await.unwrap().is_none());

        // A stored scrape wins over the dataset.
        let scraped = TorrentDetails { submitter: "Reza".into(), is_batch: true, file_count: 2, files: Vec::new(), seasons: vec![3] };
        client.store.as_ref().unwrap().save_details(200, &scraped).await.unwrap();
        let details = client.fetch_details("https://nyaa.si/view/200").await.unwrap();
        assert_eq!((details.submitter.as_str(), details.seasons), ("Reza", vec![3]));
    }
}
