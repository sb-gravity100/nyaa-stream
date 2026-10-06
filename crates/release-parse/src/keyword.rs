// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's keyword.py.

use std::collections::HashMap;
use std::sync::OnceLock;

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::element::Category;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Keyword {
    pub category: Category,
    pub identifiable: bool,
    pub searchable: bool,
    pub valid: bool,
}

pub(crate) struct KeywordManager {
    keys: HashMap<String, Keyword>,
    file_extensions: HashMap<String, Keyword>,
}

const DEFAULT: (bool, bool, bool) = (true, true, true);
const INVALID: (bool, bool, bool) = (true, true, false);
const UNIDENTIFIABLE: (bool, bool, bool) = (false, true, true);
const UNIDENTIFIABLE_INVALID: (bool, bool, bool) = (false, true, false);
const UNIDENTIFIABLE_UNSEARCHABLE: (bool, bool, bool) = (false, false, true);

impl KeywordManager {
    fn new() -> Self {
        use Category::*;
        let mut m = KeywordManager { keys: HashMap::new(), file_extensions: HashMap::new() };
        m.add(AnimeSeasonPrefix, UNIDENTIFIABLE, &["S", "SAISON", "SEASON"]);
        m.add(AnimeType, UNIDENTIFIABLE, &["GEKIJOUBAN", "MOVIE", "OAD", "OAV", "ONA", "OVA", "SPECIAL", "SPECIALS", "TV"]);
        m.add(AnimeType, UNIDENTIFIABLE_UNSEARCHABLE, &["SP"]);
        m.add(AnimeType, UNIDENTIFIABLE_INVALID, &["ED", "ENDING", "NCED", "NCOP", "OP", "OPENING", "PREVIEW", "PV"]);
        m.add(
            AudioTerm,
            DEFAULT,
            &[
                "2.0CH", "2CH", "5.1", "5.1CH", "DTS", "DTS-ES", "DTS5.1", "TRUEHD5.1", "AAC", "AACX2", "AACX3", "AACX4", "AC3", "EAC3", "E-AC-3", "FLAC", "FLACX2", "FLACX3",
                "FLACX4", "LOSSLESS", "MP3", "OGG", "VORBIS", "DUALAUDIO", "DUAL AUDIO", "DUAL-AUDIO", "MULTIAUDIO", "MULTI AUDIO", "MULTI-AUDIO",
            ],
        );
        m.add(DeviceCompatibility, DEFAULT, &["IPAD3", "IPHONE5", "IPOD", "PS3", "XBOX", "XBOX360"]);
        m.add(DeviceCompatibility, UNIDENTIFIABLE, &["ANDROID"]);
        m.add(EpisodePrefix, DEFAULT, &["EP", "EP.", "EPS", "EPS.", "EPISODE", "EPISODE.", "EPISODES", "CAPITULO", "EPISODIO", "FOLGE"]);
        // anitopy writes '\x7B2C' meaning U+7B2C (第), but Python reads it
        // as "{2C", so its 第 prefix never matched. Fixed here.
        m.add(EpisodePrefix, INVALID, &["E", "\u{7B2C}"]);
        m.add(FileExtension, DEFAULT, &["3GP", "AVI", "DIVX", "FLV", "M2TS", "MKV", "MOV", "MP4", "MPG", "OGM", "RM", "RMVB", "TS", "WEBM", "WMV"]);
        m.add(FileExtension, INVALID, &["AAC", "AIFF", "FLAC", "M4A", "MP3", "MKA", "OGG", "WAV", "WMA", "7Z", "RAR", "ZIP", "ASS", "SRT"]);
        m.add(Language, DEFAULT, &["ENG", "ENGLISH", "ESPANOL", "JAP", "PT-BR", "SPANISH", "VOSTFR"]);
        m.add(Language, UNIDENTIFIABLE, &["ESP", "ITA"]);
        m.add(Other, DEFAULT, &["REMASTER", "REMASTERED", "UNCENSORED", "UNCUT", "TS", "VFR", "WIDESCREEN", "WS"]);
        m.add(ReleaseGroup, DEFAULT, &["THORA"]);
        m.add(ReleaseInformation, DEFAULT, &["BATCH", "COMPLETE", "PATCH", "REMUX"]);
        m.add(ReleaseInformation, UNIDENTIFIABLE, &["END", "FINAL"]);
        m.add(ReleaseVersion, DEFAULT, &["V0", "V1", "V2", "V3", "V4"]);
        m.add(
            Source,
            DEFAULT,
            &["BD", "BDRIP", "BLURAY", "BLU-RAY", "DVD", "DVD5", "DVD9", "DVD-R2J", "DVDRIP", "DVD-RIP", "R2DVD", "R2J", "R2JDVD", "R2JDVDRIP", "HDTV", "HDTVRIP", "TVRIP", "TV-RIP", "WEBCAST", "WEBRIP"],
        );
        m.add(
            Subtitles,
            DEFAULT,
            &["ASS", "BIG5", "DUB", "DUBBED", "HARDSUB", "HARDSUBS", "RAW", "SOFTSUB", "SOFTSUBS", "SUB", "SUBBED", "SUBTITLED", "MULTIPLE SUBTITLE", "MULTI SUBS", "MULTI-SUBS"],
        );
        m.add(
            VideoTerm,
            DEFAULT,
            &[
                "23.976FPS", "24FPS", "29.97FPS", "30FPS", "60FPS", "120FPS", "8BIT", "8-BIT", "10BIT", "10BITS", "10-BIT", "10-BITS", "HI10", "HI10P", "HI444", "HI444P", "HI444PP", "H264",
                "H265", "H.264", "H.265", "X264", "X265", "X.264", "AVC", "HEVC", "HEVC2", "DIVX", "DIVX5", "DIVX6", "XVID", "AVI", "RMVB", "WMV", "WMV3", "WMV9", "HQ", "LQ", "HD", "SD",
            ],
        );
        m.add(VolumePrefix, DEFAULT, &["VOL", "VOL.", "VOLUME"]);
        m
    }

