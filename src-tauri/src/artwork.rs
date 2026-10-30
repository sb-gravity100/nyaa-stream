use std::path::PathBuf;

use artwork_client::{Artwork, ArtworkClient, EpisodeStill, ProviderCredentials};

fn cache_dir() -> PathBuf {
    tracing::debug!("resolving artwork cache directory");
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir)
}

#[tauri::command]
pub(super) async fn get_artwork(anilist_id: i64, season_start_date: Option<String>, credentials: ProviderCredentials) -> Result<Artwork, String> {
    tracing::debug!(anilist_id, season_start_date = ?season_start_date, "get_artwork invoked");
    if anilist_id <= 0 {
        tracing::warn!(anilist_id, "get_artwork rejected invalid AniList id");
        return Err("invalid AniList id".into());
    }
    let client = ArtworkClient::new(cache_dir(), credentials).map_err(|error| {
        tracing::error!(anilist_id, %error, "artwork client initialization failed");
        error.to_string()
    })?;
    client.get_artwork(anilist_id, season_start_date.as_deref()).await.map_err(|error| {
        tracing::error!(anilist_id, %error, "get_artwork failed");
        error.to_string()
    })
}

#[tauri::command]
pub(super) async fn get_episode_still(anilist_id: i64, episode: u32, credentials: ProviderCredentials) -> Result<Option<EpisodeStill>, String> {
    tracing::debug!(anilist_id, episode, "get_episode_still invoked");
    if anilist_id <= 0 || episode == 0 {
        tracing::warn!(anilist_id, episode, "get_episode_still rejected invalid id or episode");
        return Err("invalid AniList id or episode".into());
    }
    let client = ArtworkClient::new(cache_dir(), credentials).map_err(|error| {
        tracing::error!(anilist_id, episode, %error, "artwork client initialization failed");
        error.to_string()
    })?;
    client.get_episode_still(anilist_id, episode).await.map_err(|error| {
        tracing::error!(anilist_id, episode, %error, "get_episode_still failed");
        error.to_string()
    })
}
