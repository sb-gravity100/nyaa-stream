// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's parser_number.py.

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::element::Category;
use crate::helper::{find_non_number_in_string, find_number_in_string, py_int, py_isdigit, str2int, strip_chars};
use crate::keyword::{keyword_manager, normalize};
use crate::token::{flags, TokenCategory, TokenId};
use crate::Ctx;

pub(crate) const ANIME_YEAR_MIN: i64 = 1900;
pub(crate) const ANIME_YEAR_MAX: i64 = 2050;
const EPISODE_NUMBER_MAX: i64 = ANIME_YEAR_MIN - 1;
const VOLUME_NUMBER_MAX: i64 = 20;

macro_rules! re {
    ($pattern:expr) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($pattern).unwrap())
    }};
}

fn group<'a>(c: &'a Captures, i: usize) -> Option<&'a str> {
    c.get(i).map(|m| m.as_str())
}

fn is_valid_episode_number(number: &str) -> bool {
    str2int(number) <= EPISODE_NUMBER_MAX
}

fn first_char_is_digit(s: &str) -> bool {
    s.chars().next().is_some_and(crate::helper::is_py_digit)
}

fn last_char_is_digit(s: &str) -> bool {
    s.chars().next_back().is_some_and(crate::helper::is_py_digit)
}

impl Ctx {
    pub(crate) fn set_episode_number(&mut self, number: &str, token: TokenId, validate: bool) -> bool {
        if validate && !is_valid_episode_number(number) {
            return false;
        }
        self.tok_mut(token).category = TokenCategory::Identifier;
        let mut category = Category::EpisodeNumber;
        // Equivalent numbers, e.g. "01 (176)"
        if self.elements.check_alt_number {
            let episode_number = self.elements.first(Category::EpisodeNumber).unwrap_or_default().to_string();
            if str2int(number) > str2int(&episode_number) {
                category = Category::EpisodeNumberAlt;
            } else if str2int(number) < str2int(&episode_number) {
                self.elements.remove(Category::EpisodeNumber, &episode_number);
                self.elements.insert(Category::EpisodeNumberAlt, episode_number);
            } else {
                return false;
            }
        }
        self.elements.insert(category, number);
        true
    }

    fn set_alternative_episode_number(&mut self, number: &str, token: TokenId) {
        self.elements.insert(Category::EpisodeNumberAlt, number);
        self.tok_mut(token).category = TokenCategory::Identifier;
    }

    pub(crate) fn check_extent_keyword(&mut self, category: Category, token: TokenId) -> bool {
        let Some(next) = self.find_next(Some(token), flags::NOT_DELIMITER) else { return false };
        if self.tok(next).category != TokenCategory::Unknown {
            return false;
        }
        let content = self.tok(next).content.clone();
        if find_number_in_string(&content).is_none() {
            return false;
        }
        match category {
            Category::EpisodeNumber => {
                if !self.match_episode_patterns(&content, next) {
                    self.set_episode_number(&content, next, false);
                }
            }
            Category::VolumeNumber => {
                if !self.match_volume_patterns(&content, next) {
                    self.set_volume_number(&content, next, false);
                }
            }
            _ => return false,
        }
        self.tok_mut(token).category = TokenCategory::Identifier;
        true
    }

    fn number_comes_after_prefix(&mut self, category: Category, token: TokenId) -> bool {
        let content = self.tok(token).content.clone();
        let Some(number_begin) = find_number_in_string(&content) else { return false };
        let prefix = &content[..number_begin];
        if keyword_manager().find(&normalize(prefix), category).is_none() {
            return false;
        }
        let number = content[number_begin..].to_string();
        match category {
            Category::EpisodePrefix => self.match_episode_patterns(&number, token) || self.set_episode_number(&number, token, false),
            Category::VolumePrefix => self.match_volume_patterns(&number, token) || self.set_volume_number(&number, token, false),
            Category::AnimeSeasonPrefix => self.set_season_number(&number, token),
            _ => false,
        }
    }

