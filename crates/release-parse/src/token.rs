// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's token.py. anitopy refers to tokens by object identity;
// here every token gets a stable `id` and lookups go through `idx`, so
// inserting a token mid-parse doesn't invalidate references.

use crate::Ctx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenCategory {
    Unknown,
    Bracket,
    Delimiter,
    Identifier,
    Invalid,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub id: usize,
    pub category: TokenCategory,
    pub content: String,
    pub enclosed: bool,
}

pub(crate) type TokenId = usize;

pub(crate) mod flags {
    pub const NONE: u32 = 0;
    pub const BRACKET: u32 = 1 << 0;
    pub const NOT_BRACKET: u32 = 1 << 1;
    pub const DELIMITER: u32 = 1 << 2;
    pub const NOT_DELIMITER: u32 = 1 << 3;
    pub const IDENTIFIER: u32 = 1 << 4;
    pub const NOT_IDENTIFIER: u32 = 1 << 5;
    pub const UNKNOWN: u32 = 1 << 6;
    pub const NOT_UNKNOWN: u32 = 1 << 7;
    pub const VALID: u32 = 1 << 8;
    pub const NOT_VALID: u32 = 1 << 9;
    pub const ENCLOSED: u32 = 1 << 10;
    pub const NOT_ENCLOSED: u32 = 1 << 11;
    pub const MASK_CATEGORIES: u32 = BRACKET | NOT_BRACKET | DELIMITER | NOT_DELIMITER | IDENTIFIER | NOT_IDENTIFIER | UNKNOWN | NOT_UNKNOWN | VALID | NOT_VALID;
    pub const MASK_ENCLOSED: u32 = ENCLOSED | NOT_ENCLOSED;
}

impl Token {
    /// The enclosed flags must hold; of the category flags, any one is
    /// enough (anitopy's `check_flags`).
    pub fn check_flags(&self, f: u32) -> bool {
        use flags::*;
        let has = |flag: u32| f & flag == flag;
        if f & MASK_ENCLOSED != 0 {
            let ok = if has(ENCLOSED) { self.enclosed } else { !self.enclosed };
            if !ok {
                return false;
            }
        }
        if f & MASK_CATEGORIES != 0 {
            let check = |fe: u32, fn_: u32, cat: TokenCategory| {
                if has(fe) {
                    self.category == cat
                } else if has(fn_) {
                    self.category != cat
                } else {
                    false
                }
            };
            return check(BRACKET, NOT_BRACKET, TokenCategory::Bracket)
                || check(DELIMITER, NOT_DELIMITER, TokenCategory::Delimiter)
                || check(IDENTIFIER, NOT_IDENTIFIER, TokenCategory::Identifier)
                || check(UNKNOWN, NOT_UNKNOWN, TokenCategory::Unknown)
                || check(NOT_VALID, VALID, TokenCategory::Invalid);
        }
        true
    }
}

impl Ctx {
    pub(crate) fn add_token(&mut self, category: TokenCategory, content: &str, enclosed: bool) {
        let id = self.next_id;
        self.next_id += 1;
        self.tokens.push(Token { id, category, content: content.to_string(), enclosed });
    }

    pub(crate) fn insert_token(&mut self, index: usize, category: TokenCategory, content: &str, enclosed: bool) {
        let id = self.next_id;
        self.next_id += 1;
        self.tokens.insert(index, Token { id, category, content: content.to_string(), enclosed });
    }

    pub(crate) fn idx(&self, id: TokenId) -> usize {
        self.tokens.iter().position(|t| t.id == id).expect("token id refers to a live token")
    }

    pub(crate) fn tok(&self, id: TokenId) -> &Token {
        &self.tokens[self.idx(id)]
    }

    pub(crate) fn tok_mut(&mut self, id: TokenId) -> &mut Token {
        let i = self.idx(id);
        &mut self.tokens[i]
    }

    /// Token ids from `begin` to `end` inclusive (`None` = list start/end),
    /// optionally filtered by flags.
    pub(crate) fn get_list(&self, f: Option<u32>, begin: Option<TokenId>, end: Option<TokenId>) -> Vec<TokenId> {
        let b = begin.map(|t| self.idx(t)).unwrap_or(0);
        let e = end.map(|t| self.idx(t) + 1).unwrap_or(self.tokens.len()).min(self.tokens.len());
        if b >= e {
            return Vec::new();
        }
        self.tokens[b..e].iter().filter(|t| f.map_or(true, |f| t.check_flags(f))).map(|t| t.id).collect()
    }

    pub(crate) fn distance(&self, begin: Option<TokenId>, end: Option<TokenId>) -> isize {
        let b = begin.map(|t| self.idx(t)).unwrap_or(0) as isize;
        let e = end.map(|t| self.idx(t)).unwrap_or(self.tokens.len()) as isize;
        e - b
    }

    pub(crate) fn find(&self, f: u32) -> Option<TokenId> {
        self.tokens.iter().find(|t| t.check_flags(f)).map(|t| t.id)
    }

    /// Like anitopy, searching back from the first token wraps around to
    /// the last one (Python's `tokens[-1::-1]`).
    pub(crate) fn find_previous(&self, token: Option<TokenId>, f: u32) -> Option<TokenId> {
        let end = match token {
            None => self.tokens.len(),
            Some(t) => match self.idx(t) {
                0 => self.tokens.len(),
                i => i,
            },
        };
        self.tokens[..end].iter().rev().find(|t| t.check_flags(f)).map(|t| t.id)
    }

    pub(crate) fn find_next(&self, token: Option<TokenId>, f: u32) -> Option<TokenId> {
        let start = token.map(|t| self.idx(t) + 1).unwrap_or(0);
        self.tokens.get(start..).unwrap_or(&[]).iter().find(|t| t.check_flags(f)).map(|t| t.id)
    }

    pub(crate) fn category_of(&self, token: Option<TokenId>) -> Option<TokenCategory> {
        token.map(|t| self.tok(t).category)
    }
}
