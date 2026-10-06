// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scratch tool: parses one title per stdin line and prints one JSON object
//! of elements per line (`{"anime_title": ["..."], ...}`), for diffing
//! against anitopy or eyeballing titles.
//! `cargo run -p release-parse --example parse_lines < titles.txt`

use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for line in stdin.lock().lines() {
        let line = line.expect("utf-8 line");
        let elements = release_parse::parse(&line);
        let map: serde_json::Map<String, serde_json::Value> = elements.iter().map(|(c, v)| (c.as_str().to_string(), serde_json::json!(v))).collect();
        writeln!(out, "{}", serde_json::Value::Object(map)).unwrap();
    }
}
