// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// nyaa-stream additions to the anitopy port: title formats anitopy gets
// wrong on real nyaa.si titles (TITLE_ANALYSIS.md in the nyaa-stream repo).

use std::sync::OnceLock;

use regex::Regex;

use crate::element::Category;
use crate::token::{flags, TokenCategory};
use crate::Ctx;

macro_rules! re {
    ($pattern:expr) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($pattern).unwrap())
    }};
}

/// 一 .. 十 as numbers.
fn cjk_numeral(s: &str) -> Option<u32> {
    Some(match s {
        "一" => 1,
        "二" => 2,
        "三" => 3,
        "四" => 4,
        "五" => 5,
        "六" => 6,
        "七" => 7,
        "八" => 8,
        "九" => 9,
        "十" => 10,
        _ => return None,
    })
}

impl Ctx {
    /// `第3期`, `第三季`, `3期`: a season, not an episode. Runs before the
    /// episode search, which would otherwise take `第3期` as episode "3期"
    /// through the 第 episode prefix.
    pub(crate) fn search_for_cjk_seasons(&mut self) {
        for id in self.get_list(Some(flags::UNKNOWN), None, None) {
            let content = self.tok(id).content.clone();
            let Some(c) = re!(r"^\u{7B2C}?(\d{1,2}|[一二三四五六七八九十])[\u{671F}\u{5B63}]$").captures(&content) else { continue };
            let n = c[1].parse::<u32>().ok().or_else(|| cjk_numeral(&c[1]));
            if let Some(n) = n {
                self.elements.insert(Category::AnimeSeason, n.to_string());
                self.tok_mut(id).category = TokenCategory::Identifier;
            }
        }
    }
}

fn roman(s: &str) -> Option<u32> {
    Some(match s {
        "II" => 2,
        "III" => 3,
        "IV" => 4,
        "V" => 5,
        "VI" => 6,
        "VII" => 7,
        "VIII" => 8,
        "IX" => 9,
        _ => return None,
    })
}

fn is_dash(s: &str) -> bool {
    crate::helper::is_dash_character(s)
}

impl Ctx {
    fn insert_season_range(&mut self, a: u32, b: u32) {
        for n in a..=b {
            self.elements.insert(Category::AnimeSeason, n.to_string());
        }
    }

    fn token_after(&self, id: usize) -> Option<usize> {
        self.find_next(Some(id), flags::NOT_DELIMITER)
    }

    /// Previous non-delimiter token, without anitopy's wrap-around.
    fn token_before(&self, id: usize) -> Option<usize> {
        if self.idx(id) == 0 {
            return None;
        }
        self.find_previous(Some(id), flags::NOT_DELIMITER).filter(|&p| self.idx(p) < self.idx(id))
    }

    /// Multi-season packs: `S01-04`, `S01-S05`, `S1 - S5`, `Season 1-4`,
    /// `Seasons 1+2`, and `(Season 1 - 3)` when it fills a bracket group.
    /// Outside brackets, `Season 3 - 12` is season 3 episode 12 (Doomdos),
    /// so the spaced form only counts enclosed.
    pub(crate) fn search_for_season_ranges(&mut self) {
        let s_range = re!(r"(?i)^S(\d{1,2})[-~]S?(\d{1,2})$");
        let s_single = re!(r"(?i)^S(\d{1,2})$");
        let n_range = re!(r"^(\d{1,2})[-~+&](\d{1,2})$");
        for id in self.get_list(None, None, None) {
            let t = self.tok(id).clone();
            if t.category == TokenCategory::Delimiter || t.category == TokenCategory::Bracket {
                continue;
            }
            let mut found: Option<(u32, u32, Vec<usize>)> = None;
            if let Some(c) = s_range.captures(&t.content) {
                found = Some((c[1].parse().unwrap(), c[2].parse().unwrap(), vec![id]));
            } else if let Some(c) = s_single.captures(&t.content) {
                // "S1 - S5"
                if let Some(dash) = self.token_after(id).filter(|&d| is_dash(&self.tok(d).content)) {
                    if let Some(end) = self.token_after(dash) {
                        if let Some(c2) = s_single.captures(&self.tok(end).content.clone()) {
                            found = Some((c[1].parse().unwrap(), c2[1].parse().unwrap(), vec![id, dash, end]));
                        }
                    }
                }
            } else if matches!(t.content.to_lowercase().as_str(), "season" | "seasons") {
                if let Some(next) = self.token_after(id) {
                    let nc = self.tok(next).content.clone();
                    if let Some(c) = n_range.captures(&nc) {
                        found = Some((c[1].parse().unwrap(), c[2].parse().unwrap(), vec![id, next]));
                    } else if t.enclosed && nc.chars().all(|c| c.is_ascii_digit()) && !nc.is_empty() {
                        // "(Season 1 - 3)"
                        let dash = self.token_after(next).filter(|&d| is_dash(&self.tok(d).content));
                        let end = dash.and_then(|d| self.token_after(d)).filter(|&e| self.tok(e).content.chars().all(|c| c.is_ascii_digit()) && !self.tok(e).content.is_empty());
                        let closes = end.and_then(|e| self.token_after(e)).map_or(true, |a| self.tok(a).category == TokenCategory::Bracket);
                        if let (Some(dash), Some(end), true) = (dash, end, closes) {
                            found = Some((nc.parse().unwrap(), self.tok(end).content.parse().unwrap(), vec![id, next, dash, end]));
                        }
                    }
                }
            }
            if let Some((a, b, ids)) = found {
                if a < b && b <= 30 {
                    self.insert_season_range(a, b);
                    for i in ids {
                        self.tok_mut(i).category = TokenCategory::Identifier;
                    }
                }
            }
        }
    }