    fn number_comes_before_another_number(&mut self, token: TokenId) -> bool {
        let Some(separator) = self.find_next(Some(token), flags::NOT_DELIMITER) else { return false };
        let sep = self.tok(separator).content.clone();
        if sep != "&" && sep != "of" {
            return false;
        }
        let Some(other) = self.find_next(Some(separator), flags::NOT_DELIMITER) else { return false };
        let other_content = self.tok(other).content.clone();
        if !py_isdigit(&other_content) {
            return false;
        }
        let content = self.tok(token).content.clone();
        self.set_episode_number(&content, token, false);
        if sep == "&" {
            self.set_episode_number(&other_content, token, false);
        }
        self.tok_mut(separator).category = TokenCategory::Identifier;
        self.tok_mut(other).category = TokenCategory::Identifier;
        true
    }

    pub(crate) fn search_for_episode_patterns(&mut self, tokens: &[TokenId]) -> bool {
        for &token in tokens {
            let content = self.tok(token).content.clone();
            if !first_char_is_digit(&content) {
                // e.g. "EP.1", "Vol.1"
                if self.number_comes_after_prefix(Category::EpisodePrefix, token) {
                    return true;
                }
                if self.number_comes_after_prefix(Category::VolumePrefix, token) {
                    continue;
                }
                if self.number_comes_after_prefix(Category::AnimeSeasonPrefix, token) {
                    continue;
                }
            } else if self.number_comes_before_another_number(token) {
                // e.g. "8 & 10", "01 of 24"
                return true;
            }
            let content = self.tok(token).content.clone();
            if self.match_episode_patterns(&content, token) {
                return true;
            }
        }
        false
    }

