// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's parser.py.

use crate::element::Category;
use crate::helper::{find_number_in_string, is_crc32, is_dash_character, is_mostly_latin_string, is_resolution, py_int, py_isdigit, strip_chars};
use crate::keyword::{keyword_manager, normalize};
use crate::number::{ANIME_YEAR_MAX, ANIME_YEAR_MIN};
use crate::token::{flags, TokenCategory};
use crate::Ctx;

impl Ctx {
    pub(crate) fn parse_tokens(&mut self) -> bool {
        self.search_for_keywords();
        self.search_for_cjk_seasons();
        self.search_for_season_ranges();
        self.search_for_roman_seasons();
        self.search_for_technical_numbers();
        self.search_for_isolated_numbers();
        self.search_for_episode_number();
        self.search_for_anime_title();
        if !self.elements.contains(Category::ReleaseGroup) {
            self.search_for_release_group();
        }
        if self.elements.contains(Category::EpisodeNumber) {
            self.search_for_episode_title();
        }
        self.validate_elements();
        self.elements.iter().next().is_some()
    }

    fn search_for_keywords(&mut self) {
        for id in self.get_list(Some(flags::UNKNOWN), None, None) {
            let content = self.tok(id).content.clone();
            let mut word = strip_chars(&content, " -").to_string();
            if word.is_empty() {
                continue;
            }
            // Don't bother if the word is a number that cannot be a CRC
            if word.chars().count() != 8 && py_isdigit(&word) {
                continue;
            }

            let mut category = Category::Unknown;
            let keyword = keyword_manager().find(&normalize(&word), Category::Unknown);
            if let Some(keyword) = keyword {
                category = keyword.category;
                if !category.is_searchable() || !keyword.searchable {
                    continue;
                }
                if category.is_singular() && self.elements.contains(category) {
                    continue;
                }
                match category {
                    Category::AnimeSeasonPrefix => {
                        self.check_anime_season_keyword(id);
                        continue;
                    }
                    Category::EpisodePrefix => {
                        if keyword.valid {
                            self.check_extent_keyword(Category::EpisodeNumber, id);
                        }
                        continue;
                    }
                    Category::ReleaseVersion => word = word.chars().skip(1).collect(), // number without "v"
                    Category::VolumePrefix => {
                        self.check_extent_keyword(Category::VolumeNumber, id);
                        continue;
                    }
                    _ => {}
                }
            } else if !self.elements.contains(Category::FileChecksum) && is_crc32(&word) {
                category = Category::FileChecksum;
            } else if !self.elements.contains(Category::VideoResolution) && is_resolution(&word) {
                category = Category::VideoResolution;
            }

            if category != Category::Unknown {
                self.elements.insert(category, word);
                if keyword.is_none_or(|k| k.identifiable) {
                    self.tok_mut(id).category = TokenCategory::Identifier;
                }
            }
        }
    }

    fn search_for_isolated_numbers(&mut self) {
        for id in self.get_list(Some(flags::UNKNOWN), None, None) {
            let content = self.tok(id).content.clone();
            if !py_isdigit(&content) || !self.is_token_isolated(id) {
                continue;
            }
            let Some(number) = py_int(&content) else { continue };
            if (ANIME_YEAR_MIN..=ANIME_YEAR_MAX).contains(&number) && !self.elements.contains(Category::AnimeYear) {
                self.elements.insert(Category::AnimeYear, content);
                self.tok_mut(id).category = TokenCategory::Identifier;
                continue;
            }
            // Isolated, these are more likely the resolution than the
            // episode number (some groups drop the "p").
            if (number == 480 || number == 720 || number == 1080) && !self.elements.contains(Category::VideoResolution) {
                self.elements.insert(Category::VideoResolution, content);
                self.tok_mut(id).category = TokenCategory::Identifier;
            }
        }
    }

    fn search_for_episode_number(&mut self) {
        let tokens: Vec<_> = self.get_list(Some(flags::UNKNOWN), None, None).into_iter().filter(|&id| find_number_in_string(&self.tok(id).content).is_some()).collect();
        if tokens.is_empty() {
            return;
        }
        self.elements.check_alt_number = self.elements.contains(Category::EpisodeNumber);

        // nyaa-stream: "01 ~ 25"
        if self.search_for_spaced_ranges(&tokens) {
            return;
        }
        // A known episode pattern has to be the episode number.
        if self.search_for_episode_patterns(&tokens) {
            return;
        }
        if self.elements.contains(Category::EpisodeNumber) {
            // nyaa-stream: "4th Season - 21 [...] | Episode 93" - the dash
            // number is the season-relative one; set_episode_number keeps
            // the smaller as the episode and the other as the alt.
            let numeric: Vec<_> = tokens.iter().copied().filter(|&id| py_isdigit(&self.tok(id).content)).collect();
            self.search_for_separated_numbers(&numeric);
            return; // found via keywords
        }

        // Only numeric tokens from here on.
        let tokens: Vec<_> = tokens.into_iter().filter(|&id| py_isdigit(&self.tok(id).content)).collect();
        if tokens.is_empty() {
            return;
        }
        if self.search_for_equivalent_numbers(&tokens) {
            return;
        }
        if self.search_for_separated_numbers(&tokens) {
            return;
        }
        if self.search_for_isolated_numbers_episode(&tokens) {
            return;
        }
        self.search_for_last_number(&tokens);
    }

