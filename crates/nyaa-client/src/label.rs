//! What a release title says it contains, in the shape the frontend's
//! `EpisodeLabel` (`src/episodeParser.ts`) uses, parsed by the
//! `release-parse` crate (anitopy port + nyaa fixes, see TITLE_ANALYSIS.md).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EpisodeLabel {
    Episode {
        season: u32,
        number: u32,
        /// Every season the title names (usually just `season`).
        seasons: Vec<u32>,
    },
    Batch {
        season: u32,
        #[serde(rename = "episodeRange")]
        episode_range: Option<[u32; 2]>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        extras: bool,
        seasons: Vec<u32>,
    },
    Movie,
    /// OVA / OAD / ONA / special.
    Special,
    #[default]
    Unknown,
}

impl EpisodeLabel {
    /// Parses a release title or a file name.
    pub fn of(title: &str) -> EpisodeLabel {
        let r = release_parse::parse_release(title);
        let season = r.season();
        let label = match r.kind {
            release_parse::Kind::Episode => match r.episodes {
                Some((number, _)) => EpisodeLabel::Episode { season, number, seasons: r.seasons },
                None => EpisodeLabel::Unknown,
            },
            release_parse::Kind::Batch => EpisodeLabel::Batch {
                season,
                episode_range: r.episodes.filter(|(a, b)| a != b).map(|(a, b)| [a, b]),
                extras: r.extras,
                seasons: r.seasons,
            },
            release_parse::Kind::Movie => EpisodeLabel::Movie,
            release_parse::Kind::Special => EpisodeLabel::Special,
            release_parse::Kind::Unknown => EpisodeLabel::Unknown,
        };
        tracing::trace!(title, ?label, "labelled release");
        label
    }
}

impl EpisodeLabel {
    /// Labels a file inside a torrent from its path. The file name is parsed
    /// as `of` does; an episode or batch whose name carries no season takes
    /// the nearest parent folder that names one ("Season 2/[Group] Show -
    /// 05.mkv"), as season packs are commonly laid out.
    pub fn of_path(path: &str) -> EpisodeLabel {
        let mut parts = path.rsplit(['/', '\\']);
        let name = parts.next().unwrap_or(path);
        let mut label = EpisodeLabel::of(name);
        let (season, seasons) = match &mut label {
            EpisodeLabel::Episode { season, seasons, .. } | EpisodeLabel::Batch { season, seasons, .. } => (season, seasons),
            _ => return label,
        };
        if seasons.is_empty() {
            if let Some(folder_season) = parts.find_map(|folder| release_parse::parse_release(folder).seasons.first().copied()) {
                tracing::trace!(path, folder_season, "file season taken from its folder");
                *season = folder_season;
                *seasons = vec![folder_season];
            }
        }
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_like_the_frontend_type() {
        let json = serde_json::to_value(EpisodeLabel::of("[SubsPlease] Mob Psycho 100 III - 10 (1080p)")).unwrap();
        assert_eq!(json, serde_json::json!({"kind": "episode", "season": 3, "number": 10, "seasons": [3]}));
        let json = serde_json::to_value(EpisodeLabel::of("[Erai-raws] Jujutsu Kaisen - 01 ~ 24 [1080p]")).unwrap();
        assert_eq!(json, serde_json::json!({"kind": "batch", "season": 1, "episodeRange": [1, 24], "seasons": []}));
        let json = serde_json::to_value(EpisodeLabel::of("[ASW] Shingeki no Kyojin Movie - The Last Attack [1080p]")).unwrap();
        assert_eq!(json, serde_json::json!({"kind": "movie"}));
    }

    #[test]
    fn files_take_their_season_from_a_folder() {
        let label = EpisodeLabel::of_path("[Reza] Food Wars/Season 2/[Reza] Food Wars - 05.mkv");
        assert_eq!(label, EpisodeLabel::Episode { season: 2, number: 5, seasons: vec![2] });
        // The file's own marker wins over its folder.
        let label = EpisodeLabel::of_path("Pack/Season 2/Show S03E05.mkv");
        assert_eq!(label, EpisodeLabel::Episode { season: 3, number: 5, seasons: vec![3] });
        // No season anywhere: unchanged from `of`.
        assert_eq!(EpisodeLabel::of_path("Pack/Show - 05.mkv"), EpisodeLabel::of("Show - 05.mkv"));
    }
}
