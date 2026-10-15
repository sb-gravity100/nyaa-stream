// Scratch tool for the learned matcher prototype (ml/): live nyaa.si search
// (no release database, no AnimeTosho data), one NyaaResult per output line as JSON.
// `cargo run -p nyaa-client --example search_json -- "<query>" ["<query>" ...] > results.jsonl`
use nyaa_client::{Category, NyaaClient};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let queries: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(!queries.is_empty(), "usage: search_json <query> [<query> ...]");
    let client = NyaaClient::new();
    let mut seen = HashSet::new();
    for q in &queries {
        let results = client.search(q, Category::AllAnime).await?;
        eprintln!("{q:?}: {} results", results.len());
        for r in results {
            if seen.insert(r.view_url.clone()) {
                println!("{}", serde_json::to_string(&r)?);
            }
        }
    }
    Ok(())
}
