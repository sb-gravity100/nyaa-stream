// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Anime release title parser: a Rust port of
//! [anitopy](https://github.com/igorcmoura/anitopy) 2.1.1 (itself a port of
//! Anitomy). `parse` splits a title into named elements (anime title,
//! season, episode, release group, resolution, ...). See README.md.

mod element;
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
