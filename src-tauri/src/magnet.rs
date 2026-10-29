use std::sync::Arc;
use tauri::State;

async fn cleanup_inspection(state: &super::AppState, id: &torrent_engine::TorrentId) {
    let is_playing = state.current_torrent.lock().await.as_ref().is_some_and(|current| current == id);
    if !is_playing {
        if let Err(err) = state.torrent_engine.remove(id.clone()).await { tracing::warn!(%err, "metadata inspection cleanup failed"); }
    }
}

fn command_error(command: &'static str, err: impl std::fmt::Display) -> String {
    tracing::error!(command, %err, "custom magnet command failed");
    err.to_string()
}

#[tauri::command]
pub(super) fn parse_custom_magnet(input: String) -> Result<nyaa_client::magnet::ParsedMagnet, String> {
    tracing::debug!(input_bytes = input.len(), "parse_custom_magnet invoked");
    nyaa_client::magnet::parse(&input).map_err(|err| command_error("parse_custom_magnet", err))
}

/// Resolves only torrent metadata; no file reader is opened, so media payloads
/// are not selected or downloaded. The temporary session is removed afterward.
#[tauri::command]
pub(super) async fn inspect_custom_magnet(state: State<'_, Arc<super::AppState>>, input: String) -> Result<Vec<nyaa_client::CustomSourceFile>, String> {
    let parsed = nyaa_client::magnet::parse(&input).map_err(|err| command_error("inspect_custom_magnet", err))?;
    tracing::info!(info_hash = %parsed.info_hash, "inspect_custom_magnet invoked");
    let added = state.torrent_engine.add(&parsed.magnet).await.map_err(|err| command_error("inspect_custom_magnet", err))?;
    let result = state.torrent_engine.files(&added.id).await.map(|files| files.into_iter().map(|file| {
        let label = nyaa_client::EpisodeLabel::of_path(&file.name);
        let (season, episode) = match label {
            nyaa_client::EpisodeLabel::Episode { season, number, .. } => (Some(season), Some(number)),
            _ => (None, None),
        };
        nyaa_client::CustomSourceFile { path: file.name, size: file.length, index: file.index, season, episode, is_video: file.is_video }
    }).collect::<Vec<_>>()).map_err(|err| err.to_string());
    cleanup_inspection(&state, &added.id).await;
    result.map_err(|err| command_error("inspect_custom_magnet", err))
}

#[tauri::command]
pub(super) async fn list_custom_sources(state: State<'_, Arc<super::AppState>>, anime_id: i64) -> Result<Vec<nyaa_client::CustomSource>, String> {
    tracing::debug!(anime_id, "list_custom_sources invoked");
    state.nyaa.custom_sources(anime_id).await.map_err(|err| command_error("list_custom_sources", err))
}

#[tauri::command]
pub(super) async fn save_custom_source(
    state: State<'_, Arc<super::AppState>>, anime_id: i64, input: String, title: String, kind: String,
    season: Option<i32>, episode_start: Option<i32>, episode_end: Option<i32>,
) -> Result<nyaa_client::CustomSource, String> {
    let parsed = nyaa_client::magnet::parse(&input).map_err(|err| command_error("save_custom_source", err))?;
    if anime_id <= 0 { return Err(command_error("save_custom_source", "anime id must be positive")); }
    if !matches!(kind.as_str(), "episode" | "pack") { return Err(command_error("save_custom_source", "source kind must be episode or pack")); }
    if season.is_some_and(|value| value < 1) || episode_start.is_some_and(|value| value < 1) || episode_end.is_some_and(|value| value < 1) { return Err(command_error("save_custom_source", "season and episode numbers must be positive")); }
    if episode_start.zip(episode_end).is_some_and(|(start, end)| start > end) { return Err(command_error("save_custom_source", "episode range is reversed")); }
    if kind == "episode" && (episode_start.is_none() || episode_end.is_some_and(|end| Some(end) != episode_start)) { return Err(command_error("save_custom_source", "episode sources require one episode number")); }
    let title = title.trim();
    if title.len() > 200 { return Err(command_error("save_custom_source", "source title is too long")); }
    tracing::info!(anime_id, info_hash = %parsed.info_hash, kind, "save_custom_source invoked");
    let mut files = Vec::new();
    if kind == "pack" {
        let added = state.torrent_engine.add(&parsed.magnet).await.map_err(|err| command_error("save_custom_source", err))?;
        let inspected = state.torrent_engine.files(&added.id).await;
        cleanup_inspection(&state, &added.id).await;
        files = inspected.map_err(|err| command_error("save_custom_source", err))?.into_iter().map(|file| {
            let (season, episode) = match nyaa_client::EpisodeLabel::of_path(&file.name) {
                nyaa_client::EpisodeLabel::Episode { season, number, .. } => (Some(season), Some(number)),
                _ => (None, None),
            };
            nyaa_client::CustomSourceFile { path: file.name, size: file.length, index: file.index, season, episode, is_video: file.is_video }
        }).collect();
    }
    state.nyaa.save_custom_source(nyaa_client::CustomSourceInput {
        anime_id, info_hash: parsed.info_hash, magnet: parsed.magnet,
        title: if title.is_empty() { parsed.name.unwrap_or_else(|| "Custom source".into()) } else { title.into() },
        kind, season, episode_start, episode_end, files,
    }).await.map_err(|err| command_error("save_custom_source", err))
}

#[tauri::command]
pub(super) async fn remove_custom_source(state: State<'_, Arc<super::AppState>>, anime_id: i64, id: i64) -> Result<bool, String> {
    tracing::info!(anime_id, source_id = id, "remove_custom_source invoked");
    state.nyaa.remove_custom_source(anime_id, id).await.map_err(|err| command_error("remove_custom_source", err))
}
