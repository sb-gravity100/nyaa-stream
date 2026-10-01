mod cache;
mod download_cache;
mod media_keys;
mod metadata_fallback;
mod player;
mod resume;
mod title_match;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anilist_client::{AiringEntry, AniListClient, AnimeMedia, AnimeTitle};
use kitsu_client::{KitsuClient, KitsuMetadata};
use mpv_player::MpvPlayer;
use nyaa_client::{Category, NyaaClient, NyaaResult, TorrentDetails};
use serde::Serialize;
use tauri::{Manager, State};
use tokio::sync::Mutex;
use torrent_engine::{largest_video_file, DecoderSupport, StreamStats, SubtitleTrack, TorrentEngine, TorrentFile, TorrentId};

struct AppState {
    anilist: AniListClient,
    kitsu: KitsuClient,
    nyaa: NyaaClient,
    torrent_engine: TorrentEngine,
    /// The torrent currently backing playback, if any - tracked so
    /// `stop_playback` can remove it from the session (stop seeding, drop
    /// partial files) once the user is done, mirroring how the
    /// thumbnail-capture path already cleans up its own scratch torrents.
    /// The player process itself (embedded mpv) is tracked separately, in
    /// `player::PlayerState`.
    current_torrent: Mutex<Option<TorrentId>>,
    /// Bumped by every `stop_playback`/`play_magnet`: a deferred removal only
    /// goes ahead if nothing bumped it since (see `stop_playback`).
    stop_generation: std::sync::atomic::AtomicU64,
    thumbnail_cache_dir: PathBuf,
    /// Set after AniList turns us away - see `metadata_fallback`.
    anilist_cooldown: metadata_fallback::AniListCooldown,
    /// One torrent capture at a time: each adds a scratch torrent and a
    /// headless mpv decode, and a home page full of art-less cards used to
    /// start them all at once.
    thumbnail_captures: tokio::sync::Semaphore,
    /// Played torrents kept in `downloads/` - see `download_cache`.
    download_cache: download_cache::DownloadCache,
}

/// Forwards a frontend `console.*`/`window.onerror`/unhandled-rejection
/// message into the same `tracing` output the backend logs to - the native
/// window has no accessible devtools console during development, so
/// without this, frontend errors (a failed render, a `<video>` element
/// error, a rejected promise) are invisible outside the app itself.
/// Installed once at startup by `src/devLogger.ts`; only active in the real
/// Tauri app, never the browser-only dev preview (which has real devtools).
#[tauri::command]
fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        "info" => tracing::info!(target: "frontend", "{message}"),
        _ => tracing::debug!(target: "frontend", "{message}"),
    }
}

