// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's tokenizer.py.

use crate::helper::py_isdigit;
use crate::keyword::PEEK_ENTRIES;
use crate::token::{flags, TokenCategory, TokenId};
use crate::Ctx;

const BRACKETS: &[(char, char)] = &[
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('\u{300C}', '\u{300D}'),
    ('\u{300E}', '\u{300F}'),
    ('\u{3010}', '\u{3011}'),
    ('\u{FF08}', '\u{FF09}'),
];

impl Ctx {
    pub(crate) fn tokenize(&mut self, filename: &str) -> bool {
        self.tokenize_by_brackets(filename);
        !self.tokens.is_empty()
    }

    fn tokenize_by_brackets(&mut self, filename: &str) {
        let mut text = filename;
        let mut is_bracket_open = false;
        let mut matching_bracket = ')';
        while !text.is_empty() {
            let bracket_index = if !is_bracket_open {
                let found = text.char_indices().find_map(|(i, c)| BRACKETS.iter().find(|(open, _)| *open == c).map(|(_, close)| (i, *close)));
                if let Some((_, close)) = found {
                    matching_bracket = close;
                }
                found.map(|(i, _)| i)
            } else {
                // Looking for the matching bracket handles some nested cases.
                text.find(matching_bracket)
            };

            if bracket_index != Some(0) {
                let before = match bracket_index {
                    Some(i) => &text[..i],
                    None => text,
                };
                self.tokenize_by_preidentified(before, is_bracket_open);
            }

            match bracket_index {
                Some(i) => {
                    let len = text[i..].chars().next().map_or(1, char::len_utf8);
                    self.add_token(TokenCategory::Bracket, &text[i..i + len], true);
                    is_bracket_open = !is_bracket_open;
                    text = &text[i + len..];
                }
                None => text = "",
            }
        }
    }

    fn tokenize_by_preidentified(&mut self, text: &str, enclosed: bool) {
        let mut preidentified: Vec<(usize, usize)> = Vec::new();
        for (category, keywords) in PEEK_ENTRIES {
            for keyword in *keywords {
                if let Some(begin) = text.find(keyword) {
                    self.elements.insert(*category, *keyword);
                    preidentified.push((begin, begin + keyword.len()));
                }
            }
        }
        preidentified.sort();

        let mut last_end = 0;
        for (begin, end) in preidentified {
            if last_end != begin {
                // Text between pre-identified tokens (empty if they overlap,
                // like Python's slicing).
                let between = if begin > last_end { &text[last_end..begin] } else { "" };
                self.tokenize_by_delimiters(between, enclosed);
            }
            self.add_token(TokenCategory::Identifier, &text[begin..end], enclosed);
            last_end = end;
        }
        if last_end != text.len() {
            let rest = if last_end < text.len() { &text[last_end..] } else { "" };
            self.tokenize_by_delimiters(rest, enclosed);
        }
    }

    fn tokenize_by_delimiters(&mut self, text: &str, enclosed: bool) {
        let delimiters = self.options.allowed_delimiters;
        let mut start = 0;
        for (i, c) in text.char_indices() {
            if delimiters.contains(c) {
                if i > start {
                    self.add_token(TokenCategory::Unknown, &text[start..i], enclosed);
                }
                self.add_token(TokenCategory::Delimiter, &text[i..i + c.len_utf8()], enclosed);
                start = i + c.len_utf8();
            }
        }
        if start < text.len() {
            self.add_token(TokenCategory::Unknown, &text[start..], enclosed);
        }
        self.validate_delimiter_tokens();
    }

    fn is_delimiter_token(&self, t: Option<TokenId>) -> bool {
        self.category_of(t) == Some(TokenCategory::Delimiter)
    }

    fn is_unknown_token(&self, t: Option<TokenId>) -> bool {
        self.category_of(t) == Some(TokenCategory::Unknown)
    }

    fn is_single_character_token(&self, t: Option<TokenId>) -> bool {
        self.is_unknown_token(t) && {
            let c = &self.tok(t.unwrap()).content;
            c.chars().count() == 1 && c != "-"
        }
    }

    fn append_token_to(&mut self, token: Option<TokenId>, append_to: Option<TokenId>) {
        let (Some(token), Some(append_to)) = (token, append_to) else { return };
        let content = self.tok(token).content.clone();
        self.tok_mut(append_to).content.push_str(&content);
        self.tok_mut(token).category = TokenCategory::Invalid;
    }

    fn validate_delimiter_tokens(&mut self) {
        let content = |s: &Self, t: Option<TokenId>| t.map(|t| s.tok(t).content.clone()).unwrap_or_default();
        for id in self.get_list(None, None, None) {
            if self.tok(id).category != TokenCategory::Delimiter {
                continue;
            }
            let delimiter = self.tok(id).content.clone();
            let prev = self.find_previous(Some(id), flags::VALID);
            let mut next = self.find_next(Some(id), flags::VALID);

            // Single-character tokens: don't split group names, keywords,
            // episode numbers, etc.
            if delimiter != " " && delimiter != "_" {
                if self.is_single_character_token(prev) {
                    self.append_token_to(Some(id), prev);
                    while self.is_unknown_token(next) {
                        self.append_token_to(next, prev);
                        next = self.find_next(next, flags::VALID);
                        if self.is_delimiter_token(next) && content(self, next) == delimiter {
                            self.append_token_to(next, prev);
                            next = self.find_next(next, flags::VALID);
                        }
                    }
                    continue;
                }
                if self.is_single_character_token(next) {
                    self.append_token_to(Some(id), prev);
                    self.append_token_to(next, prev);
                    continue;
                }
            }

            // Adjacent delimiters
            if self.is_unknown_token(prev) && self.is_delimiter_token(next) {
                let next_delimiter = content(self, next);
                if delimiter != next_delimiter && delimiter != "," && (next_delimiter == " " || next_delimiter == "_") {
                    self.append_token_to(Some(id), prev);
                }
            } else if self.is_delimiter_token(prev) && self.is_delimiter_token(next) {
                let prev_delimiter = content(self, prev);
                let next_delimiter = content(self, next);
                if prev_delimiter == next_delimiter && prev_delimiter != delimiter {
                    self.tok_mut(id).category = TokenCategory::Unknown; // e.g. "&" in "_&_"
                }
            }

            // e.g. "01+02"
            if (delimiter == "&" || delimiter == "+")
                && self.is_unknown_token(prev)
                && self.is_unknown_token(next)
                && py_isdigit(&content(self, prev))
                && py_isdigit(&content(self, next))
            {
                self.append_token_to(Some(id), prev);
                self.append_token_to(next, prev);
            }
        }
        self.tokens.retain(|t| t.category != TokenCategory::Invalid);
    }
}
