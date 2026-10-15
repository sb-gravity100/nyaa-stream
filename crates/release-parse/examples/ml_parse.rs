// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scratch tool for the learned matcher prototype (ml/): one title per stdin
//! line -> one JSON `Release` per line (show title, group, seasons, episode
//! range, kind, extras), so Python can build features from the app's parser.
//! `cargo run --release -p release-parse --example ml_parse < titles.txt`

use std::io::{BufRead, Write};

fn main() {
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for line in std::io::stdin().lock().lines() {
        let r = release_parse::parse_release(&line.expect("utf-8 line"));
        let v = serde_json::json!({
            "title": r.title,
            "group": r.group,
            "seasons": r.seasons,
            "episodes": r.episodes,
            "kind": format!("{:?}", r.kind).to_lowercase(),
            "extras": r.extras,
        });
        writeln!(out, "{v}").unwrap();
    }
}
