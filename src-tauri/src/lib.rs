use std::sync::Arc;

use anilist_client::{AniListClient, AnimeMedia};
use mpv_ipc::MpvPlayer;
use nyaa_client::{Category, NyaaClient, NyaaResult};
use tauri::State;
use tokio::sync::Mutex;
use torrent_engine::TorrentEngine;

struct AppState {
    anilist: AniListClient,
    nyaa: NyaaClient,
    torrent_engine: TorrentEngine,
    player: Mutex<Option<MpvPlayer>>,
}

#[tauri::command]
async fn search_anime(state: State<'_, Arc<AppState>>, query: String) -> Result<Vec<AnimeMedia>, String> {
    state.anilist.search(&query, 20).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_torrents(state: State<'_, Arc<AppState>>, query: String) -> Result<Vec<NyaaResult>, String> {
    state
        .nyaa
        .search(&query, Category::AnimeEnglishTranslated)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn play_magnet(state: State<'_, Arc<AppState>>, magnet: String, title: String) -> Result<(), String> {
    let added = state.torrent_engine.add(&magnet).await.map_err(|e| e.to_string())?;
    let stream_url = state.torrent_engine.stream_url(added.id, 0);

    let player = MpvPlayer::spawn(&stream_url, &title).await.map_err(|e| e.to_string())?;
    *state.player.lock().await = Some(player);
    Ok(())
}

#[tauri::command]
async fn set_pause(state: State<'_, Arc<AppState>>, paused: bool) -> Result<(), String> {
    let guard = state.player.lock().await;
    if let Some(player) = guard.as_ref() {
        player.set_pause(paused).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = tauri::async_runtime::block_on(async {
        let download_dir = dirs::download_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("nyaa-stream");
        let torrent_engine = TorrentEngine::start(download_dir)
            .await
            .expect("failed to start torrent engine");

        Arc::new(AppState {
            anilist: AniListClient::new(),
            nyaa: NyaaClient::new(),
            torrent_engine,
            player: Mutex::new(None),
        })
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            search_anime,
            search_torrents,
            play_magnet,
            set_pause
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
