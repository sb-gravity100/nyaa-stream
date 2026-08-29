use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anilist_client::{AiringEntry, AniListClient, AnimeMedia, AnimeTitle};
use base64::Engine;
use kitsu_client::{KitsuClient, KitsuMetadata};
use mpv_ipc::MpvPlayer;
use nyaa_client::{Category, NyaaClient, NyaaResult, TorrentDetails};
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use torrent_engine::{StreamStats, TorrentEngine, TorrentId};

struct AppState {
    anilist: AniListClient,
    kitsu: KitsuClient,
    nyaa: NyaaClient,
    torrent_engine: TorrentEngine,
    player: Mutex<Option<MpvPlayer>>,
    /// The torrent currently backing `player`, if any - tracked separately
    /// so `stop_playback` can remove it from the session (stop seeding,
    /// drop partial files) once the user is done, mirroring how the
    /// thumbnail-capture path already cleans up its own scratch torrents.
    current_torrent: Mutex<Option<TorrentId>>,
    thumbnail_cache_dir: PathBuf,
}

#[tauri::command]
async fn search_anime(state: State<'_, Arc<AppState>>, query: String) -> Result<Vec<AnimeMedia>, String> {
    tracing::debug!(%query, "search_anime invoked");
    match state.anilist.search(&query, 20).await {
        Ok(results) => {
            tracing::info!(%query, count = results.len(), "search_anime succeeded");
            Ok(results)
        }
        Err(err) => {
            tracing::error!(%query, %err, "search_anime failed");
            Err(err.to_string())
        }
    }
}

/// Fetches recent episode airings for a set of saved-library anime ids
/// within `[from, to]` (unix seconds, computed client-side so the
/// "this week + last week" window logic lives in one place).
#[tauri::command]
async fn get_latest_episodes(
    state: State<'_, Arc<AppState>>,
    media_ids: Vec<i64>,
    from: i64,
    to: i64,
) -> Result<Vec<AiringEntry>, String> {
    tracing::debug!(count = media_ids.len(), from, to, "get_latest_episodes invoked");
    match state.anilist.get_recent_airing_episodes(&media_ids, from, to).await {
        Ok(entries) => {
            tracing::info!(count = entries.len(), "get_latest_episodes succeeded");
            Ok(entries)
        }
        Err(err) => {
            tracing::error!(%err, "get_latest_episodes failed");
            Err(err.to_string())
        }
    }
}

/// Fetches full AniList details for one specific anime (description,
/// episode count, etc) — deliberately not part of `search_anime`'s
/// response, since that fetches up to 20 results per keystroke and most are
/// never opened; this is only called once, for whichever anime the user
/// actually selects.
#[tauri::command]
async fn get_anime_details(state: State<'_, Arc<AppState>>, id: i64) -> Result<AnimeMedia, String> {
    tracing::debug!(id, "get_anime_details invoked");
    match state.anilist.get_by_id(id).await {
        Ok(media) => {
            tracing::info!(id, "get_anime_details succeeded");
            Ok(media)
        }
        Err(err) => {
            tracing::error!(id, %err, "get_anime_details failed");
            Err(err.to_string())
        }
    }
}

/// Fetches Kitsu's backdrop banner and per-episode thumbnails for one
/// AniList anime. Used instead of AniList's own `streamingEpisodes`/
/// `coverImage` for this purpose - see kitsu_client::KitsuClient::get_metadata
/// doc comment for why (Kitsu had full episode-thumbnail coverage for a show
/// AniList had none for, and its `coverImage` is a proper wide banner rather
/// than a stretched poster). Returns `Ok(None)` when Kitsu has no mapping
/// for this id, which the frontend must treat as "no backdrop/thumbnails
/// available", not an error.
#[tauri::command]
async fn get_kitsu_metadata(state: State<'_, Arc<AppState>>, anilist_id: i64) -> Result<Option<KitsuMetadata>, String> {
    tracing::debug!(anilist_id, "get_kitsu_metadata invoked");
    match state.kitsu.get_metadata(anilist_id).await {
        Ok(metadata) => {
            tracing::info!(anilist_id, found = metadata.is_some(), "get_kitsu_metadata succeeded");
            Ok(metadata)
        }
        Err(err) => {
            tracing::error!(anilist_id, %err, "get_kitsu_metadata failed");
            Err(err.to_string())
        }
    }
}