    fn add(&mut self, category: Category, (identifiable, searchable, valid): (bool, bool, bool), keywords: &[&str]) {
        let container = if category == Category::FileExtension { &mut self.file_extensions } else { &mut self.keys };
        for k in keywords {
            if k.is_empty() || container.contains_key(*k) {
                continue;
            }
            container.insert(k.to_string(), Keyword { category, identifiable, searchable, valid });
        }
    }

    /// `category` = `Unknown` searches every non-extension keyword.
    pub fn find(&self, string: &str, category: Category) -> Option<Keyword> {
        let container = if category == Category::FileExtension { &self.file_extensions } else { &self.keys };
        let keyword = *container.get(string)?;
        if category != Category::Unknown && keyword.category != category {
            return None;
        }
        Some(keyword)
    }
}

pub(crate) fn keyword_manager() -> &'static KeywordManager {
    static M: OnceLock<KeywordManager> = OnceLock::new();
    M.get_or_init(KeywordManager::new)
}

/// Accents and other combining marks removed, upper-cased.
pub(crate) fn normalize(string: &str) -> String {
    string.nfkd().filter(|c| !is_combining_mark(*c)).collect::<String>().to_uppercase()
}

/// Keywords found verbatim anywhere in `string` before tokenizing (they
/// contain delimiters), as byte ranges, sorted.
pub(crate) const PEEK_ENTRIES: &[(Category, &[&str])] = &[
    (Category::AudioTerm, &["Dual Audio", "Multi Audio"]),
    (Category::VideoTerm, &["H264", "H.264", "h264", "h.264"]),
    (Category::VideoResolution, &["480p", "720p", "1080p"]),
    (Category::Subtitles, &["Multiple Subtitle", "Multi Subs"]),
    (Category::Source, &["Blu-Ray"]),
];
