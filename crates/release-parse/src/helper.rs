// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's parser_helper.py, plus the Python string semantics the
// port relies on (`str.isdigit`, `int()`).

use std::sync::OnceLock;

use regex::Regex;

use crate::element::Category;
use crate::token::{flags, TokenCategory, TokenId};
use crate::Ctx;

pub(crate) const DASHES: &str = "-\u{2010}\u{2011}\u{2012}\u{2013}\u{2014}\u{2015}";

/// Python's `str.isdigit` for one character: decimal digits plus
/// superscript/subscript/circled digits.
pub(crate) fn is_py_digit(c: char) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    if c.is_ascii_digit() {
        return true;
    }
    let re = RE.get_or_init(|| Regex::new(r"^[\p{Nd}\u{B2}\u{B3}\u{B9}\u{2070}\u{2074}-\u{2079}\u{2080}-\u{2089}\u{2460}-\u{2468}]$").unwrap());
    let mut buf = [0u8; 4];
    re.is_match(c.encode_utf8(&mut buf))
}

/// Python's `str.isdigit`.
pub(crate) fn py_isdigit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_py_digit)
}

/// Python's `int(str)` for the digits nyaa titles use (ASCII and
/// full-width); `None` where Python would raise.
pub(crate) fn py_int(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let digits: String = digits.chars().filter(|c| *c != '_').collect();
    if digits.is_empty() {
        return None;
    }
    let mut n: i64 = 0;
    for c in digits.chars() {
        let d = match c {
            '0'..='9' => c as i64 - '0' as i64,
            '\u{FF10}'..='\u{FF19}' => c as i64 - 0xFF10,
            _ => return None,
        };
        n = n.checked_mul(10)?.checked_add(d)?;
    }
    Some(if neg { -n } else { n })
}

/// anitopy's `str2int`: 0 when not a number.
pub(crate) fn str2int(s: &str) -> i64 {
    py_int(s).unwrap_or(0)
}

/// Byte index of the first digit.
pub(crate) fn find_number_in_string(s: &str) -> Option<usize> {
    s.char_indices().find(|(_, c)| is_py_digit(*c)).map(|(i, _)| i)
}

/// Byte index of the first non-digit.
pub(crate) fn find_non_number_in_string(s: &str) -> Option<usize> {
    s.char_indices().find(|(_, c)| !is_py_digit(*c)).map(|(i, _)| i)
}

fn is_hexadecimal_string(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn get_number_from_ordinal(s: &str) -> Option<&'static str> {
    Some(match s {
        "1st" | "First" => "1",
        "2nd" | "Second" => "2",
        "3rd" | "Third" => "3",
        "4th" | "Fourth" => "4",
        "5th" | "Fifth" => "5",
        "6th" | "Sixth" => "6",
        "7th" | "Seventh" => "7",
        "8th" | "Eighth" => "8",
        "9th" | "Ninth" => "9",
        _ => return None,
    })
}

pub(crate) fn is_crc32(s: &str) -> bool {
    s.chars().count() == 8 && is_hexadecimal_string(s)
}

pub(crate) fn is_dash_character(s: &str) -> bool {
    let mut chars = s.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if DASHES.contains(c))
}

/// Whether the character's Unicode name contains "LATIN" (anitopy's test),
/// approximated by the Latin letter blocks.
fn is_latin_char(c: char) -> bool {
    matches!(c as u32,
        0x41..=0x5A | 0x61..=0x7A
        | 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x24F
        | 0x250..=0x2AF
        | 0x1D00..=0x1D2B | 0x1D62..=0x1D65 | 0x1D6B..=0x1D77 | 0x1D79..=0x1DBE
        | 0x1E00..=0x1EFF
        | 0x2C60..=0x2C7F | 0xA720..=0xA7FF | 0xAB30..=0xAB64
        | 0xFB00..=0xFB06
        | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A)
}

pub(crate) fn is_mostly_latin_string(s: &str) -> bool {
    let total = s.chars().count();
    if total == 0 {
        return false;
    }
    let latin = s.chars().filter(|c| is_latin_char(*c)).count();
    latin as f64 / total as f64 >= 0.5
}

pub(crate) fn is_resolution(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{3,4}([pP]|([xX\u{00D7}]\d{3,4}))$").unwrap()).is_match(s)
}

/// Python's `str.strip(chars)`.
pub(crate) fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

impl Ctx {
    pub(crate) fn content(&self, t: Option<TokenId>) -> String {
        t.map(|t| self.tok(t).content.clone()).unwrap_or_default()
    }

    pub(crate) fn check_anime_season_keyword(&mut self, token: TokenId) -> bool {
        let previous = self.find_previous(Some(token), flags::NOT_DELIMITER);
        if let Some(previous) = previous {
            if let Some(number) = get_number_from_ordinal(&self.tok(previous).content) {
                self.set_anime_season(previous, token, number.to_string());
                return true;
            }
        }
        let next = self.find_next(Some(token), flags::NOT_DELIMITER);
        if let Some(next) = next {
            let c = self.tok(next).content.clone();
            if py_isdigit(&c) {
                self.set_anime_season(token, next, c);
                return true;
            }
        }
        false
    }

    fn set_anime_season(&mut self, first: TokenId, second: TokenId, content: String) {
        self.elements.insert(Category::AnimeSeason, content);
        self.tok_mut(first).category = TokenCategory::Identifier;
        self.tok_mut(second).category = TokenCategory::Identifier;
    }

    pub(crate) fn is_token_isolated(&self, token: TokenId) -> bool {
        let previous = self.find_previous(Some(token), flags::NOT_DELIMITER);
        if self.category_of(previous) != Some(TokenCategory::Bracket) {
            return false;
        }
        let next = self.find_next(Some(token), flags::NOT_DELIMITER);
        self.category_of(next) == Some(TokenCategory::Bracket)
    }

    pub(crate) fn build_element(&mut self, category: Category, begin: Option<TokenId>, end: Option<TokenId>, keep_delimiters: bool) {
        let mut element = String::new();
        for id in self.get_list(None, begin, end) {
            let t = self.tok(id).clone();
            match t.category {
                TokenCategory::Unknown => {
                    element.push_str(&t.content);
                    self.tok_mut(id).category = TokenCategory::Identifier;
                }
                TokenCategory::Bracket => element.push_str(&t.content),
                TokenCategory::Delimiter => {
                    if keep_delimiters {
                        element.push_str(&t.content);
                    } else if Some(id) != begin && Some(id) != end {
                        if t.content == "," || t.content == "&" {
                            element.push_str(&t.content);
                        } else {
                            element.push(' ');
                        }
                    }
                }
                _ => {}
            }
        }
        let element = if keep_delimiters { element } else { strip_chars(&element, &format!(" {DASHES}")).to_string() };
        if !element.is_empty() {
            self.elements.insert(category, element);
        }
    }
}
