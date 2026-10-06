// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scratch tool: one title per stdin line → one `Release` (Debug) per line,
//! for comparing against the frontend's `episodeParser.ts` labels.
//! `cargo run -p release-parse --example labels < titles.txt`

use std::io::{BufRead, Write};

fn main() {
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for line in std::io::stdin().lock().lines() {
        let r = release_parse::parse_release(&line.expect("utf-8 line"));
        let kind = format!("{:?}", r.kind).to_lowercase();
        let eps = r.episodes.map_or("null".to_string(), |(a, b)| format!("[{a},{b}]"));
        writeln!(out, r#"{{"kind":"{kind}","season":{},"seasons":{:?},"episodes":{eps},"extras":{}}}"#, r.season(), r.seasons, r.extras).unwrap();
    }
}
