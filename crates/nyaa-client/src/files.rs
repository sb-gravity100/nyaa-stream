//! A torrent's file list as nyaa.si's view page shows it, with each video
//! file's season / episode read from its name (or, failing that, from the
//! folder it sits in: `Season 2/[Reza] Food Wars - 03.mkv`).

use serde::{Deserialize, Serialize};

/// Extensions treated as video (lowercase, no dot).
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "webm", "m4v", "mov", "ts", "wmv"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TorrentFile {
    /// Folders and file name joined with `/`.
    pub path: String,
    /// Bytes (nyaa.si only shows a rounded KiB/MiB/GiB/TiB figure).
    pub size: u64,
    /// Position among all files, in the order the view page lists them.
    pub index: usize,
    /// Video files only. `None` = no season marker anywhere (unknown).
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
}

/// `"1.2 GiB"` -> bytes. nyaa.si writes `Bytes`, `KiB`, `MiB`, `GiB`, `TiB`.
pub(crate) fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim().trim_start_matches('(').trim_end_matches(')').trim();
    let (number, unit) = text.split_once(char::is_whitespace)?;
    let number: f64 = number.replace(',', "").parse().ok()?;
    let factor: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "bytes" | "byte" | "b" => 1.0,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * factor).round() as u64)
}

fn is_video(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Builds the file list from `(path, size)` pairs in listed order, and the
/// ascending, deduplicated union of the video files' seasons.
pub(crate) fn build_files(raw: Vec<(String, u64)>) -> (Vec<TorrentFile>, Vec<u32>) {
    let mut seasons = Vec::new();
    let files: Vec<TorrentFile> = raw
        .into_iter()
        .enumerate()
        .map(|(index, (path, size))| {
            let (season, episode) = if is_video(&path) { video_season_episode(&path) } else { (None, None) };
            if let Some(season) = season {
                seasons.push(season);
            }
            tracing::trace!(path, ?season, ?episode, "parsed torrent file");
            TorrentFile { path, size, index, season, episode }
        })
        .collect();
    seasons.sort_unstable();
    seasons.dedup();
    (files, seasons)
}

/// Season and episode of one video file path.
fn video_season_episode(path: &str) -> (Option<u32>, Option<u32>) {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or(path);
    let release = release_parse::parse_release(name);
    let episode = release.episodes.map(|(first, _)| first);
    let mut season = release.seasons.first().copied();
    if season.is_none() {
        // Nearest folder that names a season wins.
        season = parts.iter().rev().find_map(|folder| release_parse::parse_release(folder).seasons.first().copied());
        if season.is_some() {
            tracing::debug!(path, ?season, "file season taken from its folder");
        }
    }
    (season, episode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("(2.0 GiB)"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size("1.5 MiB"), Some(1572864));
        assert_eq!(parse_size("512 Bytes"), Some(512));
        assert_eq!(parse_size("3 KiB"), Some(3072));
        assert_eq!(parse_size("1 TiB"), Some(1u64 << 40));
        assert_eq!(parse_size("lots"), None);
    }

    fn one(path: &str) -> (Option<u32>, Option<u32>) {
        let (files, _) = build_files(vec![(path.to_string(), 1)]);
        (files[0].season, files[0].episode)
    }

    #[test]
    fn reads_season_and_episode() {
        let cases: &[(&str, Option<u32>, Option<u32>)] = &[
            // Marker in the name.
            ("[Reza] Food Wars/Season 1/[Reza] Food Wars S01E01.mkv", Some(1), Some(1)),
            ("[Reza] Food Wars/Season 5/[Reza] Food Wars S05E13.mkv", Some(5), Some(13)),
            // Name wins over a contradicting folder.
            ("Season 1/Show S02E03.mkv", Some(2), Some(3)),
            // Folder supplies the season.
            ("Season 2/[Grp] Show - 03 [1080p].mkv", Some(2), Some(3)),
            ("Show/S02/[Grp] Show - 04.mkv", Some(2), Some(4)),
            ("Show/Season 02/[Grp] Show - 05.mkv", Some(2), Some(5)),
            ("Show/2nd Season/[Grp] Show - 06.mkv", Some(2), Some(6)),
            // Nearest folder wins when nested.
            ("Show/Season 1/Season 3/[Grp] Show - 07.mkv", Some(3), Some(7)),
            // No season anywhere: unknown.
            ("[Grp] Show - 07 [1080p].mkv", None, Some(7)),
            ("[Grp] Show Batch/[Grp] Show - 08.mkv", None, Some(8)),
            // Non-video files carry nothing.
            ("Season 2/Show S02E01.srt", None, None),
            ("Season 2/cover.jpg", None, None),
            ("Season 2/Show S02E01.nfo", None, None),
        ];
        for (path, season, episode) in cases {
            assert_eq!(one(path), (*season, *episode), "{path}");
        }
    }

    #[test]
    fn collects_indexes_and_season_union() {
        let raw = vec![
            ("Show/Season 2/Show S02E01.mkv".to_string(), 10),
            ("Show/Season 2/Show S02E02.MKV".to_string(), 11),
            ("Show/Season 1/Show S01E01.mkv".to_string(), 12),
            ("Show/readme.txt".to_string(), 1),
            ("Show/[Grp] Show - 09.mkv".to_string(), 13),
        ];
        let (files, seasons) = build_files(raw);
        assert_eq!(seasons, vec![1, 2]);
        assert_eq!(files.iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        assert_eq!(files[3].season, None);
        assert_eq!(files[4].season, None);
        assert_eq!(files[4].episode, Some(9));
        assert_eq!(files[1].season, Some(2));
        assert_eq!(files[2].size, 12);
    }
}
