// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Anime release title parser: a Rust port of
//! [anitopy](https://github.com/igorcmoura/anitopy) 2.1.1 (itself a port of
//! Anitomy). `parse` splits a title into named elements (anime title,
//! season, episode, release group, resolution, ...). See README.md.

mod element;
mod extra;
mod helper;
mod keyword;
mod number;
mod parser;
mod token;
mod tokenizer;

pub use element::{Category, Elements};

use keyword::{keyword_manager, normalize};
use token::Token;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub allowed_delimiters: &'static str,
}

const DEFAULT_OPTIONS: Options = Options { allowed_delimiters: " _.&+,|" };

/// Parser state for one title (anitopy keeps this in globals).
pub(crate) struct Ctx {
    pub tokens: Vec<Token>,
    pub elements: Elements,
    pub next_id: usize,
    pub options: Options,
}

/// Parses a release title or file name into its elements.
pub fn parse(filename: &str) -> Elements {
    tracing::trace!(filename, "release-parse: parsing");
    let mut ctx = Ctx { tokens: Vec::new(), elements: Elements::default(), next_id: 0, options: DEFAULT_OPTIONS };
    ctx.elements.insert(Category::FileName, filename);
    let (name, extension) = remove_extension_from_filename(filename);
    if let Some(extension) = extension {
        ctx.elements.insert(Category::FileExtension, extension);
    }
    if !name.is_empty() && ctx.tokenize(name) {
        ctx.parse_tokens();
    }
    tracing::trace!(elements = ?ctx.elements, "release-parse: parsed");
    ctx.elements
}