#[tauri::command]
async fn search_anime(state: State<'_, Arc<AppState>>, query: String) -> Result<Vec<AnimeMedia>, String> {
    tracing::debug!(%query, "search_anime invoked");
    if !state.anilist_cooldown.active() {
        match state.anilist.search(&query, 20).await {
            Ok(results) => {
                tracing::info!(%query, count = results.len(), "search_anime succeeded");
                return Ok(results);
            }
            Err(err) if metadata_fallback::anilist_unavailable(&err) => {
                tracing::warn!(%query, %err, "search_anime: AniList unavailable, falling back to Kitsu");
                state.anilist_cooldown.start();
            }
            Err(err) => {
                tracing::error!(%query, %err, "search_anime failed");
                return Err(err.to_string());
            }
        }
    }
    match state.kitsu.search_anime(&query, 20).await {
        Ok(results) => {
            tracing::info!(%query, count = results.len(), "search_anime succeeded via Kitsu");
            Ok(results.into_iter().map(metadata_fallback::to_media).collect())
        }
        Err(err) => {
            tracing::error!(%query, %err, "search_anime failed on Kitsu too");
            Err(format!("AniList is unavailable and Kitsu failed too: {err}"))
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
    if !state.anilist_cooldown.active() {
        match state.anilist.get_by_id(id).await {
            Ok(media) => {
                tracing::info!(id, "get_anime_details succeeded");
                return Ok(media);
            }
            Err(err) if metadata_fallback::anilist_unavailable(&err) => {
                tracing::warn!(id, %err, "get_anime_details: AniList unavailable, falling back to Kitsu");
                state.anilist_cooldown.start();
            }
            Err(err) => {
                tracing::error!(id, %err, "get_anime_details failed");
                return Err(err.to_string());
            }
        }
    }
    match state.kitsu.anime_by_anilist_id(id).await {
        Ok(Some(anime)) => {
            tracing::info!(id, "get_anime_details succeeded via Kitsu");
            Ok(metadata_fallback::to_media(anime))
        }
        Ok(None) => {
            tracing::warn!(id, "get_anime_details: Kitsu has no mapping for this AniList id");
            Err("AniList is unavailable and Kitsu doesn't know this anime".to_string())
        }
        Err(err) => {
            tracing::error!(id, %err, "get_anime_details failed on Kitsu too");
            Err(format!("AniList is unavailable and Kitsu failed too: {err}"))
        }
    }
}

/// Cumulative episode count of every prior season in the franchise, per
/// AniList's relations graph - see `anilist_client::AniListClient::
/// cumulative_prequel_episodes`'s doc comment for the full explanation.
/// Used to recognize when a release numbers an episode absolutely across
/// the whole franchise (e.g. "Season 4 Episode 92") instead of relative to
/// the season being browsed, and correct it (episodeParser.ts can't do
/// this from title text alone - it has no notion of a franchise's other
/// seasons). Returns 0 (not an error) if AniList's relations graph has
/// nothing useful here - same as "no prior seasons found", the correction
/// step just won't have anything to do for this anime.
#[tauri::command]
async fn get_absolute_episode_offset(state: State<'_, Arc<AppState>>, id: i64) -> Result<i32, String> {
    tracing::debug!(id, "get_absolute_episode_offset invoked");
    match state.anilist.cumulative_prequel_episodes(id).await {
        Ok(offset) => {
            tracing::info!(id, offset, "get_absolute_episode_offset succeeded");
            Ok(offset)
        }
        Err(err) => {
            tracing::error!(id, %err, "get_absolute_episode_offset failed");
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
    // Disk cache: episode thumbnails and backdrops belong to a fixed
    // episode/show, so a day-old copy is served without asking Kitsu (new
    // episodes gaining thumbnails is the only reason to refetch at all), and
    // any copy is served when Kitsu fails.
    let cache_path = state.thumbnail_cache_dir.join("kitsu").join(format!("{anilist_id}.json"));
    let cached: Option<(Option<KitsuMetadata>, std::time::Duration)> = async {
        let modified = tokio::fs::metadata(&cache_path).await.ok()?.modified().ok()?;
        let bytes = tokio::fs::read(&cache_path).await.ok()?;
        Some((serde_json::from_slice(&bytes).ok()?, modified.elapsed().unwrap_or_default()))
    }
    .await;
    if let Some((metadata, age)) = &cached {
        if *age < KITSU_CACHE_FRESH_FOR {
            tracing::debug!(anilist_id, age_s = age.as_secs(), "get_kitsu_metadata served from cache");
            return Ok(metadata.clone());
        }
    }
    match state.kitsu.get_metadata(anilist_id).await {
        Ok(metadata) => {
            tracing::info!(anilist_id, found = metadata.is_some(), "get_kitsu_metadata succeeded");
            let write = async {
                tokio::fs::create_dir_all(cache_path.parent().expect("has parent")).await?;
                tokio::fs::write(&cache_path, serde_json::to_vec(&metadata)?).await?;
                anyhow::Ok(())
            };
            if let Err(err) = write.await {
                tracing::warn!(anilist_id, %err, "failed to cache kitsu metadata");
            }
            Ok(metadata)
        }
        Err(err) => match cached {
            Some((metadata, age)) => {
                tracing::warn!(anilist_id, %err, age_s = age.as_secs(), "get_kitsu_metadata failed, serving cached copy");
                Ok(metadata)
            }
            None => {
                tracing::error!(anilist_id, %err, "get_kitsu_metadata failed");
                Err(err.to_string())
            }
        },
    }
}

/// How long cached Kitsu metadata is used without refetching.
const KITSU_CACHE_FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// URI scheme serving cached thumbnail JPEGs straight to `<img>` tags (see
/// `thumbnail_protocol`), so no image bytes ever cross IPC as base64.
const THUMBNAIL_SCHEME: &str = "thumb";
/// A failed torrent capture isn't retried for this long - each attempt
/// costs a nyaa search, a scratch torrent and up to
/// THUMBNAIL_CAPTURE_TIMEOUT of mpv decoding.
const THUMBNAIL_FAILURE_RETRY_AFTER: Duration = Duration::from_secs(24 * 3600);

/// `{anilistId}-{episode}` only: the key becomes a file name.
fn valid_thumbnail_key(cache_key: &str) -> bool {
    !cache_key.is_empty() && cache_key.chars().all(|c| c.is_ascii_digit() || c == '-')
}

fn thumbnail_path(state: &AppState, cache_key: &str) -> PathBuf {
    state.thumbnail_cache_dir.join(format!("{cache_key}.jpg"))
}

fn thumbnail_failure_path(state: &AppState, cache_key: &str) -> PathBuf {
    state.thumbnail_cache_dir.join(format!("{cache_key}.fail"))
}

/// The `thumb` URL for a cached thumbnail, or None if there isn't one. The
/// file's mtime is the version, so a re-saved frame gets a new URL and the
/// webview can cache every URL forever.
async fn thumbnail_url(path: &std::path::Path, cache_key: &str) -> Option<String> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let version = modified.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    // WebView2 (and Android) only load custom schemes in this form.
    let base = if cfg!(any(windows, target_os = "android")) {
        format!("http://{THUMBNAIL_SCHEME}.localhost")
    } else {
        format!("{THUMBNAIL_SCHEME}://localhost")
    };
    Some(format!("{base}/{cache_key}.jpg?v={version}"))
}

/// Whether a capture for this key failed within THUMBNAIL_FAILURE_RETRY_AFTER.
async fn thumbnail_failed_recently(state: &AppState, cache_key: &str) -> bool {
    let Ok(meta) = tokio::fs::metadata(thumbnail_failure_path(state, cache_key)).await else {
        return false;
    };
    meta.modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age < THUMBNAIL_FAILURE_RETRY_AFTER)
}

/// Serves `thumb://localhost/{key}.jpg` from the thumbnail cache dir.
fn thumbnail_protocol(state: &AppState, request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};
    let file = request.uri().path().trim_start_matches('/');
    let key = file.strip_suffix(".jpg").unwrap_or("");
    let not_found = || Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap();
    if !valid_thumbnail_key(key) {
        tracing::warn!(path = file, "thumbnail protocol rejected path");
        return not_found();
    }
    match std::fs::read(thumbnail_path(state, key)) {
        Ok(bytes) => {
            tracing::debug!(cache_key = key, bytes = bytes.len(), "thumbnail protocol served");
            Response::builder()
                .header(header::CONTENT_TYPE, "image/jpeg")
                // URLs carry the file's mtime (see thumbnail_url).
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .body(bytes)
                .unwrap()
        }
        Err(err) => {
            tracing::debug!(cache_key = key, %err, "thumbnail protocol miss");
            not_found()
        }
    }
}

/// Saves the player's last shown frame as the episode's thumbnail, in the
/// same cache `capture_torrent_thumbnail` uses - it's where the user left
/// off, and it costs no torrent capture. The body is the raw JPEG (no
/// base64); the cache key comes in the `x-cache-key` header. Returns the
/// new `thumb` URL.
#[tauri::command]
async fn save_frame_thumbnail(state: State<'_, Arc<AppState>>, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let cache_key = request
        .headers()
        .get("x-cache-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if !valid_thumbnail_key(&cache_key) {
        tracing::warn!(cache_key, "save_frame_thumbnail rejected cache key");
        return Err("invalid cache key".to_string());
    }
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        tracing::warn!(cache_key, "save_frame_thumbnail expected a raw body");
        return Err("expected raw JPEG bytes".to_string());
    };
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        tracing::warn!(cache_key, bytes = bytes.len(), "save_frame_thumbnail body is not a JPEG");
        return Err("expected a JPEG".to_string());
    }
    let cache_path = thumbnail_path(&state, &cache_key);
    let tmp = cache_path.with_extension("tmp");
    let write = async {
        tokio::fs::create_dir_all(&state.thumbnail_cache_dir).await?;
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, &cache_path).await
    };
    if let Err(err) = write.await {
        tracing::error!(cache_key, %err, "failed to save player frame thumbnail");
        return Err(err.to_string());
    }
    // A real frame beats any earlier failed capture.
    let _ = tokio::fs::remove_file(thumbnail_failure_path(&state, &cache_key)).await;
    tracing::info!(cache_key, bytes = bytes.len(), "saved last player frame as thumbnail");
    thumbnail_url(&cache_path, &cache_key).await.ok_or_else(|| "saved thumbnail vanished".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedThumbnail {
    url: Option<String>,
    /// A capture failed recently - callers skip the nyaa search a new
    /// capture would need.
    failed_recently: bool,
}

/// A torrent-captured (or saved-frame) thumbnail from the disk cache only -
/// never captures. The frontend asks this first so a cached frame comes
/// back without the nyaa search it needs to pick a release for a capture.
#[tauri::command]
async fn cached_torrent_thumbnail(state: State<'_, Arc<AppState>>, cache_key: String) -> Result<CachedThumbnail, String> {
    if !valid_thumbnail_key(&cache_key) {
        tracing::warn!(cache_key, "cached_torrent_thumbnail rejected cache key");
        return Err("invalid cache key".to_string());
    }
    let url = thumbnail_url(&thumbnail_path(&state, &cache_key), &cache_key).await;
    let failed_recently = url.is_none() && thumbnail_failed_recently(&state, &cache_key).await;
    tracing::debug!(cache_key, hit = url.is_some(), failed_recently, "cached_torrent_thumbnail");
    Ok(CachedThumbnail { url, failed_recently })
}

/// Fallback seek target when there's no duration estimate to compute a real
/// midpoint from (see `capture_thumbnail_uncached`) - far enough to usually
/// be past a cold-open/company-logo black frame, still early enough that
/// reaching it by just letting playback run from 0 (no `--start`, avoiding
/// mpv needing to seek at all) only pulls a small amount of data.
const THUMBNAIL_SEEK_SECONDS: f64 = 8.0;
/// Slack allowed when we *do* have a duration estimate and ask mpv to
/// start already near the midpoint via `--start` (see mpv-player's
/// `spawn_headless` doc comment): without a Matroska Cues index on a
/// still-downloading torrent, that seek can land on the nearest keyframe
/// *before* the exact target rather than exactly on it, so the wait loop
/// below accepts anything within this much of the target instead of
/// waiting for an exact match that might never come.
const THUMBNAIL_SEEK_TOLERANCE_SECONDS: f64 = 5.0;
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
/// play it headlessly just long enough to reach roughly the middle of the
/// episode (a cold-open/black-frame/logo risk at the very start, unlike a
/// frame from partway through - `duration_minutes` is the frontend's
/// AniList-derived estimate, same source as the HLS playlist duration;
/// falls back to a fixed early point if it's unavailable, since waiting
/// for *actual* real-time playback to reach a real midpoint - many minutes
/// - isn't practical within this call's timeout), grabs one frame, then
/// removes the torrent - it only ever existed to produce this one image.
/// Results are cached to disk by `cache_key` (caller-chosen, e.g.
/// `{anilist_id}-{episode}`) so this only runs once per episode ever, not
/// once per app launch. Returns `Ok(None)` rather than `Err` for any
/// failure in the pipeline (mpv missing, torrent add failed, swarm too
/// slow, etc) - this is a nice-to-have, not something that should surface
/// as a user-facing error.
/// Error string `capture_torrent_thumbnail` returns while a stream is
/// playing - `torrentThumbnail.ts` treats it as "try again later".
const THUMBNAIL_DEFERRED: &str = "thumbnail-deferred-playback-active";

#[tauri::command]
async fn capture_torrent_thumbnail(
    state: State<'_, Arc<AppState>>,
    magnet: String,
    cache_key: String,
    duration_minutes: Option<f64>,
) -> Result<Option<String>, String> {
    if !valid_thumbnail_key(&cache_key) {
        tracing::warn!(cache_key, "capture_torrent_thumbnail rejected cache key");
        return Err("invalid cache key".to_string());
    }
    let cache_path = thumbnail_path(&state, &cache_key);

    if let Some(url) = thumbnail_url(&cache_path, &cache_key).await {
        tracing::debug!(cache_key, "capture_torrent_thumbnail cache hit");
        return Ok(Some(url));
    }
    if thumbnail_failed_recently(&state, &cache_key).await {
        tracing::debug!(cache_key, "capture_torrent_thumbnail skipped, failed recently");
        return Ok(None);
    }

    tracing::debug!(cache_key, "capture_torrent_thumbnail waiting for a capture slot");
    let _permit = state.thumbnail_captures.acquire().await.map_err(|err| err.to_string())?;
    // Another request may have produced it while this one waited.
    if let Some(url) = thumbnail_url(&cache_path, &cache_key).await {
        tracing::debug!(cache_key, "capture_torrent_thumbnail filled while queued");
        return Ok(Some(url));
    }

    // A capture adds a second torrent and an mpv decode while the user is
    // watching something - both compete with the stream for bandwidth and
    // CPU (seen live: captures running mid-playback). Defer instead; the
    // frontend doesn't cache this error and retries later.
    if state.current_torrent.lock().await.is_some() {
        tracing::debug!(cache_key, "capture_torrent_thumbnail deferred, playback active");
        return Err(THUMBNAIL_DEFERRED.to_string());
    }
    tracing::info!(cache_key, "capture_torrent_thumbnail cache miss, capturing from torrent");
    match capture_thumbnail_uncached(&state, &magnet, &cache_path, duration_minutes).await {
        Ok(()) => Ok(thumbnail_url(&cache_path, &cache_key).await),
        Err(err) => {
            tracing::warn!(cache_key, %err, "capture_torrent_thumbnail failed, falling back to no thumbnail");
            if let Err(err) = tokio::fs::write(thumbnail_failure_path(&state, &cache_key), b"").await {
                tracing::warn!(cache_key, %err, "failed to record thumbnail capture failure");
            }
            Ok(None)
        }
    }
}

async fn capture_thumbnail_uncached(
    state: &Arc<AppState>,
    magnet: &str,
    cache_path: &std::path::Path,
    duration_minutes: Option<f64>,
) -> anyhow::Result<()> {
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let added = state.torrent_engine.add(magnet).await?;
    // Largest video file rather than index 0: single-episode torrents
    // sometimes lead with a sample/NCOP, and index 0 was never a real
    // choice anyway - just the only one available before file listing.
    let files = state.torrent_engine.files(&added.id).await?;
    let file_idx = largest_video_file(&files).unwrap_or(0);
    let stream_url = state.torrent_engine.stream_url(&added.id, file_idx);

    // Some(_) means "seek there via mpv's --start", None means "no
    // estimate, just play from 0" - see spawn_headless/the wait loop below
    // for why these two cases are handled differently rather than always
    // going through --start with THUMBNAIL_SEEK_SECONDS as its target.
    let target_seconds = duration_minutes.filter(|m| *m > 0.0).map(|minutes| minutes * 60.0 / 2.0);

    let capture_result = tokio::time::timeout(THUMBNAIL_CAPTURE_TIMEOUT, async {
        let mut player = MpvPlayer::spawn_headless(&stream_url, target_seconds).await?;

        let effective_target = target_seconds.unwrap_or(THUMBNAIL_SEEK_SECONDS);
        let deadline = tokio::time::Instant::now() + THUMBNAIL_CAPTURE_TIMEOUT;
        loop {
            let position = player.get_time_position().await.unwrap_or(0.0);
            let reached = match target_seconds {
                Some(_) => position >= effective_target - THUMBNAIL_SEEK_TOLERANCE_SECONDS,
                None => position >= effective_target,
            };
            if reached {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for playback to reach {effective_target}s");
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
    let torrent_id = added.id;
    if let Err(err) = state.torrent_engine.remove(torrent_id.clone()).await {
        tracing::warn!(torrent_id = %torrent_id, %err, "failed to remove scratch thumbnail torrent");
    }
    let app = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FILE_RELEASE_DELAY).await;
        let _ = tokio::task::spawn_blocking(move || app.download_cache.discard_unindexed(&torrent_id, &files)).await;
    });

    match capture_result {
        Ok(Ok(())) => Ok(()),
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

/// Searches nyaa.si for an AniList anime and merges the results
/// (deduplicated by view URL).
///
/// It queries by both the English and romaji titles (plus season-stripped
/// forms and Latin synonyms) unconditionally, not as a fallback: verified
/// live against "That Time I Got Reincarnated as a Slime", many fansub
/// groups (SubsPlease, Erai-raws, Ironclad, ASW, and others) title their
/// releases in romaji only - 489 of that show's 950 real releases were being
/// silently dropped by English-only search. `title_match` decides which
/// queries are worth sending (nyaa.si AND-matches words, so a superset query
/// only costs requests) and afterwards drops results that don't carry any of
/// the show's names. The remaining queries run concurrently, but every
/// request goes through nyaa-client's throttle, which paces them and backs
/// off when nyaa.si rate limits.
#[tauri::command]
async fn search_torrents_for_anime(
    state: State<'_, Arc<AppState>>,
    title: AnimeTitle,
    synonyms: Option<Vec<String>>,
    fansubber: Option<String>,
) -> Result<Vec<NyaaResult>, String> {
    let synonyms = synonyms.unwrap_or_default();
    let candidates = title_match::build_candidates(title.english.as_deref(), title.romaji.as_deref(), &synonyms);
    if candidates.is_empty() {
        tracing::warn!("search_torrents_for_anime called with no usable title");
        return Ok(Vec::new());
    }
    tracing::debug!(?candidates, "search_torrents_for_anime invoked");

    let mut searches = tokio::task::JoinSet::new();
    for (order, candidate) in candidates.into_iter().enumerate() {
        let app = state.inner().clone();
        searches.spawn(async move {
            let outcome = app.nyaa.search(&candidate, Category::AnimeEnglishTranslated).await;
            (order, candidate, outcome)
        });
    }
    let mut outcomes = Vec::new();
    while let Some(joined) = searches.join_next().await {
        match joined {
            Ok(outcome) => outcomes.push(outcome),
            Err(err) => tracing::error!(%err, "candidate search task panicked"),
        }
    }
    // Merge in candidate order so the result order doesn't depend on which
    // query happened to finish first.
    outcomes.sort_by_key(|(order, _, _)| *order);

    let mut seen_view_urls = std::collections::HashSet::new();
    let mut merged = Vec::new();
    let mut last_err = None;
    for (_, candidate, outcome) in outcomes {
        match outcome {
            Ok(results) => {
                tracing::info!(query = candidate, count = results.len(), "candidate search completed");
                for result in results {
                    if seen_view_urls.insert(result.view_url.clone()) {
                        merged.push(result);
                    }
                }
            }
            Err(err) => {
                tracing::error!(query = candidate, %err, "candidate search failed");
                last_err = Some(err);
            }
        }
    }
    if merged.is_empty() {
        if let Some(err) = last_err {
            return Err(err.to_string());
        }
    }

    let names = title_match::match_names(title.english.as_deref(), title.romaji.as_deref(), &synonyms);
    let before = merged.len();
    merged.retain(|result| title_match::matches_show(&result.title, &names));
    tracing::info!(kept = merged.len(), dropped = before - merged.len(), "dropped releases that don't name the show");

    // A preferred fansubber whose nyaa.si account isn't known yet: learn it
    // from one of their releases here, so the next search can go straight to
    // their uploads (`search_fansubber_releases`).
    if let Some(tag) = fansubber.as_deref().and_then(|text| text.split_whitespace().next()) {
        let app = state.inner().clone();
        let (tag, releases) = (tag.to_string(), merged.clone());
        tokio::spawn(async move {
            if app.nyaa.resolve_fansubber(&tag).await.is_none() {
                app.nyaa.learn_fansubber(&tag, &releases).await;
            }
        });
    }
    Ok(merged)
}

/// Releases for the show already in the local database - instant, no nyaa.si
/// request; whatever earlier searches stored (possibly incomplete).
#[tauri::command]
async fn search_local_releases(
    state: State<'_, Arc<AppState>>,
    title: AnimeTitle,
    synonyms: Option<Vec<String>>,
) -> Result<Vec<NyaaResult>, String> {
    let synonyms = synonyms.unwrap_or_default();
    let candidates = title_match::build_candidates(title.english.as_deref(), title.romaji.as_deref(), &synonyms);
    let mut lists = Vec::new();
    for candidate in &candidates {
        lists.push(state.nyaa.search_local(candidate, Category::AnimeEnglishTranslated).await);
    }
    let names = title_match::match_names(title.english.as_deref(), title.romaji.as_deref(), &synonyms);
    let mut merged = merge_unique(lists);
    merged.retain(|result| title_match::matches_show(&result.title, &names));
    tracing::info!(count = merged.len(), "local release search for the show");
    Ok(merged)
}

/// The show's releases from one fansubber only (nyaa.si's per-uploader
/// search - a few results instead of every group's pages). `fansubber` is
/// the Settings text ("ToonsHub CR"): its first word names the group. With a
/// known nyaa.si account it searches that account's uploads; otherwise (some
/// groups, ToonsHub included, upload anonymously) it adds the group name to
/// the query, which nyaa.si matches as a word of the release title.
#[tauri::command]
async fn search_fansubber_releases(
    state: State<'_, Arc<AppState>>,
    title: AnimeTitle,
    synonyms: Option<Vec<String>>,
    fansubber: String,
) -> Result<Vec<NyaaResult>, String> {
    let Some(tag) = fansubber.split_whitespace().next() else { return Ok(Vec::new()) };
    let user = state.nyaa.resolve_fansubber(tag).await;
    let synonyms = synonyms.unwrap_or_default();
    let candidates = title_match::build_candidates(title.english.as_deref(), title.romaji.as_deref(), &synonyms);

    let mut searches = tokio::task::JoinSet::new();
    for (order, candidate) in candidates.into_iter().enumerate() {
        let (app, user, tag) = (state.inner().clone(), user.clone(), tag.to_string());
        searches.spawn(async move {
            let outcome = match &user {
                Some(user) => app.nyaa.search_user(user, &candidate, Category::AnimeEnglishTranslated).await,
                None => app.nyaa.search(&format!("{tag} {candidate}"), Category::AnimeEnglishTranslated).await,
            };
            (order, outcome)
        });
    }
    let mut outcomes = Vec::new();
    while let Some(joined) = searches.join_next().await {
        if let Ok(outcome) = joined {
            outcomes.push(outcome);
        }
    }
    outcomes.sort_by_key(|(order, _)| *order);
    let lists: Vec<Vec<NyaaResult>> = outcomes
        .into_iter()
        .filter_map(|(_, outcome)| outcome.inspect_err(|err| tracing::warn!(%err, "uploader-only search failed")).ok())
        .collect();

    let names = title_match::match_names(title.english.as_deref(), title.romaji.as_deref(), &synonyms);
    let mut merged = merge_unique(lists);
    let tag_word = title_match::normalize(tag);
    merged.retain(|result| title_match::matches_show(&result.title, &names) && (user.is_some() || title_match::normalize(&result.title).split(' ').any(|w| w == tag_word)));
    tracing::info!(?user, count = merged.len(), "fansubber-only release search");
    Ok(merged)
}

/// Popular fansub group names for Settings' suggestions.
#[tauri::command]
async fn get_popular_fansubbers(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    Ok(state.nyaa.popular_fansubbers(40).await)
}

/// `lists` flattened, one entry per nyaa.si release (by view URL), first
/// occurrence wins.
fn merge_unique(lists: Vec<Vec<NyaaResult>>) -> Vec<NyaaResult> {
    let mut seen = std::collections::HashSet::new();
    lists.into_iter().flatten().filter(|r| seen.insert(r.view_url.clone())).collect()
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
struct PlayFile {
    #[serde(flatten)]
    file: TorrentFile,
    /// Base HLS playlist URL for this file - the frontend appends
    /// `?duration=<seconds>`.
    hls_url: String,
    /// Raw Range-capable byte stream of this file - what the embedded mpv
    /// player opens.
    stream_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaySession {
    torrent_id: TorrentId,
    /// Every file in the torrent with its own HLS URL - the frontend picks
    /// which one to play (matching the requested episode inside a batch
    /// with its own title parser) and can switch between them without
    /// re-adding the torrent.
    files: Vec<PlayFile>,
    /// Largest video file - the frontend's fallback pick.
    default_file_idx: usize,
}

/// Adds `magnet` to the torrent session and returns every file's raw
/// stream URL (what the embedded mpv opens - see `player.rs`) plus its HLS
/// playlist URL (the fallback player's, used when mpv isn't installed).
#[tauri::command]
async fn play_magnet(
    state: State<'_, Arc<AppState>>,
    magnet: String,
    title: String,
    watch: Option<String>,
    episode: Option<String>,
) -> Result<PlaySession, String> {
    tracing::debug!(%title, ?watch, ?episode, "play_magnet invoked");
    // How the file starts (see torrent_engine::WatchHint): "first" (no
    // saved progress) or "resume" (a start time follows).
    let watch_hint = match watch.as_deref() {
        Some("first") => Some(torrent_engine::WatchHint::First),
        Some("resume") => Some(torrent_engine::WatchHint::Resume),
        None => None,
        Some(other) => {
            tracing::warn!(%title, watch = other, "play_magnet: unknown watch hint ignored");
            None
        }
    };

    // Cancels a deferred removal from the previous player view (see
    // `stop_playback`).
    state.stop_generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    // The next episode of the same torrent (a batch) reuses the session, with
    // whatever the low-priority preload already downloaded. Anything else
    // replaces it: the frontend calls stop_playback before navigating away or
    // starting a new stream, but a leftover session (e.g. a client that
    // skipped that call) shouldn't leak alongside a new one.
    let wanted = magnet_info_hash(&magnet);
    let current = state.current_torrent.lock().await.clone();
    let added_id = match (wanted, current) {
        (Some(wanted), Some(current)) if current.eq_ignore_ascii_case(&wanted) => {
            tracing::info!(%title, torrent_id = %current, "play_magnet reusing the current torrent");
            current
        }
        _ => {
            cleanup_playback(&state).await;
            let added = match state.torrent_engine.add(&magnet).await {
                Ok(added) => added,
                Err(err) => {
                    tracing::error!(%title, %err, "play_magnet failed to add torrent");
                    return Err(err.to_string());
                }
            };
            *state.current_torrent.lock().await = Some(added.id.clone());
            added.id
        }
    };
    let added = torrent_engine::AddedTorrent { id: added_id };
    // The HLS URL is only for the fallback player: raw container bytes
    // aren't reliably playable in a browser <video> element (see
    // torrent-engine's hls_playlist_handler), while mpv reads them fine.
    let files = state.torrent_engine.files(&added.id).await.map_err(|err| {
        tracing::error!(%title, torrent_id = %added.id, %err, "play_magnet failed to get file list");
        err.to_string()
    })?;
    let default_file_idx = largest_video_file(&files).unwrap_or(0);
    state.download_cache.touch(&added.id, &title, &files, episode.as_deref());
    resume::attach(&state.torrent_engine, &added.id, &files).await;
    if let Err(err) = state.torrent_engine.set_watch_hint(&added.id, watch_hint).await {
        tracing::warn!(%title, torrent_id = %added.id, %err, "play_magnet failed to set the watch hint");
    }
    let files: Vec<PlayFile> = files
        .into_iter()
        .map(|file| PlayFile {
            hls_url: state.torrent_engine.hls_playlist_url(&added.id, file.index),
            stream_url: state.torrent_engine.stream_url(&added.id, file.index),
            file,
        })
        .collect();
    tracing::info!(%title, torrent_id = %added.id, file_count = files.len(), default_file_idx, "torrent added, streaming");

    Ok(PlaySession { torrent_id: added.id, files, default_file_idx })
}

/// The Continue watching row's episodes (`<animeId>:<episodeKey>`), whose
/// torrents the download cache keeps past its cap; sent on every change.
#[tauri::command]
async fn set_download_cache_keep(state: State<'_, Arc<AppState>>, episodes: Vec<String>) -> Result<(), String> {
    tracing::debug!(count = episodes.len(), "set_download_cache_keep invoked");
    let playing = state.current_torrent.lock().await.clone();
    let app = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        app.download_cache.set_keep(episodes);
        app.download_cache.evict(playing.as_deref(), false, "left Continue watching");
    })
    .await
    .map_err(|err| err.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadCacheStatus {
    used_bytes: u64,
    limit_bytes: u64,
    entries: usize,
}

/// Download cache usage and cap, for Settings.
#[tauri::command]
async fn download_cache_status(state: State<'_, Arc<AppState>>) -> Result<DownloadCacheStatus, String> {
    tracing::debug!("download_cache_status invoked");
    let app = state.inner().clone();
    let (used_bytes, limit_bytes, entries) = tokio::task::spawn_blocking(move || app.download_cache.status()).await.map_err(|err| err.to_string())?;
    Ok(DownloadCacheStatus { used_bytes, limit_bytes, entries })
}

/// Sets the download cache cap (bytes, 0 = delete on stop) and trims to it.
#[tauri::command]
async fn set_download_cache_limit(state: State<'_, Arc<AppState>>, limit_bytes: u64) -> Result<(), String> {
    tracing::debug!(limit_bytes, "set_download_cache_limit invoked");
    let playing = state.current_torrent.lock().await.clone();
    let app = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        app.download_cache.set_limit(limit_bytes);
        app.download_cache.evict(playing.as_deref(), false, "limit changed");
    })
    .await
    .map_err(|err| err.to_string())
}

/// How long libtorrent may keep a removed torrent's files open.
const FILE_RELEASE_DELAY: Duration = Duration::from_secs(2);

/// Removes the backing torrent from the session (stop seeding) - used by
/// `stop_playback` and defensively by `play_magnet` before starting a new
/// session. Its files stay in the download cache, which is then trimmed to
/// its cap.
async fn cleanup_playback(state: &Arc<AppState>) {
    if let Some(torrent_id) = state.current_torrent.lock().await.take() {
        if let Err(err) = state.torrent_engine.remove(torrent_id.clone()).await {
            tracing::warn!(torrent_id = %torrent_id, %err, "cleanup_playback: failed to remove torrent");
        }
        let app = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(FILE_RELEASE_DELAY).await;
            let playing = app.current_torrent.lock().await.clone();
            let _ = tokio::task::spawn_blocking(move || app.download_cache.evict(playing.as_deref(), false, "playback stopped")).await;
        });
    }
}

/// Download progress/speed/peer-count snapshot for the currently-playing
/// torrent, polled by the frontend to show buffering feedback (mirrors
/// Stremio's streaming-server statistics endpoint - see
/// `torrent_engine::StreamStats` doc comment).
#[tauri::command]
async fn get_stream_stats(state: State<'_, Arc<AppState>>, torrent_id: TorrentId, file_idx: usize) -> Result<StreamStats, String> {
    state.torrent_engine.stats(&torrent_id, file_idx).await.map_err(|err| {
        tracing::debug!(torrent_id = %torrent_id, file_idx, %err, "get_stream_stats failed");
        err.to_string()
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleTrackInfo {
    index: usize,
    language: Option<String>,
    title: Option<String>,
    codec: String,
    default: bool,
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleInfo {
    tracks: Vec<SubtitleTrackInfo>,
    /// Embedded font attachment URLs, for the frontend's libass renderer.
    fonts: Vec<String>,
}

/// Lists `torrent_id`'s embedded text subtitle tracks and font attachments
/// for one file of the torrent - see
/// `TorrentEngine::media_probe`. Errors while the container header hasn't
/// downloaded yet; the frontend retries until this succeeds, since that
/// failure is transient and never cached.
#[tauri::command]
async fn get_subtitle_tracks(state: State<'_, Arc<AppState>>, torrent_id: TorrentId, file_idx: usize) -> Result<SubtitleInfo, String> {
    tracing::debug!(torrent_id = %torrent_id, file_idx, "get_subtitle_tracks invoked");
    let probe = state.torrent_engine.media_probe(&torrent_id, file_idx).await.map_err(|err| {
        tracing::warn!(torrent_id = %torrent_id, %err, "get_subtitle_tracks failed");
        err.to_string()
    })?;
    tracing::info!(torrent_id = %torrent_id, count = probe.subtitles.len(), fonts = probe.fonts.len(), "get_subtitle_tracks succeeded");
    let tracks = probe
        .subtitles
        .into_iter()
        .map(|track: SubtitleTrack| SubtitleTrackInfo {
            url: state.torrent_engine.subtitle_url(&torrent_id, file_idx, track.index),
            index: track.index,
            language: track.language,
            title: track.title,
            codec: track.codec,
            default: track.default,
        })
        .collect();
    let fonts = probe.fonts.iter().map(|font| state.torrent_engine.font_url(&torrent_id, file_idx, font.index)).collect();
    Ok(SubtitleInfo { tracks, fonts })
}

/// Frontend reports which codecs its WebView decodes natively (MSE
/// `isTypeSupported`), so the streaming server only transcodes what it has
/// to - see `torrent_engine::plan_video`.
#[tauri::command]
fn set_decoder_support(state: State<'_, Arc<AppState>>, support: DecoderSupport) {
    tracing::debug!(?support, "set_decoder_support invoked");
    state.torrent_engine.set_decoder_support(support);
}

/// Puts a video frame on the system clipboard. The body is raw RGBA pixels
/// (sent as binary IPC - a 1080p frame is ~8MB, too big for JSON), with
/// the dimensions in `x-width`/`x-height` headers. Native rather than the
/// WebView's `navigator.clipboard.write`, which in WebView2 raises a
/// browser-style "See text and images copied to the clipboard" permission
/// prompt (verified live).
#[tauri::command]
async fn copy_frame_to_clipboard(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected a raw RGBA body".to_string());
    };
    let header = |name: &str| -> Result<usize, String> {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| format!("missing or invalid {name} header"))
    };
    let (width, height) = (header("x-width")?, header("x-height")?);
    if bytes.len() != width * height * 4 {
        tracing::warn!(width, height, len = bytes.len(), "copy_frame_to_clipboard size mismatch");
        return Err(format!("expected {} bytes for {width}x{height}, got {}", width * height * 4, bytes.len()));
    }
    tracing::debug!(width, height, "copy_frame_to_clipboard invoked");
    set_clipboard_image(width, height, bytes.clone()).await?;
    tracing::info!(width, height, "frame copied to clipboard");
    Ok(())
}

/// Puts an RGBA image on the native clipboard, retrying while another
/// process holds it.
pub(crate) async fn set_clipboard_image(width: usize, height: usize, pixels: Vec<u8>) -> Result<(), String> {
    // Windows lets only one process open the clipboard at a time, and other
    // apps (clipboard managers, RDP, the shell) hold it briefly all the
    // time - verified live: "held by another party" on a first attempt
    // with nothing holding it moments later. Retry for about a second.
    const ATTEMPTS: u32 = 12;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(80);
    tokio::task::spawn_blocking(move || {
        let mut last_err = String::new();
        for attempt in 1..=ATTEMPTS {
            let result = arboard::Clipboard::new().and_then(|mut clipboard| {
                clipboard.set_image(arboard::ImageData { width, height, bytes: std::borrow::Cow::Borrowed(&pixels) })
            });
            match result {
                Ok(()) => return Ok(()),
                Err(err) => {
                    tracing::debug!(attempt, %err, "clipboard busy, retrying");
                    last_err = err.to_string();
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
        Err(last_err)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| {
        tracing::error!(%err, "clipboard image write failed");
        err
    })
}

/// Removes the active torrent (stop seeding, drop partial files) - called
/// when the user closes the player or navigates away from the media page.
///
/// The removal is deferred a few seconds: an episode change unmounts the old
/// player view (this call) just before the new one asks for its torrent, and
/// a batch's next episode is the same torrent - `play_magnet` cancels the
/// removal and keeps it, preloaded data included.
///
/// `resume` (an episode still in progress) first saves its resume buffer
/// while the torrent is still in the engine - see `resume`.
#[tauri::command]
async fn stop_playback(state: State<'_, Arc<AppState>>, resume: Option<resume::ResumeRequest>) -> Result<(), String> {
    /// Long enough for the next episode's `play_magnet` to arrive.
    const KEEP_FOR_NEXT_EPISODE: Duration = Duration::from_secs(4);
    tracing::debug!(?resume, "stop_playback invoked");
    let generation = state.stop_generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let app = state.inner().clone();
    let playing = app.current_torrent.lock().await.clone();
    tokio::spawn(async move {
        let started = tokio::time::Instant::now();
        match (resume, playing) {
            (Some(request), Some(torrent_id)) if magnet_info_hash(&request.magnet).is_some_and(|h| h.eq_ignore_ascii_case(&torrent_id)) => {
                if let Err(err) = resume::save(&app.torrent_engine, &torrent_id, request).await {
                    tracing::warn!(torrent_id = %torrent_id, %err, "resume buffer not saved");
                }
            }
            (Some(_), playing) => tracing::debug!(?playing, "resume request doesn't match the playing torrent, no buffer"),
            (None, _) => {}
        }
        tokio::time::sleep(KEEP_FOR_NEXT_EPISODE.saturating_sub(started.elapsed())).await;
        if app.stop_generation.load(std::sync::atomic::Ordering::SeqCst) == generation {
            cleanup_playback(&app).await;
            tracing::info!("stop_playback completed");
        } else {
            tracing::debug!("stop_playback removal cancelled, the torrent is in use again");
        }
    });
    Ok(())
}

/// The player opened `file_idx` (mpv's `file-loaded`): what it read so far
/// is what a resume buffer must hold - see torrent_engine::resume_buffer.
#[tauri::command]
fn stream_file_loaded(state: State<'_, Arc<AppState>>, torrent_id: TorrentId, file_idx: usize) {
    tracing::debug!(torrent_id = %torrent_id, file_idx, "stream_file_loaded invoked");
    state.torrent_engine.finish_open_reads(&torrent_id, file_idx);
}

/// Registers `file_idx` (the next episode's file in the playing batch) to be
/// fetched at the lowest priority next to the playing file; `None` stops.
/// Best effort - failures only mean no preload.
#[tauri::command]
async fn preload_next_file(state: State<'_, Arc<AppState>>, torrent_id: TorrentId, file_idx: Option<usize>) -> Result<(), String> {
    state.torrent_engine.preload_file(&torrent_id, file_idx).await.map_err(|err| {
        tracing::debug!(%err, "preload_next_file failed");
        err.to_string()
    })
}

/// The 40-hex info hash of a magnet link (`xt=urn:btih:...`), lowercased;
/// `None` for anything else (base32 hashes, .torrent URLs).
fn magnet_info_hash(magnet: &str) -> Option<String> {
    let rest = magnet.split("btih:").nth(1)?;
    let hash: String = rest.chars().take_while(char::is_ascii_alphanumeric).collect();
    (hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_lowercase())
}

/// Where clips go unless the export dialog picked a folder.
fn default_clip_dir() -> std::path::PathBuf {
    dirs::video_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream clips")
}

/// The folder the export dialog starts with.
#[tauri::command]
fn default_clip_folder() -> String {
    default_clip_dir().to_string_lossy().into_owned()
}

/// Cuts an A-B clip of the playing torrent file to `<folder>/<name>.mp4` (the
/// folder defaults to `<Videos>/nyaa-stream clips`; normalized H.264/AAC - see
/// `TorrentEngine::export_clip`) and returns the written path.
#[tauri::command]
async fn export_clip(
    state: State<'_, Arc<AppState>>,
    torrent_id: TorrentId,
    file_idx: usize,
    start_seconds: f64,
    end_seconds: f64,
    audio_stream: usize,
    name: String,
    folder: Option<String>,
    include_subs: Option<bool>,
    audio_id: Option<i64>,
    subtitle_id: Option<i64>,
    subtitle_settings: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> Result<String, String> {
    tracing::debug!(torrent_id = %torrent_id, file_idx, start_seconds, end_seconds, audio_stream, %name, ?folder, "export_clip invoked");
    if !(end_seconds > start_seconds) {
        return Err("The clip's end must be after its start.".into());
    }
    let dir = match folder.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        Some(folder) => std::path::PathBuf::from(folder),
        None => default_clip_dir(),
    };
    tokio::fs::create_dir_all(&dir).await.map_err(|err| format!("Couldn't create {}: {err}", dir.display()))?;
    // Filesystem-safe stem; never overwrites an earlier clip.
    let stem: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let stem = stem.trim().chars().take(80).collect::<String>();
    let mut out = dir.join(format!("{stem}.mp4"));
    let mut n = 2;
    while out.exists() {
        out = dir.join(format!("{stem} ({n}).mp4"));
        n += 1;
    }
    // Burned-in subtitles go through libmpv's encoder (libass, the player's
    // own style); without them the in-process FFmpeg export is used.
    let exported = if include_subs.unwrap_or(false) && subtitle_id.is_some() {
        let mut options: Vec<(String, String)> = subtitle_settings
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| {
                let text = match value {
                    serde_json::Value::Bool(flag) => (if flag { "yes" } else { "no" }).to_string(),
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                };
                (name, text)
            })
            .collect();
        if let Ok(fonts) = player::install_fonts().await {
            options.push(("sub-fonts-dir".into(), fonts.to_string_lossy().into_owned()));
        }
        mpv_player::encode_clip(mpv_player::EncodeClip {
            input: state.torrent_engine.stream_url(&torrent_id, file_idx),
            out: out.clone(),
            start_seconds,
            end_seconds,
            audio_id,
            subtitle_id,
            options,
        })
        .await
    } else {
        state.torrent_engine.export_clip(&torrent_id, file_idx, start_seconds, end_seconds, audio_stream, out.clone()).await
    };
    match exported {
        Ok(()) => {
            tracing::info!(out = %out.display(), "export_clip succeeded");
            Ok(out.to_string_lossy().into_owned())
        }
        Err(err) => {
            tracing::warn!(%err, "export_clip failed");
            let _ = tokio::fs::remove_file(&out).await;
            Err(err.to_string())
        }
    }
}

/// Where the rolling log files live: `<local data dir>/nyaa-stream/logs`.
fn log_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("nyaa-stream")
        .join("logs")
}

/// Keeps the file writer's flush thread alive for the whole run.
static LOG_GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
    std::sync::OnceLock::new();

/// Logs to stdout (dev terminal) and to a daily-rotating file, since a release
/// build has no console. Keeps the last 7 days.
fn init_logging() {
    use tracing_subscriber::prelude::*;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        "nyaa_stream_lib=debug,anilist_client=debug,nyaa_client=debug,torrent_engine=debug,mpv_player=debug,frontend=debug,info"
            .into()
    });
    let stdout = tracing_subscriber::fmt::layer();
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("nyaa-stream")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir());
    match appender {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = LOG_GUARD.set(guard);
            let file = tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer);
            tracing_subscriber::registry().with(filter).with(stdout).with(file).init();
        }
        Err(err) => {
            tracing_subscriber::registry().with(filter).with(stdout).init();
            tracing::warn!("file logging disabled: {err}");
        }
    }
    tracing::info!(dir = %log_dir().display(), "logging initialised");
}

/// Swaps the splash window for the main one. Idempotent: called by the
/// frontend's `app_ready` and by the startup fallback timer.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(splash) = app.get_webview_window("splash") {
        tracing::debug!("closing splash window");
        if let Err(err) = splash.close() {
            tracing::warn!(%err, "failed to close splash window");
        }
    }
    match app.get_webview_window("main") {
        Some(main) => {
            tracing::info!("showing main window");
            if let Err(err) = main.show().and_then(|_| main.set_focus()) {
                tracing::error!(%err, "failed to show main window");
            }
        }
        None => tracing::error!("main window missing in show_main_window"),
    }
}

