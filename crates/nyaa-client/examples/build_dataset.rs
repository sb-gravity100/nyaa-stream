//! Builds an importable nyaa.si release database (Settings > Backup >
//! "nyaa.si release database") from crawl JSON files:
//! `cargo run -p nyaa-client --example build_dataset -- out.db crawl1.json crawl2.json ...`
//!
//! Accepts nyaasi_extractor output (`{"items": [...]}`, from "Extract this
//! page" or the auto-next crawl) and a plain `{"<query>": [items]}` map of
//! extractor items. An item needs `url` (/view/<id>), `title` and `magnet`;
//! items without a magnet are skipped. Category is the item's
//! `categoryId`, else `1_2` (Anime - English-translated).

use std::path::PathBuf;

use nyaa_client::{EpisodeLabel, NyaaResult};
use serde_json::Value;

fn items(json: &Value) -> Vec<&Value> {
    match json.get("items").and_then(Value::as_array) {
        Some(items) => items.iter().collect(),
        None => json.as_object().map(|map| map.values().filter_map(Value::as_array).flatten().collect()).unwrap_or_default(),
    }
}

fn text(item: &Value, key: &str) -> String {
    item.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: build_dataset <out.db> <crawl.json>..."));
    let mut releases = Vec::new();
    let (mut seen, mut skipped) = (std::collections::HashSet::new(), 0usize);
    for path in args {
        let json: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let before = releases.len();
        for item in items(&json) {
            let (title, magnet, view_url) = (text(item, "title"), text(item, "magnet"), text(item, "url"));
            let Some(id) = nyaa_client::release_id(&view_url) else {
                skipped += 1;
                continue;
            };
            if title.is_empty() || magnet.is_empty() {
                skipped += 1;
                continue;
            }
            if !seen.insert(id) {
                continue;
            }
            let category = item.get("categoryId").and_then(Value::as_str).unwrap_or("1_2").to_string();
            let count = |key: &str| item.get(key).and_then(Value::as_u64).unwrap_or(0) as u32;
            let release = NyaaResult {
                label: EpisodeLabel::of(&title),
                title,
                magnet,
                torrent_url: text(item, "torrent"),
                view_url,
                size: text(item, "size"),
                seeders: count("seeders"),
                leechers: count("leechers"),
                published: text(item, "date"),
            };
            releases.push((category, release));
        }
        tracing::info!(path, added = releases.len() - before, "read crawl file");
    }
    let written = nyaa_client::write_release_database(&out, &releases)?;
    tracing::info!(out = %out.display(), written, skipped, "dataset built");
    Ok(())
}