fn remove_extension_from_filename(filename: &str) -> (&str, Option<&str>) {
    let Some((name, extension)) = filename.rsplit_once('.') else { return (filename, None) };
    if extension.chars().count() > 4 || extension.is_empty() || !extension.chars().all(char::is_alphanumeric) {
        return (filename, None);
    }
    if keyword_manager().find(&normalize(extension), Category::FileExtension).is_none() {
        return (filename, None);
    }
    (name, Some(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(e: &Elements, c: Category) -> Option<&str> {
        e.first(c)
    }

    #[test]
    fn anitopy_readme_example() {
        let e = parse("[TaigaSubs]_Toradora!_(2008)_-_01v2_-_Tiger_and_Dragon_[1280x720_H.264_FLAC][1234ABCD].mkv");
        assert_eq!(first(&e, Category::AnimeTitle), Some("Toradora!"));
        assert_eq!(first(&e, Category::AnimeYear), Some("2008"));
        assert_eq!(first(&e, Category::EpisodeNumber), Some("01"));
        assert_eq!(first(&e, Category::ReleaseVersion), Some("2"));
        assert_eq!(first(&e, Category::EpisodeTitle), Some("Tiger and Dragon"));
        assert_eq!(first(&e, Category::ReleaseGroup), Some("TaigaSubs"));
        assert_eq!(first(&e, Category::VideoResolution), Some("1280x720"));
        assert_eq!(first(&e, Category::FileChecksum), Some("1234ABCD"));
        assert_eq!(first(&e, Category::FileExtension), Some("mkv"));
        assert_eq!(e.get(Category::VideoTerm), ["H.264"]);
        assert_eq!(e.get(Category::AudioTerm), ["FLAC"]);
    }

    #[test]
    fn nyaa_titles() {
        let e = parse("[SubsPlease] One Piece - 1180 (1080p) [ADE3A4B9].mkv");
        assert_eq!(first(&e, Category::AnimeTitle), Some("One Piece"));
        assert_eq!(first(&e, Category::EpisodeNumber), Some("1180"));
        let e = parse("[Judas] Shingeki no Kyojin (Attack on Titan) - S04E29-31v2 [1080p][HEVC x265 10bit][Multi-Subs] (Weekly)");
        assert_eq!(e.get(Category::AnimeSeason), ["04"]);
        assert_eq!(e.get(Category::EpisodeNumber), ["29", "31"]);
        let e = parse("[Erai-raws] Jujutsu Kaisen 2nd Season - 01 ~ 23 [1080p][BATCH][Multiple Subtitle]");
        assert_eq!(first(&e, Category::AnimeTitle), Some("Jujutsu Kaisen"));
        assert_eq!(e.get(Category::AnimeSeason), ["2"]);
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;

    #[test]
    fn cjk_episode_and_season_counters() {
        let e = parse("[Doomdos] - Re:ZERO -Starting Life in Another World- Season 4 - \u{7B2C}19\u{8BDD} - [1080p BILIBILI COM WEB-DL]");
        assert_eq!(e.get(Category::EpisodeNumber), ["19"]);
        assert_eq!(e.get(Category::AnimeSeason), ["4"]);
        let e = parse("[CRUCiBLE] Oshi no Ko Season 2 (S02) (BD Remux 1080p FLAC H.264) [Dual Audio] | \u{7B2C}2\u{671F}");
        assert!(e.get(Category::EpisodeNumber).is_empty());
        assert!(e.get(Category::AnimeSeason).iter().all(|s| s.trim_start_matches('0') == "2"));
    }

    fn seasons(title: &str) -> Vec<String> {
        let mut v = parse(title).get(Category::AnimeSeason).iter().map(|s| s.trim_start_matches('0').to_string()).collect::<Vec<_>>();
        v.dedup();
        v
    }

    #[test]
    fn season_ranges() {
        assert_eq!(seasons("[TatakaeFuniSubs] Attack on Titan S01-04 (BD 1080p) [Dual Audio]"), ["1", "2", "3", "4"]);
        assert_eq!(seasons("[Anime Time] Attack On Titan (Ultimate Collection) (S01-S04+OVA+Movies+Junior High) [BD]"), ["1", "2", "3", "4"]);
        assert_eq!(seasons("Shingeki no Kyojin (Attack on Titan) S1-3 Dual-Audio [BDRip 1920x1080 HEVC x265 10bit Dual-Audio]"), ["1", "2", "3"]);
        assert_eq!(seasons("[Furretar] Attack on Titan (Season 1-4 + Kanketsu-hen + OVA + Junior High)"), ["1", "2", "3", "4"]);
        assert_eq!(seasons("[Suki Desu] Shingeki no Kyojin (Attack on Titan) (Season 1 - 3) [BD 1080p][HEVC x265 10bit]"), ["1", "2", "3"]);
        // Not a range: season 3, episode 9.
        let e = parse("[Doomdos] - Mushoku Tensei Jobless Reincarnation Season 3 - 9 [2160p IQ WEB-DL]");
        assert_eq!(e.get(Category::AnimeSeason), ["3"]);
        assert_eq!(e.get(Category::EpisodeNumber), ["9"]);
    }

    #[test]
    fn episode_ranges() {
        let e = parse("[Erai-raws] Jujutsu Kaisen - 01 ~ 24 [1080p][Multiple Subtitle]");
        assert_eq!(e.get(Category::EpisodeNumber), ["01", "24"]);
        let e = parse("[ToonsHub] Attack on Titan - The Final Season Part 3 - S04E29~E31 (JAP 720p x264 AAC) [Multi-Subs]");
        assert_eq!(e.get(Category::EpisodeNumber), ["29", "31"]);
        assert_eq!(e.first(Category::AnimeTitle), Some("Attack on Titan - The Final Season Part 3"));
        let e = parse("Fate\u{FF0F}Zero \u{7B2C}14\u{FF5E}25\u{8A71} (BD 1920x1080 x264 AAC EnJp)");
        assert_eq!(e.get(Category::EpisodeNumber), ["14", "25"]);
        let e = parse("[Whisper] Kaguya-sama: Love Is War - The First Kiss That Never Ends \u{7B2C}\u{FF11}\u{591C} (AI)");
        assert!(e.get(Category::EpisodeNumber).is_empty());
    }

    #[test]
    fn roman_seasons() {
        let e = parse("[Anime Time] Mob Psycho 100 III - 10 [Dual Audio] [1080p][HEVC 10bit x265][AAC][Multi Sub].mkv");
        assert_eq!(e.first(Category::AnimeTitle), Some("Mob Psycho 100"));
        assert_eq!(e.get(Category::AnimeSeason), ["3"]);
        assert_eq!(e.get(Category::EpisodeNumber), ["10"]);
        assert_eq!(seasons("[Erai-raws] Mushoku Tensei III: Isekai Ittara Honki Dasu - 14 [1080p CR WEBRip HEVC AAC][MultiSub]"), ["3"]);
        assert_eq!(seasons("[Erai-raws] Overlord IV - 01 ~ 13 [480p][BATCH][Multiple Subtitle]"), ["4"]);
        // Not seasons
        assert!(seasons("[Beatrice-Raws] Fate Zero Part II [BDRip 1920x1080 HEVC TrueHD]").is_empty());
        assert!(seasons("[MeruMeruSubs] Rockman Irregular Hunter X - The Day of Sigma v2").is_empty());
        assert!(seasons("[Doomdos] - Demon Slayer: Kimetsu no Yaiba Infinity Castle I - 1 - [4K BILIBILI COM WEB-DL]").is_empty());
        assert!(seasons("Final Fantasy VII Advent Children (BD 1080p)").is_empty());
    }
}
