//! Magnet URI fields the engine needs before sbtl parses the magnet.

/// Lowercase hex info-hash from a magnet's `xt=urn:btih:` (hex or base32).
pub(crate) fn info_hash(url: &str) -> Option<String> {
    let xt = url.split(['?', '&']).find_map(|kv| kv.strip_prefix("xt=urn:btih:"))?;
    match xt.len() {
        40 => Some(xt.to_ascii_lowercase()),
        32 => {
            // base32 -> 20 bytes
            let mut bits = 0u64;
            let mut nbits = 0;
            let mut out = Vec::with_capacity(20);
            for c in xt.bytes() {
                let v = match c.to_ascii_uppercase() {
                    b'A'..=b'Z' => c.to_ascii_uppercase() - b'A',
                    b'2'..=b'7' => c - b'2' + 26,
                    _ => return None,
                } as u64;
                bits = bits << 5 | v;
                nbits += 5;
                if nbits >= 8 {
                    nbits -= 8;
                    out.push((bits >> nbits) as u8);
                }
            }
            Some(hex::encode(out))
        }
        _ => None,
    }
}

/// A magnet's `tr=` announce URLs, decoded. A cached .torrent holds only
/// the info dictionary, so these are added explicitly.
pub(crate) fn trackers(url: &str) -> Vec<String> {
    url.split(['?', '&'])
        .filter_map(|kv| kv.strip_prefix("tr="))
        .filter_map(|v| urlencoding::decode(&v.replace('+', " ")).ok().map(|d| d.into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes() {
        assert_eq!(
            super::info_hash("magnet:?xt=urn:btih:DD8255ECDC7CA55FB0BBF81323D87062DB1F6D1C&dn=x").as_deref(),
            Some("dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c")
        );
        // base32 of the same hash
        assert_eq!(
            super::info_hash("magnet:?xt=urn:btih:3WBFL3G4PSSV7MF37AJSHWDQMLNR63I4").as_deref(),
            Some("dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c")
        );
        assert_eq!(super::info_hash("magnet:?dn=x"), None);
    }

    #[test]
    fn tracker_list() {
        let m = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce&dn=x&tr=https%3A%2F%2Fa.b%2Fannounce";
        assert_eq!(
            super::trackers(m),
            vec!["udp://tracker.example:1337/announce".to_string(), "https://a.b/announce".to_string()]
        );
    }
}
