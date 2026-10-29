//! Bounded parsing for user supplied BitTorrent magnet links.
use anyhow::{bail, ensure, Result};

const MAX_MAGNET_BYTES: usize = 4096;
const MAX_TRACKERS: usize = 32;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMagnet {
    pub info_hash: String,
    pub name: Option<String>,
    pub trackers: usize,
    pub magnet: String,
}

pub fn parse(input: &str) -> Result<ParsedMagnet> {
    ensure!(input.len() <= MAX_MAGNET_BYTES, "magnet link is too long");
    let input = input.trim();
    let lower = input.to_ascii_lowercase();
    let (info_hash, name, tracker_values) = if lower.starts_with("magnet:?") {
        let query = input.split_once('?').map(|(_, q)| q).unwrap_or_default();
        let mut hash = None;
        let mut name: Option<String> = None;
        let mut trackers: Vec<String> = Vec::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let plus_decoded = value.replace('+', " ");
            let key = urlencoding::decode(key).map_err(|_| anyhow::anyhow!("magnet contains invalid parameter encoding"))?.to_ascii_lowercase();
            let value = urlencoding::decode(&plus_decoded).map_err(|_| anyhow::anyhow!("magnet contains invalid parameter encoding"))?;
            match key.as_str() {
                "xt" if hash.is_none() => {
                    let xt = value.as_ref();
                    if xt.get(..9).is_some_and(|prefix| prefix.eq_ignore_ascii_case("urn:btih:")) {
                        let hex = &xt[9..];
                        hash = Some(normalize_hash(hex)?);
                    }
                }
                "dn" if name.is_none() => {
                    let cleaned = value.trim();
                    if !cleaned.is_empty() { name = Some(cleaned.chars().take(200).collect()); }
                }
                "tr" => {
                    if !value.trim().is_empty() { trackers.push(value.into_owned()); }
                    ensure!(trackers.len() <= MAX_TRACKERS, "magnet link has too many trackers");
                }
                _ => {}
            }
        }
        (hash.ok_or_else(|| anyhow::anyhow!("magnet must include a supported btih hash"))?, name, trackers)
    } else {
        (normalize_hash(input)?, None, Vec::new())
    };
    let mut magnet = format!("magnet:?xt=urn:btih:{info_hash}");
    if let Some(name) = &name { magnet.push_str(&format!("&dn={}", urlencoding::encode(name))); }
    for tracker in &tracker_values { magnet.push_str(&format!("&tr={}", urlencoding::encode(tracker))); }
    Ok(ParsedMagnet { info_hash, name, trackers: tracker_values.len(), magnet })
}

fn normalize_hash(value: &str) -> Result<String> {
    if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(value.to_ascii_lowercase());
    }
    if value.len() == 32 && value.bytes().all(|b| b.is_ascii_alphanumeric()) {
        let mut out = String::with_capacity(40);
        let mut acc: u32 = 0;
        let mut bits = 0;
        for byte in value.bytes() {
            let digit = match byte.to_ascii_uppercase() {
                b'A'..=b'Z' => (byte.to_ascii_uppercase() - b'A') as u32,
                b'2'..=b'7' => (byte - b'2' + 26) as u32,
                _ => bail!("invalid base32 btih hash"),
            };
            acc = (acc << 5) | digit;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                out.push_str(&format!("{:02x}", (acc >> bits) as u8));
                acc &= (1 << bits) - 1;
            }
        }
        ensure!(out.len() == 40, "invalid base32 btih hash");
        return Ok(out);
    }
    bail!("btih hash must be 40 hex or 32 base32 characters")
}