/// How far into the episode to seek before grabbing a frame - far enough to
/// usually be past a cold-open/company-logo black frame, still early enough
/// that reaching it (by just letting playback run, see below) only pulls a
/// small amount of data. Not seeked-to via mpv's seek command (which can
/// require data librqbit hasn't prioritized yet for a torrent this fresh);
/// instead we let mpv play from the start and poll its playback position,
/// so we only ever consume what streams past sequentially.
const THUMBNAIL_SEEK_SECONDS: f64 = 8.0;
/// Upper bound on the whole capture attempt (spawn mpv, wait for the swarm
/// to deliver enough data to reach THUMBNAIL_SEEK_SECONDS, screenshot).
/// Deliberately short: this is a best-effort background enhancement for a
/// home-page thumbnail, not something worth blocking on if peers are slow.
const THUMBNAIL_CAPTURE_TIMEOUT: Duration = Duration::from_secs(25);
const THUMBNAIL_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Last-resort thumbnail source, used only when Kitsu and AniList both have
/// no per-episode art for a show (verified live: happens for lower-profile
/// currently-airing anime that simply haven't been backfilled into either
/// database yet). Adds the given single-episode-release magnet, lets mpv
/// play it headlessly just long enough to reach THUMBNAIL_SEEK_SECONDS
/// (downloading only that much, not the whole episode), grabs one frame,
/// then removes the torrent - it only ever existed to produce this one
/// image. Results are cached to disk by `cache_key` (caller-chosen, e.g.
/// `{anilist_id}-{episode}`) so this only runs once per episode ever, not
/// once per app launch. Returns `Ok(None)` rather than `Err` for any
/// failure in the pipeline (mpv missing, torrent add failed, swarm too
/// slow, etc) - this is a nice-to-have, not something that should surface
/// as a user-facing error.
#[tauri::command]
async fn capture_torrent_thumbnail(
    state: State<'_, Arc<AppState>>,
    magnet: String,
    cache_key: String,
) -> Result<Option<String>, String> {
    let cache_path = state.thumbnail_cache_dir.join(format!("{cache_key}.jpg"));

    if let Ok(bytes) = tokio::fs::read(&cache_path).await {
        tracing::debug!(cache_key, "capture_torrent_thumbnail cache hit");
        return Ok(Some(to_data_uri(&bytes)));
    }

    tracing::info!(cache_key, "capture_torrent_thumbnail cache miss, capturing from torrent");
    match capture_thumbnail_uncached(&state, &magnet, &cache_path).await {
        Ok(bytes) => Ok(Some(to_data_uri(&bytes))),
        Err(err) => {
            tracing::warn!(cache_key, %err, "capture_torrent_thumbnail failed, falling back to no thumbnail");
            Ok(None)
        }
    }
}

fn to_data_uri(jpeg_bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg_bytes)
    )
}

async fn capture_thumbnail_uncached(
    state: &Arc<AppState>,
    magnet: &str,
    cache_path: &std::path::Path,
) -> anyhow::Result<Vec<u8>> {
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let added = state.torrent_engine.add(magnet).await?;
    let stream_url = state.torrent_engine.stream_url(added.id, 0);

    let capture_result = tokio::time::timeout(THUMBNAIL_CAPTURE_TIMEOUT, async {
        let mut player = MpvPlayer::spawn_headless(&stream_url).await?;

        let deadline = tokio::time::Instant::now() + THUMBNAIL_CAPTURE_TIMEOUT;
        loop {
            let position = player.get_time_position().await.unwrap_or(0.0);
            if position >= THUMBNAIL_SEEK_SECONDS {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for playback to reach {THUMBNAIL_SEEK_SECONDS}s");
            }
            tokio::time::sleep(THUMBNAIL_POLL_INTERVAL).await;
        }

        let cache_path_str = cache_path.to_string_lossy().to_string();
        let screenshot_result = player.screenshot_to_file(&cache_path_str).await;
        let _ = player.quit().await;
        screenshot_result
    })
    .await;

    // Always clean up the scratch torrent, regardless of how capture went.
    if let Err(err) = state.torrent_engine.remove(added.id).await {
        tracing::warn!(torrent_id = added.id, %err, "failed to remove scratch thumbnail torrent");
    }

    match capture_result {
        Ok(Ok(())) => Ok(tokio::fs::read(cache_path).await?),
        Ok(Err(err)) => Err(err),
        Err(_) => anyhow::bail!("thumbnail capture timed out after {THUMBNAIL_CAPTURE_TIMEOUT:?}"),
    }
}

