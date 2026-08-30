// Scratch debugging tool: `cargo run -p nyaa-client --example search_debug -- "<query>"`
// Prints every NyaaResult title our own search() returns, for checking
// episode-parsing mismatches against real nyaa.si data without launching
// the full app.
use nyaa_client::{Category, NyaaClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let query = std::env::args().nth(1).expect("usage: search_debug <query>");
    let client = NyaaClient::new();
    let results = client.search(&query, Category::AnimeEnglishTranslated).await?;
    println!("{} results", results.len());
    for r in &results {
        println!("{}", r.title);
    }
    Ok(())
}