#[tauri::command]
fn app_ready(app: tauri::AppHandle) {
    tracing::info!("app_ready received");
    show_main_window(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    let app_state = tauri::async_runtime::block_on(async {
        tracing::info!("starting nyaa-stream");
        // Segments from the last run are unreachable garbage now.
        cache::purge_hls_cache();
        // The user's actual Downloads folder isn't the right place for
        // torrent scratch data the app manages and cleans up itself
        // (stop_playback removes the torrent, but the on-disk file lingers
        // until then) - AppData/cache is where transient app-owned data
        // belongs, same as thumbnail_cache_dir below.
        let download_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("nyaa-stream")
            .join("downloads");
        tracing::debug!(?download_dir, "torrent download dir");
        let download_cache = download_cache::DownloadCache::load(download_dir.clone());
        // Before the engine opens anything, so leftovers can go too.
        download_cache.evict(None, true, "startup");
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
            // Cache is disposable ("Clear cache"); the release database is permanent user data.
            nyaa: NyaaClient::with_storage(
                dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream").join("nyaa_cache"),
                &dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream").join("nyaa.db"),
            ),
            torrent_engine,
            current_torrent: Mutex::new(None),
            stop_generation: Default::default(),
            thumbnail_cache_dir,
            anilist_cooldown: Default::default(),
            thumbnail_captures: tokio::sync::Semaphore::new(1),
            download_cache,
        })
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(media_keys::plugin())
        .manage(app_state.clone())
        .manage(player::PlayerState::default())
        .setup(|app| {
            // A broken page must not leave the app invisible: show the main
            // window anyway if app_ready never arrives.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(20)).await;
                let hidden = handle.get_webview_window("main").and_then(|w| w.is_visible().ok()) == Some(false);
                if hidden {
                    tracing::warn!("app_ready not received within 20s, showing main window anyway");
                    show_main_window(&handle);
                }
            });
            Ok(())
        })
        // The embedded mpv is pre-spawned only after the page has loaded:
        // spawning it into the window at launch, while WebView2 was still
        // initializing there, delayed the page by up to ~34s (vs 0.5-2s).
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                tracing::debug!(url = %payload.url(), "page load finished");
                player::warm_mpv_once(tauri::Manager::app_handle(webview).clone());
            }
        })
        .register_asynchronous_uri_scheme_protocol(THUMBNAIL_SCHEME, move |_ctx, request, responder| {
            let state = app_state.clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(thumbnail_protocol(&state, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            app_ready,
            log_frontend,
            search_anime,
            get_anime_details,
            get_absolute_episode_offset,
            get_latest_episodes,
            get_kitsu_metadata,
            capture_torrent_thumbnail,
            cached_torrent_thumbnail,
            save_frame_thumbnail,
            search_torrents,
            search_torrents_for_anime,
            preload_next_file,
            stream_file_loaded,
            default_clip_folder,
            player::mpv_save_frame,
            player::default_screenshot_folder,
            search_local_releases,
            search_fansubber_releases,
            get_popular_fansubbers,
            get_torrent_details_batch,
            play_magnet,
            get_stream_stats,
            get_subtitle_tracks,
            set_decoder_support,
            copy_frame_to_clipboard,
            stop_playback,
            export_clip,
            media_keys::set_media_keys,
            cache::get_cache_sizes,
            cache::clear_cache,
            set_download_cache_keep,
            download_cache_status,
            set_download_cache_limit,
            cache::export_backup,
            cache::import_backup,
            player::mpv_available,
            player::set_mpv_path,
            player::mpv_start,
            player::mpv_command,
            player::mpv_stop,
            player::mpv_frame,
            player::mpv_copy_frame
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                cache::purge_hls_cache();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::magnet_info_hash;

    #[test]
    fn magnet_info_hash_reads_hex_hashes_only() {
        let hex = "0123456789ABCDEF0123456789abcdef01234567";
        assert_eq!(magnet_info_hash(&format!("magnet:?xt=urn:btih:{hex}&dn=x&tr=y")).as_deref(), Some(hex.to_lowercase().as_str()));
        assert_eq!(magnet_info_hash("magnet:?xt=urn:btih:MFRGGZDFMZTWQ2LKNNWG23TPOBYXE43U&dn=x"), None);
        assert_eq!(magnet_info_hash("https://nyaa.si/download/1.torrent"), None);
    }
}
