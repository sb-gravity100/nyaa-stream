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
