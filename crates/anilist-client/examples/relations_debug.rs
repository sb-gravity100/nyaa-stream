// Scratch debugging tool: `cargo run -p anilist-client --example relations_debug -- <id>`
// Prints the raw relations edges AniList reports for a media id.
use anilist_client::AniListClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id: i64 = std::env::args().nth(1).expect("usage: relations_debug <id>").parse()?;
    let client = reqwest::Client::new();
    let query = r#"
    query ($id: Int) {
      Media(id: $id, type: ANIME) {
        id
        title { english }
        relations {
          edges {
            relationType(version: 2)
            node { id title { english } episodes format type }
          }
        }
      }
    }
    "#;
    let body = serde_json::json!({ "query": query, "variables": { "id": id } });
    let resp = client.post("https://graphql.anilist.co").json(&body).send().await?;
    let text = resp.text().await?;
    println!("{text}");
    let _ = AniListClient::new(); // keep dependency used
    Ok(())
}
