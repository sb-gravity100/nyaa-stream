// Scratch debugging tool: `cargo run -p anilist-client --example offset_debug -- <query>`
// Searches AniList for the query, then prints each result's id, episodes,
// and cumulative_prequel_episodes() - for checking the absolute-episode
// offset against real data without launching the full app.
use anilist_client::AniListClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let query = std::env::args().nth(1).expect("usage: offset_debug <query>");
    let client = AniListClient::new();
    let results = client.search(&query, 10).await?;
    for media in &results {
        let offset = client.cumulative_prequel_episodes(media.id).await?;
        println!(
            "id={} title={:?} episodes={:?} offset={}",
            media.id, media.title.english, media.episodes, offset
        );
    }
    Ok(())
}