    fn match_single_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let Some(c) = re!(r"^(\d{1,3})[vV](\d)$").captures(word) else { return false };
        let (n, v) = (c[1].to_string(), c[2].to_string());
        self.set_episode_number(&n, token, false);
        self.elements.insert(Category::ReleaseVersion, v);
        true
    }

    fn match_multi_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let Some(c) = re!(r"^(\d{1,3})(?:[vV](\d))?[-~&+](\d{1,3})(?:[vV](\d))?$").captures(word) else { return false };
        let (lower, upper) = (c[1].to_string(), c[3].to_string());
        let (v1, v2) = (group(&c, 2).map(str::to_string), group(&c, 4).map(str::to_string));
        // Avoid matching expressions such as "009-1" or "5-2"
        if py_int(&lower) < py_int(&upper) && self.set_episode_number(&lower, token, true) {
            self.set_episode_number(&upper, token, false);
            if let Some(v) = v1 {
                self.elements.insert(Category::ReleaseVersion, v);
            }
            if let Some(v) = v2 {
                self.elements.insert(Category::ReleaseVersion, v);
            }
            return true;
        }
        false
    }

    fn match_season_and_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let re = re!(r"(?i)^S?(\d{1,2})(?:-S?(\d{1,2}))?(?:x|[ ._-x]?E)(\d{1,3})(?:-E?(\d{1,3}))?(?:[vV](\d))?$");
        let Some(c) = re.captures(word) else { return false };
        let g: Vec<Option<String>> = (0..6).map(|i| group(&c, i).map(str::to_string)).collect();
        self.elements.insert(Category::AnimeSeason, g[1].clone().unwrap());
        if let Some(s) = &g[2] {
            self.elements.insert(Category::AnimeSeason, s.clone());
        }
        self.set_episode_number(g[3].as_deref().unwrap(), token, false);
        if let Some(e) = &g[4] {
            self.set_episode_number(e, token, false);
        }
        if let Some(v) = &g[5] {
            self.elements.insert(Category::ReleaseVersion, v.clone());
        }
        true
    }

    fn match_type_and_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let Some(number_begin) = find_number_in_string(word) else { return false };
        let prefix = &word[..number_begin];
        let Some(keyword) = keyword_manager().find(&normalize(prefix), Category::AnimeType) else { return false };
        self.elements.insert(Category::AnimeType, prefix);
        let number = word[number_begin..].to_string();
        if self.match_episode_patterns(&number, token) || self.set_episode_number(&number, token, true) {
            // Split the token, prefix first.
            let index = self.idx(token);
            let enclosed = self.tok(token).enclosed;
            self.tok_mut(token).content = number;
            let category = if keyword.identifiable { TokenCategory::Identifier } else { TokenCategory::Unknown };
            self.insert_token(index, category, prefix, enclosed);
            return true;
        }
        false
    }

    fn match_fractional_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        // Only ".5": other fractions are titles ("Evangelion: 1.11") or
        // keywords ("5.1").
        re!(r"^\d+\.5$").is_match(word) && self.set_episode_number(word, token, true)
    }

    fn match_partial_episode_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let suffix = match find_non_number_in_string(word) {
            Some(i) => &word[i..],
            None => word,
        };
        let valid_suffix = suffix.chars().count() == 1 && "ABCabc".contains(suffix);
        valid_suffix && self.set_episode_number(word, token, true)
    }

    fn match_number_sign_pattern(&mut self, word: &str, token: TokenId) -> bool {
        if !word.starts_with('#') {
            return false;
        }
        let Some(c) = re!(r"^#(\d{1,3})(?:[-~&+](\d{1,3}))?(?:[vV](\d))?$").captures(word) else { return false };
        let (a, b, v) = (c[1].to_string(), group(&c, 2).map(str::to_string), group(&c, 3).map(str::to_string));
        if self.set_episode_number(&a, token, true) {
            if let Some(b) = b {
                self.set_episode_number(&b, token, true);
            }
            if let Some(v) = v {
                self.elements.insert(Category::ReleaseVersion, v);
            }
            return true;
        }
        false
    }

    fn match_japanese_counter_pattern(&mut self, word: &str, token: TokenId) -> bool {
        if !word.ends_with('\u{8A71}') {
            return false;
        }
        let Some(c) = re!("^(\\d{1,3})\u{8A71}$").captures(word) else { return false };
        let n = c[1].to_string();
        self.set_episode_number(&n, token, false)
    }

    pub(crate) fn match_episode_patterns(&mut self, word: &str, token: TokenId) -> bool {
        if py_isdigit(word) {
            return false;
        }
        let word = strip_chars(word, " -").to_string();
        if word.is_empty() {
            return false;
        }
        let front = first_char_is_digit(&word);
        let back = last_char_is_digit(&word);

        // e.g. "01v2"
        if front && back && self.match_single_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "01-02", "03-05v2"
        if front && back && self.match_multi_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "2x01", "S01E03", "S01-02xE001-150", "S01E06v2"
        if back && self.match_season_and_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "ED1", "OP4a", "OVA2"
        if !front && self.match_type_and_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "07.5"
        if front && back && self.match_fractional_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "4a", "111C"
        if front && !back && self.match_partial_episode_pattern(&word, token) {
            return true;
        }
        // e.g. "#01", "#02-03v2"
        if back && self.match_number_sign_pattern(&word, token) {
            return true;
        }
        // U+8A71 counts episodes
        front && self.match_japanese_counter_pattern(&word, token)
    }

    pub(crate) fn set_volume_number(&mut self, number: &str, token: TokenId, validate: bool) -> bool {
        if validate && py_int(number).is_none_or(|n| n > VOLUME_NUMBER_MAX) {
            return false;
        }
        self.elements.insert(Category::VolumeNumber, number);
        self.tok_mut(token).category = TokenCategory::Identifier;
        true
    }

    fn match_single_volume_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let Some(c) = re!(r"^(\d{1,2})[vV](\d)$").captures(word) else { return false };
        let (n, v) = (c[1].to_string(), c[2].to_string());
        self.set_volume_number(&n, token, false);
        self.elements.insert(Category::ReleaseVersion, v);
        true
    }

    fn match_multi_volume_pattern(&mut self, word: &str, token: TokenId) -> bool {
        let Some(c) = re!(r"^(\d{1,2})[-~&+](\d{1,2})(?:[vV](\d))?$").captures(word) else { return false };
        let (lower, upper, v) = (c[1].to_string(), c[2].to_string(), group(&c, 3).map(str::to_string));
        if py_int(&lower) < py_int(&upper) && self.set_volume_number(&lower, token, true) {
            self.set_volume_number(&upper, token, false);
            if let Some(v) = v {
                self.elements.insert(Category::ReleaseVersion, v);
            }
            return true;
        }
        false
    }

    pub(crate) fn match_volume_patterns(&mut self, word: &str, token: TokenId) -> bool {
        if py_isdigit(word) {
            return false;
        }
        let word = strip_chars(word, " -").to_string();
        if word.is_empty() {
            return false;
        }
        let front = first_char_is_digit(&word);
        let back = last_char_is_digit(&word);
        if front && back && self.match_single_volume_pattern(&word, token) {
            return true;
        }
        front && back && self.match_multi_volume_pattern(&word, token)
    }

    fn set_season_number(&mut self, number: &str, token: TokenId) -> bool {
        if !py_isdigit(number) {
            return false;
        }
        self.elements.insert(Category::AnimeSeason, number);
        self.tok_mut(token).category = TokenCategory::Identifier;
        true
    }

    /// e.g. "01 (176)", "29 (04)"
    pub(crate) fn search_for_equivalent_numbers(&mut self, tokens: &[TokenId]) -> bool {
        for &token in tokens {
            let content = self.tok(token).content.clone();
            if self.is_token_isolated(token) || !is_valid_episode_number(&content) {
                continue;
            }
            let next = self.find_next(Some(token), flags::NOT_DELIMITER);
            if self.category_of(next) != Some(TokenCategory::Bracket) {
                continue;
            }
            let next = self.find_next(next, flags::ENCLOSED | flags::NOT_DELIMITER);
            if self.category_of(next) != Some(TokenCategory::Unknown) {
                continue;
            }
            let next = next.unwrap();
            let next_content = self.tok(next).content.clone();
            if !self.is_token_isolated(next) || !py_isdigit(&next_content) || !is_valid_episode_number(&next_content) {
                continue;
            }
            let (a, b) = (str2int(&content), str2int(&next_content));
            // Python's min/max keep the first on ties.
            let (episode, alt) = if b < a { (next, token) } else { (token, next) };
            let (episode, alt) = if a == b { (token, token) } else { (episode, alt) };
            let ec = self.tok(episode).content.clone();
            let ac = self.tok(alt).content.clone();
            self.set_episode_number(&ec, episode, false);
            self.set_alternative_episode_number(&ac, alt);
            return true;
        }
        false
    }

    /// e.g. " - 08"
    pub(crate) fn search_for_separated_numbers(&mut self, tokens: &[TokenId]) -> bool {
        for &token in tokens {
            let previous = self.find_previous(Some(token), flags::NOT_DELIMITER);
            if self.category_of(previous) == Some(TokenCategory::Unknown) && crate::helper::is_dash_character(&self.content(previous)) {
                let content = self.tok(token).content.clone();
                if self.set_episode_number(&content, token, true) {
                    self.tok_mut(previous.unwrap()).category = TokenCategory::Identifier;
                    return true;
                }
            }
        }
        false
    }

    /// e.g. "[12]", "(2006)"
    pub(crate) fn search_for_isolated_numbers_episode(&mut self, tokens: &[TokenId]) -> bool {
        for &token in tokens {
            if !self.tok(token).enclosed || !self.is_token_isolated(token) {
                continue;
            }
            let content = self.tok(token).content.clone();
            if self.set_episode_number(&content, token, true) {
                return true;
            }
        }
        false
    }

    pub(crate) fn search_for_last_number(&mut self, tokens: &[TokenId]) -> bool {
        for &token in tokens {
            let index = self.idx(token);
            // The episode number comes after the title, so not first.
            if index == 0 || self.tok(token).enclosed {
                continue;
            }
            // Not the first non-enclosed, non-delimiter token either.
            if self.tokens[..index].iter().all(|t| t.enclosed || t.category == TokenCategory::Delimiter) {
                continue;
            }
            // Not after "Movie" or "Part".
            let previous = self.find_previous(Some(token), flags::NOT_DELIMITER);
            if self.category_of(previous) == Some(TokenCategory::Unknown) {
                let p = self.content(previous).to_lowercase();
                if p == "movie" || p == "part" {
                    continue;
                }
            }
            let content = self.tok(token).content.clone();
            if self.set_episode_number(&content, token, true) {
                return true;
            }
        }
        false
    }
}