#[tauri::command]
async fn search_torrents(state: State<'_, Arc<AppState>>, query: String) -> Result<Vec<NyaaResult>, String> {
    tracing::debug!(%query, "search_torrents invoked");
    match state.nyaa.search(&query, Category::AnimeEnglishTranslated).await {
        Ok(results) => {
            tracing::info!(%query, count = results.len(), "search_torrents succeeded");
            Ok(results)
        }
        Err(err) => {
            tracing::error!(%query, %err, "search_torrents failed");
            Err(err.to_string())
        }
    }
}

/// Minimum result count before we consider a title's search "good enough"
/// and stop trying further candidate queries. Chosen empirically: nyaa.si's
/// own tokenizer is per-word AND matching, so a title that actually has
/// releases will typically return well above this once the query string
/// isn't mangled by unnormalized punctuation (see nyaa_client::sanitize_query).
const MIN_ACCEPTABLE_RESULTS: usize = 3;

/// Searches nyaa.si for an AniList anime, trying the English title first and
/// falling back to the romaji title if the English title returns too few
/// results. This is deterministic (fixed candidate order, fixed threshold),
/// not fuzzy matching. Verified against nyaa.si's live search: fansub groups
/// overwhelmingly use the romaji title (e.g. "Sousou no Frieren") even when
/// AniList's English title exists, so English-only search under-matches.
#[tauri::command]
async fn search_torrents_for_anime(
    state: State<'_, Arc<AppState>>,
    title: AnimeTitle,
) -> Result<Vec<NyaaResult>, String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(english) = &title.english {
        candidates.push(english.clone());
    }
    if let Some(romaji) = &title.romaji {
        if Some(romaji) != title.english.as_ref() {
            candidates.push(romaji.clone());
        }
    }
    if candidates.is_empty() {
        tracing::warn!("search_torrents_for_anime called with no usable title");
        return Ok(Vec::new());
    }

    tracing::debug!(?candidates, "search_torrents_for_anime invoked");

    let mut last_results = Vec::new();
    for (i, candidate) in candidates.iter().enumerate() {
        match state.nyaa.search(candidate, Category::AnimeEnglishTranslated).await {
            Ok(results) => {
                tracing::info!(query = candidate, count = results.len(), attempt = i, "candidate search completed");
                if results.len() >= MIN_ACCEPTABLE_RESULTS || i == candidates.len() - 1 {
                    return Ok(results);
                }
                last_results = results;
            }
            Err(err) => {
                tracing::error!(query = candidate, %err, "candidate search failed");
                if i == candidates.len() - 1 {
                    return Err(err.to_string());
                }
            }
        }
    }
    Ok(last_results)
}

/// Cap on simultaneous view-page fetches. Bounded deliberately: a search can
/// return dozens of results, and firing that many concurrent requests at
/// nyaa.si at once would be both slow to schedule and impolite to their
/// server. 6 was picked as a reasonable balance, not measured.
const DETAIL_FETCH_CONCURRENCY: usize = 6;

/// Fetches batch/submitter details for many torrents concurrently (bounded
/// by DETAIL_FETCH_CONCURRENCY). Returns one `Option<TorrentDetails>` per
/// input URL, in the same order; `None` means that specific view-page fetch
/// failed (logged individually), not that the whole batch failed.
#[tauri::command]
async fn get_torrent_details_batch(
    state: State<'_, Arc<AppState>>,
    view_urls: Vec<String>,
) -> Result<Vec<Option<TorrentDetails>>, String> {
    let total = view_urls.len();
    tracing::debug!(total, "get_torrent_details_batch invoked");

    let app = state.inner().clone();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(DETAIL_FETCH_CONCURRENCY));

    let mut set = tokio::task::JoinSet::new();
    for (idx, url) in view_urls.into_iter().enumerate() {
        let app = app.clone();
        let semaphore = Arc::clone(&semaphore);
        set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.expect("semaphore never closed");
            let outcome = app.nyaa.fetch_details(&url).await;
            if let Err(err) = &outcome {
                tracing::warn!(view_url = url, %err, "failed to fetch torrent details, leaving unenriched");
            }
            (idx, outcome.ok())
        });
    }

    let mut results: Vec<Option<TorrentDetails>> = vec![None; total];
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((idx, details)) => results[idx] = details,
            Err(err) => tracing::error!(%err, "torrent details fetch task panicked"),
        }
    }

    let enriched = results.iter().filter(|r| r.is_some()).count();
    tracing::info!(total, enriched, "get_torrent_details_batch completed");
    Ok(results)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaySession {
    torrent_id: TorrentId,
}

