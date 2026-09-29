//! Scratch tool (`cargo run -p nyaa-client --example store_debug -- "<query>" [fansubber]`):
//! runs a search through a throwaway database twice (network, then replay),
//! then a local word match, and optionally resolves + searches one fansubber.

use nyaa_client::{Category, NyaaClient};
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("nyaa_client=debug").with_target(false).init();
    let mut args = std::env::args().skip(1);
    let query = args.next().unwrap_or_else(|| "Fate/strange Fake".into());
    let dir = std::env::temp_dir().join(format!("nyaa-store-debug-{}", std::process::id()));
    let client = NyaaClient::with_storage(dir.join("cache"), &dir.join("nyaa.db"));

    let t = Instant::now();
    let first = client.search(&query, Category::AnimeEnglishTranslated).await?;
    println!("network: {} results in {:?}", first.len(), t.elapsed());
    let t = Instant::now();
    let again = client.search(&query, Category::AnimeEnglishTranslated).await?;
    println!("replay:  {} results in {:?}", again.len(), t.elapsed());
    let t = Instant::now();
    let local = client.search_local("fate strange fake", Category::AnimeEnglishTranslated).await;
    println!("local:   {} results in {:?}", local.len(), t.elapsed());

    if let Some(group) = args.next() {
        let user = client.resolve_fansubber(&group).await;
        println!("fansubber {group} -> {user:?}");
        if user.is_none() {
            client.learn_fansubber(&group, &first).await;
            println!("after learning -> {:?}", client.resolve_fansubber(&group).await);
        }
        if let Some(user) = client.resolve_fansubber(&group).await {
            let t = Instant::now();
            let mine = client.search_user(&user, &query, Category::AnimeEnglishTranslated).await?;
            println!("{user} only: {} results in {:?}", mine.len(), t.elapsed());
        }
    }
    println!("popular: {:?}", client.popular_fansubbers(15).await);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
