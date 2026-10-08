//! Prints an entry's franchise as the matcher sees it.
//! `cargo run -p anilist-client --example franchise_debug -- <anilist id>`

use anilist_client::AniListClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id: i64 = std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("usage: franchise_debug <anilist id>"))?.parse()?;
    let franchise = AniListClient::new().franchise(id).await?;
    println!("format: {:?}, has_tv_prequel: {}", franchise.format, franchise.has_tv_prequel);
    for relative in franchise.relatives {
        println!("{:>8} {:<12} {:<8} {} / {}", relative.id, relative.relation, relative.format.as_deref().unwrap_or("?"), relative.title.english.as_deref().unwrap_or("-"), relative.title.romaji.as_deref().unwrap_or("-"));
    }
    Ok(())
}