    fn search_for_anime_title(&mut self) {
        let mut enclosed_title = false;
        // The first non-enclosed unknown token...
        let mut token_begin = self.find(flags::NOT_ENCLOSED | flags::UNKNOWN);
        // ...else the first unknown token of the second enclosed group,
        // assuming the first one is the release group.
        if token_begin.is_none() {
            enclosed_title = true;
            token_begin = self.tokens.first().map(|t| t.id);
            let mut skipped_previous_group = false;
            while token_begin.is_some() {
                token_begin = self.find_next(token_begin, flags::UNKNOWN);
                let Some(begin) = token_begin else { break };
                // Ignore groups of non-Latin characters
                if is_mostly_latin_string(&self.tok(begin).content) && skipped_previous_group {
                    break;
                }
                token_begin = self.find_next(token_begin, flags::BRACKET);
                skipped_previous_group = true;
            }
        }
        let Some(begin) = token_begin else { return };

        // Up to an identifier (or a bracket, if the title is enclosed).
        let mut token_end = self.find_next(Some(begin), flags::IDENTIFIER | if enclosed_title { flags::BRACKET } else { flags::NONE });

        if !enclosed_title {
            // An unpaired open bracket in the interval: end there.
            let mut last_bracket = token_end;
            let mut bracket_open = false;
            for id in self.get_list(Some(flags::BRACKET), Some(begin), token_end) {
                last_bracket = Some(id);
                bracket_open = !bracket_open;
            }
            if bracket_open {
                token_end = last_bracket;
            }

            // Ending with an enclosed group ("Title [Fansub]"): end before
            // it. Parentheses are kept, e.g. "(TV)".
            let mut token = self.find_previous(token_end, flags::NOT_DELIMITER);
            while let Some(t) = token {
                let tk = self.tok(t);
                if !(tk.category == TokenCategory::Bracket && tk.content != ")") {
                    break;
                }
                token = self.find_previous(Some(t), flags::BRACKET);
                if token.is_some() {
                    token_end = token;
                    token = self.find_previous(token_end, flags::NOT_DELIMITER);
                }
            }
        }

        // token_end is a bracket: include the token before it.
        let token_end = self.find_previous(token_end, flags::VALID);
        self.build_element(Category::AnimeTitle, Some(begin), token_end, false);
    }

    fn search_for_release_group(&mut self) {
        let mut token_end = None;
        loop {
            let token_begin = match token_end {
                Some(_) => self.find_next(token_end, flags::ENCLOSED | flags::UNKNOWN),
                None => self.find(flags::ENCLOSED | flags::UNKNOWN),
            };
            let Some(begin) = token_begin else { return };
            token_end = self.find_next(Some(begin), flags::BRACKET | flags::IDENTIFIER);
            let Some(end) = token_end else { return };
            if self.tok(end).category != TokenCategory::Bracket {
                continue;
            }
            // Only the first non-delimiter token in its group.
            let previous = self.find_previous(Some(begin), flags::NOT_DELIMITER);
            if previous.is_some_and(|p| self.tok(p).category != TokenCategory::Bracket) {
                continue;
            }
            let end = self.find_previous(Some(end), flags::VALID);
            self.build_element(Category::ReleaseGroup, Some(begin), end, true);
            return;
        }
    }

    fn search_for_episode_title(&mut self) {
        let mut token_end = None;
        loop {
            let token_begin = match token_end {
                Some(_) => self.find_next(token_end, flags::NOT_ENCLOSED | flags::UNKNOWN),
                None => self.find(flags::NOT_ENCLOSED | flags::UNKNOWN),
            };
            let Some(begin) = token_begin else { return };
            token_end = self.find_next(Some(begin), flags::BRACKET | flags::IDENTIFIER);

            // Only a dash: skip. (anitopy loops forever when that dash is
            // the last token; stop instead.)
            if self.distance(Some(begin), token_end) <= 2 && is_dash_character(&self.tok(begin).content) {
                if token_end.is_none() {
                    return;
                }
                continue;
            }
            if token_end.is_some_and(|e| self.tok(e).category == TokenCategory::Bracket) {
                token_end = self.find_previous(token_end, flags::VALID);
            }
            self.build_element(Category::EpisodeTitle, Some(begin), token_end, false);
            return;
        }
    }

    fn validate_elements(&mut self) {
        // An episode title that is (or contains) an anime type.
        if !self.elements.contains(Category::AnimeType) || !self.elements.contains(Category::EpisodeTitle) {
            return;
        }
        let episode_title = self.elements.first(Category::EpisodeTitle).unwrap_or_default().to_string();
        for anime_type in self.elements.get(Category::AnimeType).to_vec() {
            if anime_type == episode_title {
                self.elements.erase(Category::EpisodeTitle);
            } else if episode_title.contains(&anime_type) && keyword_manager().find(&normalize(&anime_type), Category::AnimeType).is_some() {
                self.elements.remove(Category::AnimeType, &anime_type);
            }
        }
    }
}
