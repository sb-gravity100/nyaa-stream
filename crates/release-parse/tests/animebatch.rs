use release_parse::{parse_release, Kind};

#[test]
fn supplied_labeled_cases() {
    let cases: Vec<(String, Option<String>, Vec<u32>, Option<u32>, Option<u32>)> =
        serde_json::from_str(include_str!("animebatch_cases.json")).unwrap();
    let mut failures = Vec::new();
    for (raw, batch_kind, seasons, start, end) in cases {
        let got = parse_release(&raw);
        let is_batch = got.kind == Kind::Batch;
        let expected_batch = batch_kind.is_some();
        if got.seasons != seasons || got.episodes != start.zip(end) || is_batch != expected_batch {
            failures.push(format!("{raw}\n  expected: {batch_kind:?} {seasons:?} {start:?}-{end:?}\n  got: {:?}", got));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
