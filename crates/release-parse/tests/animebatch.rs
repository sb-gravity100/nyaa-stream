use release_parse::{parse_batch, parse_release, search_batch, BatchKind, Kind, SearchOptions};

#[test]
fn supplied_labeled_cases() {
    let cases: Vec<(String, Option<String>, Vec<u32>, Option<u32>, Option<u32>)> =
        serde_json::from_str(include_str!("animebatch_cases.json")).unwrap();
    let mut failures = Vec::new();
    for (raw, batch_kind, seasons, start, end) in cases {
        let got = parse_release(&raw);
        let is_batch = got.kind == Kind::Batch;
        let expected_batch = batch_kind.is_some();
        let detailed = parse_batch(&raw);
        let expected_kind = batch_kind.as_deref().map(|k| match k {
            "multi_season" => BatchKind::MultiSeason, "complete" => BatchKind::Complete,
            "box_set" => BatchKind::BoxSet, "episode_range" => BatchKind::EpisodeRange,
            "movie_pack" => BatchKind::MoviePack, "season_pack" => BatchKind::SeasonPack,
            "batch_tag" => BatchKind::BatchTag, "disc_pack" => BatchKind::DiscPack,
            _ => panic!("unknown kind: {k}"),
        });
        if got.seasons != seasons || got.episodes != start.zip(end) || is_batch != expected_batch || detailed.batch_kind != expected_kind {
            failures.push(format!("{raw}\n  expected: {batch_kind:?} {seasons:?} {start:?}-{end:?}\n  got: {:?} {:?}", got, detailed.batch_kind));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn supplied_search_season_filter() {
    let titles = [
        "[SubsPlease] Tantei wa Mou, Shindeiru. S2 - 03 (480p) [DABACC21].mkv",
        "[Doomdos] - Tantei wa Mou, Shindeiru. - S01E08 [1080p IQIYI WEB-DL] [The Detective Is Already Dead]",
        "[ASW] Tantei wa Mou, Shindeiru. S2 - 04 [1080p HEVC x265 10Bit][AAC]",
        "[X] Tantei wa Mou, Shindeiru. Complete Series [BD 1080p]",
        "[Y] Tantei wa Mou, Shindeiru. Season 1-2 [BD 1080p]",
        "[Z] Some Other Show S2 - 03 [1080p]",
    ];
    let hits = search_batch(titles.iter().copied(), "Tantei wa Mou, Shindeiru", SearchOptions { season: Some(2), ..Default::default() });
    let mut reasons: Vec<_> = hits.iter().map(|m| m.reason).collect();
    reasons.sort();
    assert_eq!(reasons, ["complete", "multi_season", "season", "season"]);
}

#[test]
fn supplied_bilingual_alias() {
    let raw = "[LoliHouse] 炒翻天 / 铁锅料理王 / Tetsunabe no Jan! [01-12 合集][WebRip 1080p HEVC-10bit AAC][简繁内封字幕][Fin]";
    let release = parse_batch(raw);
    assert_eq!(release.release.episodes, Some((1, 12)));
    assert!(matches!(release.batch_kind, Some(BatchKind::Complete | BatchKind::EpisodeRange)));
    assert!(!search_batch([raw], "Tetsunabe no Jan", SearchOptions::default()).is_empty());
}

#[test]
fn oracle_metadata_report() {
    let oracle: Vec<serde_json::Value> = serde_json::from_str(include_str!("animebatch_oracle.json")).unwrap();
    let mut differences = Vec::new();
    for expected in oracle {
        let raw = expected["raw"].as_str().unwrap();
        let got = parse_batch(raw);
        for (field, actual) in [
            ("title", Some(got.title.as_str())),
            ("resolution", got.resolution.as_deref()),
            ("source", got.source.as_deref()),
        ] {
            if expected[field].as_str() != actual {
                differences.push(format!("{raw}\n  {field}: expected {:?}, got {actual:?}", expected[field]));
            }
        }
        for (field, actual) in [
            ("season_source", got.season_source),
        ] {
            if expected[field].as_str() != Some(actual) {
                differences.push(format!("{raw}\n  {field}: expected {:?}, got {actual}", expected[field]));
            }
        }
        for (field, actual) in [
            ("is_complete", got.is_complete), ("has_movies", got.has_movies), ("has_extras", got.has_extras),
        ] {
            if expected[field].as_bool() != Some(actual) {
                differences.push(format!("{raw}\n  {field}: expected {:?}, got {actual}", expected[field]));
            }
        }
        if expected["ep_total"].as_u64() != got.ep_total.map(u64::from) {
            differences.push(format!("{raw}\n  ep_total: expected {:?}, got {:?}", expected["ep_total"], got.ep_total));
        }
    }
    assert!(differences.is_empty(), "{} metadata differences\n{}", differences.len(), differences.join("\n"));
}
