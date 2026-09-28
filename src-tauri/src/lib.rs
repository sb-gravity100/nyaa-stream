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
    /// Playback itself is an HTML5 `<video>` element in the frontend
    /// pointed straight at torrent-engine's stream URL - no player process
    /// to track here (see PLAN.md's Known gaps for why mpv embedding was
    /// dropped).
    current_torrent: Mutex<Option<TorrentId>>,
    thumbnail_cache_dir: PathBuf,
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

/// Fallback seek target when there's no duration estimate to compute a real
/// midpoint from (see `capture_thumbnail_uncached`) - far enough to usually
/// be past a cold-open/company-logo black frame, still early enough that
/// reaching it by just letting playback run from 0 (no `--start`, avoiding
/// mpv needing to seek at all) only pulls a small amount of data.
const THUMBNAIL_SEEK_SECONDS: f64 = 8.0;
/// Slack allowed when we *do* have a duration estimate and ask mpv to
/// start already near the midpoint via `--start` (see mpv-ipc's
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
    let cache_path = state.thumbnail_cache_dir.join(format!("{cache_key}.jpg"));

    if let Ok(bytes) = tokio::fs::read(&cache_path).await {
        tracing::debug!(cache_key, "capture_torrent_thumbnail cache hit");
        return Ok(Some(to_data_uri(&bytes)));
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
    duration_minutes: Option<f64>,
) -> anyhow::Result<Vec<u8>> {
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

/// Searches nyaa.si for an AniList anime using both its English and romaji
/// titles and merges the results (deduplicated by view URL, since a release
/// whose own title happens to contain both would otherwise show up twice).
///
/// This used to search English first and only fall back to romaji if
/// English returned fewer than a handful of results - but nyaa.si's own
/// per-word AND-matching tokenizer means a real show's English-title search
/// almost always clears that threshold on its own, so the romaji fallback
/// essentially never fired in practice. Verified live against "That Time I
/// Got Reincarnated as a Slime": many fansub groups (SubsPlease, Erai-raws,
/// Ironclad, ASW, and others) title their releases in romaji only, with no
/// English cross-reference text at all - 489 of that show's 950 real
/// releases were being silently dropped by English-only search, entirely
/// missed regardless of how many result pages got fetched. Searching both
/// unconditionally (not as a fallback) is the only way to actually get all
/// of a show's sources.
/// Strips a trailing "Season N"/"Nth Season"/"Part N"/"Final Season"
/// qualifier off an AniList title, e.g. "That Time I Got Reincarnated as a
/// Slime Season 4" -> Some("That Time I Got Reincarnated as a Slime"). Used
/// to build an extra, unqualified search candidate alongside the full
/// title - see search_torrents_for_anime's doc comment: nyaa.si's search is
/// per-word AND-matching, so a query carrying literal "Season 4" only
/// matches releases whose *title text* also contains "Season" and "4" as
/// separate words. Verified live: ToonsHub numbers this exact show
/// "S04E21" (no "Season" token at all), so the full-title query returns
/// zero of its ~100 real Season 4 releases even though a plain "Slime"
/// search finds every one of them on the first page. Returns None when no
/// such suffix is found, so the caller can skip adding a redundant
/// duplicate candidate.
fn strip_season_suffix(title: &str) -> Option<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        let lower = word.trim_end_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if lower == "season" || lower == "cour" {
            // A leading ordinal/number ("4th Season", "2nd Season") or
            // "Final" (e.g. "Final Season") belongs to the suffix too, not
            // the show's own name - cut before it.
            let prev_is_qualifier = i > 0
                && (words[i - 1].chars().next().is_some_and(|c| c.is_ascii_digit())
                    || words[i - 1].eq_ignore_ascii_case("final"));
            let cut = if prev_is_qualifier { i - 1 } else { i };
            if cut == 0 {
                return None; // "Season" is the whole title - nothing to strip.
            }
            return Some(words[..cut].join(" "));
        }
        if lower == "part" && i > 0 {
            return Some(words[..i].join(" "));
        }
    }
    None
}

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
    for base in [title.english.as_deref(), title.romaji.as_deref()].into_iter().flatten() {
        if let Some(stripped) = strip_season_suffix(base) {
            if !candidates.iter().any(|c| c.eq_ignore_ascii_case(&stripped)) {
                candidates.push(stripped);
            }
        }
    }
    if candidates.is_empty() {
        tracing::warn!("search_torrents_for_anime called with no usable title");
        return Ok(Vec::new());
    }

    tracing::debug!(?candidates, "search_torrents_for_anime invoked");

    let mut seen_view_urls = std::collections::HashSet::new();
    let mut merged = Vec::new();
    let mut last_err = None;
    for candidate in &candidates {
        match state.nyaa.search(candidate, Category::AnimeEnglishTranslated).await {
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
    Ok(merged)
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

/// Adds `magnet` to the torrent session and returns an HLS playlist URL for
/// it. Playback itself is an HTML5 `<video>` element driven by `hls.js` in
/// the frontend - there's no player process to spawn or drive over IPC
/// here, unlike the mpv-based approach this replaced (see PLAN.md's Known
/// gaps).
#[tauri::command]
async fn play_magnet(state: State<'_, Arc<AppState>>, magnet: String, title: String) -> Result<PlaySession, String> {
    tracing::debug!(%title, "play_magnet invoked");

    // Defensive: the frontend calls stop_playback before navigating away or
    // starting a new stream, but a leftover session (e.g. a client that
    // skipped that call) shouldn't be allowed to leak alongside a new one.
    cleanup_playback(&state).await;

    let added = match state.torrent_engine.add(&magnet).await {
        Ok(added) => added,
        Err(err) => {
            tracing::error!(%title, %err, "play_magnet failed to add torrent");
            return Err(err.to_string());
        }
    };
    *state.current_torrent.lock().await = Some(added.id.clone());
    // HLS (via ffmpeg-produced segments) rather than the raw stream_url -
    // see torrent-engine's hls_playlist_handler doc comment for why the raw
    // container bytes aren't reliably playable in a browser <video> element.
    let files = state.torrent_engine.files(&added.id).await.map_err(|err| {
        tracing::error!(%title, torrent_id = %added.id, %err, "play_magnet failed to get file list");
        err.to_string()
    })?;
    let default_file_idx = largest_video_file(&files).unwrap_or(0);
    let files: Vec<PlayFile> = files
        .into_iter()
        .map(|file| PlayFile { hls_url: state.torrent_engine.hls_playlist_url(&added.id, file.index), file })
        .collect();
    tracing::info!(%title, torrent_id = %added.id, file_count = files.len(), default_file_idx, "torrent added, streaming");

    Ok(PlaySession { torrent_id: added.id, files, default_file_idx })
}

/// Removes the backing torrent (stop seeding, drop partial files) - used by
/// `stop_playback` and defensively by `play_magnet` before starting a new
/// session.
async fn cleanup_playback(state: &AppState) {
    if let Some(torrent_id) = state.current_torrent.lock().await.take() {
        if let Err(err) = state.torrent_engine.remove(torrent_id.clone()).await {
            tracing::warn!(torrent_id = %torrent_id, %err, "cleanup_playback: failed to remove torrent");
        }
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

/// Removes the active torrent (stop seeding, drop partial files) - called
/// when the user closes the player or navigates away from the media page.
#[tauri::command]
async fn stop_playback(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    tracing::debug!("stop_playback invoked");
    cleanup_playback(&state).await;
    tracing::info!("stop_playback completed");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "nyaa_stream_lib=debug,anilist_client=debug,nyaa_client=debug,torrent_engine=debug,mpv_ipc=debug,frontend=debug,info"
                .into()
        }))
        .init();

    let app_state = tauri::async_runtime::block_on(async {
        tracing::info!("starting nyaa-stream");
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
            current_torrent: Mutex::new(None),
            thumbnail_cache_dir,
        })
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            log_frontend,
            search_anime,
            get_anime_details,
            get_absolute_episode_offset,
            get_latest_episodes,
            get_kitsu_metadata,
            capture_torrent_thumbnail,
            search_torrents,
            search_torrents_for_anime,
            get_torrent_details_batch,
            play_magnet,
            get_stream_stats,
            get_subtitle_tracks,
            set_decoder_support,
            stop_playback
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
