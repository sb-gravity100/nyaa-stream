//! Scratch tool: `cargo run -p kitsu-client --example fallback_debug -- "<query>" [anilist_id]`
//! prints what the AniList-fallback search / by-id lookups return.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let query = args.next().unwrap_or_else(|| "frieren".into());
    let client = kitsu_client::KitsuClient::new();
    for anime in client.search_anime(&query, 10).await? {
        println!(
            "anilist {} | kitsu {} | {:?} / {:?} | {:?} eps x {:?}min | {:?} {:?} | rating {:?}",
            anime.anilist_id, anime.kitsu_id, anime.title_english, anime.title_romaji, anime.episode_count, anime.episode_length, anime.subtype, anime.start_date, anime.average_rating
        );
    }
    if let Some(id) = args.next().and_then(|id| id.parse().ok()) {
        println!("by id {id}: {:#?}", client.anime_by_anilist_id(id).await?);
    }
    Ok(())
}