#[tauri::command]
async fn play_magnet(state: State<'_, Arc<AppState>>, magnet: String, title: String) -> Result<PlaySession, String> {
    tracing::debug!(%title, "play_magnet invoked");

    let added = match state.torrent_engine.add(&magnet).await {
        Ok(added) => added,
        Err(err) => {
            tracing::error!(%title, %err, "play_magnet failed to add torrent");
            return Err(err.to_string());
        }
    };
    let stream_url = state.torrent_engine.stream_url(added.id, 0);
    tracing::info!(%title, torrent_id = added.id, %stream_url, "torrent added, spawning mpv");

    let player = match MpvPlayer::spawn(&stream_url, &title).await {
        Ok(player) => player,
        Err(err) => {
            tracing::error!(%title, %err, "play_magnet failed to spawn mpv");
            let _ = state.torrent_engine.remove(added.id).await;
            return Err(err.to_string());
        }
    };
    *state.player.lock().await = Some(player);
    *state.current_torrent.lock().await = Some(added.id);
    tracing::info!(%title, "mpv spawned and playing");
    Ok(PlaySession { torrent_id: added.id })
}

/// Download progress/speed/peer-count snapshot for the currently-playing
/// torrent, polled by the frontend to show buffering feedback (mirrors
/// Stremio's streaming-server statistics endpoint - see
/// `torrent_engine::StreamStats` doc comment).
#[tauri::command]
async fn get_stream_stats(state: State<'_, Arc<AppState>>, torrent_id: TorrentId) -> Result<StreamStats, String> {
    state.torrent_engine.stats(torrent_id).map_err(|err| err.to_string())
}

#[tauri::command]
async fn set_pause(state: State<'_, Arc<AppState>>, paused: bool) -> Result<(), String> {
    tracing::debug!(paused, "set_pause invoked");
    let guard = state.player.lock().await;
    let Some(player) = guard.as_ref() else {
        tracing::warn!("set_pause called with no active player");
        return Ok(());
    };
    if let Err(err) = player.set_pause(paused).await {
        tracing::error!(paused, %err, "set_pause failed");
        return Err(err.to_string());
    }
    Ok(())
}

/// Stops the active mpv instance and removes its backing torrent (stop
/// seeding, drop partial files) - called when the user closes the
/// in-app stats/stop overlay. Also called defensively before starting a
/// new stream in case the previous one wasn't cleanly stopped.
#[tauri::command]
async fn stop_playback(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    tracing::debug!("stop_playback invoked");
    if let Some(mut player) = state.player.lock().await.take() {
        if let Err(err) = player.quit().await {
            tracing::warn!(%err, "stop_playback: mpv quit failed, killing process");
            let _ = player.kill().await;
        }
    }
    if let Some(torrent_id) = state.current_torrent.lock().await.take() {
        if let Err(err) = state.torrent_engine.remove(torrent_id).await {
            tracing::warn!(torrent_id, %err, "stop_playback: failed to remove torrent");
        }
    }
    tracing::info!("stop_playback completed");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "nyaa_stream_lib=debug,anilist_client=debug,nyaa_client=debug,torrent_engine=debug,mpv_ipc=debug,info"
                .into()
        }))
        .init();

    let app_state = tauri::async_runtime::block_on(async {
        tracing::info!("starting nyaa-stream");
        let download_dir = dirs::download_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("nyaa-stream");
        tracing::debug!(?download_dir, "torrent download dir");
        let torrent_engine = TorrentEngine::start(download_dir).await.unwrap_or_else(|err| {
            tracing::error!(%err, "failed to start torrent engine");
            panic!("failed to start torrent engine: {err}");
        });
        tracing::info!("torrent engine started");

        let thumbnail_cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("nyaa-stream")
            .join("thumbnails");
        tracing::debug!(?thumbnail_cache_dir, "torrent thumbnail cache dir");

        Arc::new(AppState {
            anilist: AniListClient::new(),
            kitsu: KitsuClient::new(),
            nyaa: NyaaClient::new(),
            torrent_engine,
            player: Mutex::new(None),
            current_torrent: Mutex::new(None),
            thumbnail_cache_dir,
        })
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            search_anime,
            get_anime_details,
            get_latest_episodes,
            get_kitsu_metadata,
            capture_torrent_thumbnail,
            search_torrents,
            search_torrents_for_anime,
            get_torrent_details_batch,
            play_magnet,
            get_stream_stats,
            set_pause,
            stop_playback
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