    /// A roman numeral II-IX right after title words is a season:
    /// `Mob Psycho 100 III - 10`, `Overlord IV`, `Mushoku Tensei II: ...`.
    /// Only when followed by the end, a bracket, a dash, `:` / `|` / `/`, a
    /// number or an identifier - so `Final Fantasy VII Advent Children`
    /// stays a title. `I` never counts (`Infinity Castle I` is a movie part),
    /// nor `X` (`Rockman Irregular Hunter X`), nor after Part/Cour/Vol.
    pub(crate) fn search_for_roman_seasons(&mut self) {
        for id in self.get_list(Some(flags::UNKNOWN), None, None) {
            let content = self.tok(id).content.clone();
            let Some(n) = roman(content.trim_end_matches(':')) else { continue };
            let Some(prev) = self.token_before(id) else { continue };
            let prev_word = self.tok(prev).content.to_lowercase();
            // "Fate Zero Part II" is a cour, "Vol II" a volume.
            if matches!(prev_word.as_str(), "part" | "cour" | "vol" | "vol." | "volume" | "chapter" | "movie") {
                continue;
            }
            if self.tok(prev).category != TokenCategory::Unknown || self.tok(prev).enclosed != self.tok(id).enclosed {
                continue;
            }
            let boundary = content.ends_with(':')
                || match self.token_after(id) {
                    None => true,
                    Some(next) => {
                        let nt = self.tok(next);
                        nt.category == TokenCategory::Bracket
                            || nt.category == TokenCategory::Identifier
                            || is_dash(&nt.content)
                            || matches!(nt.content.as_str(), ":" | "|" | "/")
                            || nt.content.chars().next().is_some_and(|c| c.is_ascii_digit())
                            || re!(r"(?i)^S\d").is_match(&nt.content)
                    }
                };
            if boundary {
                self.elements.insert(Category::AnimeSeason, n.to_string());
                self.tok_mut(id).category = TokenCategory::Identifier;
            }
        }
    }

    /// `01 ~ 25`, `001 ~ 366`: a spaced tilde range (anitopy only reads the
    /// unspaced `01~25`). Returns whether one was found.
    pub(crate) fn search_for_spaced_ranges(&mut self, tokens: &[usize]) -> bool {
        for &id in tokens {
            let a = self.tok(id).content.clone();
            if !a.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Some(tilde) = self.token_after(id).filter(|&t| matches!(self.tok(t).content.as_str(), "~" | "\u{FF5E}")) else { continue };
            let Some(end) = self.token_after(tilde) else { continue };
            let b = self.tok(end).content.clone();
            if b.is_empty() || !b.chars().all(|c| c.is_ascii_digit()) || a.parse::<u32>().ok() >= b.parse::<u32>().ok() {
                continue;
            }
            if self.set_episode_number(&a, id, true) {
                self.set_episode_number(&b, end, false);
                self.tok_mut(tilde).category = TokenCategory::Identifier;
                return true;
            }
        }
        false
    }
}
