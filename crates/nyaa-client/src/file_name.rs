//! File-name rules for files *inside* a torrent, on top of `release-parse`
//! (which is tuned for release titles). Measured on the AnimeTosho file lists
//! (PLAN.md "File-name coverage"): extras numbered like episodes (`NCOP03`,
//! `ED2`, `Menu 2`) and episode names the title parser leaves unread
//! (`S01E06-Title`, `12 - Title`, `21.mkv`, `Ep-12`, `第3話`, `Show -04`).

use std::sync::OnceLock;

use regex::Regex;

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}

/// A folder that only holds extras ("NC", "Creditless", "Extras", "Menus", ...).
fn extra_folder(folder: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, r"(?i)^\s*(?:\d+\s*[.)-]?\s*)?(?:NCs?|NCOP|NCED|NCOP\s*(?:&|and|\+)?\s*NCED|Creditless(?:\s.*)?|Textless(?:\s.*)?|Clean\s+(?:OP|ED|Openings?|Endings?).*|Extras?|Bonus(?:es)?|Bonus\s+.*|Specials?|SPs?|OVAs?|Menus?|CMs?|PVs?|Trailers?|Previews?|Omake|Features?|Commercials?|Promos?)\s*$")
        .is_match(folder)
}

/// A file name that names an extra rather than an episode: creditless
/// openings/endings, menus, previews, PVs, CMs, numbered specials.
fn extra_name(name: &str) -> bool {
    // Short tokens are case-sensitive ("OP", "ED", "SP"): lowercase "ed"/"op" occur in real titles.
    // CRC32 tags ("[5B1F11ED]") are dropped first: their hex letters would read as "ED"/"CM".
    static CRC: OnceLock<Regex> = OnceLock::new();
    static SHORT: OnceLock<Regex> = OnceLock::new();
    static WORDS: OnceLock<Regex> = OnceLock::new();
    static NUMBERED: OnceLock<Regex> = OnceLock::new();
    let name = re(&CRC, r"[\[(][0-9A-Fa-f]{8}[\])]").replace_all(name, "");
    let name = name.as_ref();
    // A letter suffix ("NCED01a") only after NC tokens: "ED60f" is a codec/hash fragment.
    // The bare tokens must end the name part: number, bracket, dash or extension next
    // ("OP - 02", "ED25 [BD]", "Menu 2.m2ts"), not a word ("Makes Me OP Even", "[OP_HNK]",
    // "SP Professional - 07" is a sequel title).
    re(&SHORT, r"(?:^|[^A-Za-z0-9])(?:(?:NC\s?OP|NC\s?ED)[\s_-]?\d{0,3}[a-z]?(?:[^A-Za-z0-9]|$)|(?:OP|ED|PV|CM|SP|MENU|Menu)(?:[\s_-]?\d{1,3})?(?:$|[\s_]*[\[\]().]|[\s_]*-))").is_match(name)
        || re(&NUMBERED, r"(?i)(?:^|[^a-z])extras?\s+\d{1,3}(?:$|[^0-9])").is_match(name)
        || re(&WORDS, r"(?i)(?:^|[^a-z])(?:creditless|textless|trailer|teaser|preview|clean[\s_.-]+(?:opening|ending)|mini[\s_.-]+ep)(?:[^a-z]|$)").is_match(name)
}

/// True when the file at `path` (folders joined by `/` or `\`) is an extra.
pub(crate) fn is_extra(path: &str) -> bool {
    let mut parts = path.rsplit(['/', '\\']);
    let name = parts.next().unwrap_or(path);
    let extra = extra_name(name) || parts.any(extra_folder);
    if extra {
        tracing::trace!(path, "file is an extra");
    }
    extra
}

fn num(s: &str) -> Option<u32> {
    s.parse().ok().filter(|&n| n > 0)
}

/// Season (when the name has one) and episode of a file name the title
/// parser could not read. `None` when no rule applies.
pub(crate) fn fallback_episode(name: &str) -> Option<(Option<u32>, u32)> {
    static SXE: OnceLock<Regex> = OnceLock::new();
    static LEADING: OnceLock<Regex> = OnceLock::new();
    static EP_WORD: OnceLock<Regex> = OnceLock::new();
    static CJK: OnceLock<Regex> = OnceLock::new();
    static TAIL_GROUPS: OnceLock<Regex> = OnceLock::new();
    static TRAILING: OnceLock<Regex> = OnceLock::new();
    let stem = name.rsplit_once('.').filter(|(_, ext)| ext.len() <= 4 && !ext.is_empty()).map_or(name, |(stem, _)| stem);

    // "S01E06-Title", "Show.S02E10.1080p", "S01E21'": season + episode.
    if let Some(c) = re(&SXE, r"(?i)(?:^|[^A-Za-z0-9])S(\d{1,2})E(\d{1,4})(?:[^0-9]|$)").captures(stem) {
        if let Some(ep) = num(&c[2]) {
            tracing::trace!(name, rule = "SxxEyy", ep, "file episode");
            return Some((num(&c[1]), ep));
        }
    }
    // "12 - Title", "12. Title", "001 Title", "21": a leading number (not a year).
    if let Some(c) = re(&LEADING, r"^(\d{1,4})(?:v\d)?(?:[\s._-]|$)").captures(stem) {
        let digits = &c[1];
        let year = digits.len() == 4 && (digits.starts_with("19") || digits.starts_with("20"));
        if let (false, Some(ep)) = (year, num(digits)) {
            tracing::trace!(name, rule = "leading number", ep, "file episode");
            return Some((None, ep));
        }
    }
    // "Ep-20 Title", "Ep12", "Episode 5", "EP01eng".
    if let Some(c) = re(&EP_WORD, r"(?i)(?:^|[^A-Za-z])(?:ep|episode|episodio|épisode)[\s._-]*(\d{1,4})(?:[^0-9]|$)").captures(stem) {
        if let Some(ep) = num(&c[1]) {
            tracing::trace!(name, rule = "Ep prefix", ep, "file episode");
            return Some((None, ep));
        }
    }
    // "第3話", "第09章", "3話".
    if let Some(c) = re(&CJK, r"第\s*(\d{1,4})\s*[話话集回章羽]|(\d{1,4})\s*[話话]").captures(stem) {
        if let Some(ep) = c.get(1).or(c.get(2)).and_then(|m| num(m.as_str())) {
            tracing::trace!(name, rule = "CJK counter", ep, "file episode");
            return Some((None, ep));
        }
    }
    // "Show -04", "[HRS]BtX-Neo-14", "Megalo Box 08 (720p Blu-ray)": a number
    // ending the name once trailing [..] / (..) groups are dropped.
    let core = re(&TAIL_GROUPS, r"(?:\s*(?:\[[^\]]*\]|\([^)]*\)))+\s*$").replace(stem, "");
    if let Some(c) = re(&TRAILING, r"[\s_-](\d{1,3})(?:v\d)?\s*$").captures(&core) {
        if let Some(ep) = num(&c[1]) {
            tracing::trace!(name, rule = "trailing number", ep, "file episode");
            return Some((None, ep));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extras_are_not_episodes() {
        for path in [
            "[NoobSubs] Owarimonogatari NCOP03 (720p Blu-ray 8bit AAC).mp4",
            "[SallySubs] Sword Art Online II - NCED 4 [BD 1080p FLAC] [2D8F2BAA].mkv",
            "[FFF] Aldnoah Zero - OP01 [BD][720p-AAC][4250C446].mkv",
            "[Ironclad] Ame to Kimi to - ED2 [BD.1080p.AV1].mkv",
            "[-__-'] Macross Frontier Menu 2 [A7C1318D].m2ts",
            "[GHOST][1080p] Maria Watches Over Us s4 - SP10 [BD HEVC 10bit AC3][B9D3A22B].mkv",
            "[Exiled-Destiny]_The_Helpful_Fox_Senko-san_Mini_Ep11_[BD_720P_8Bit]_Subbed_(4F8781C7).mkv",
            "Extras/[-__-'] Girls und Panzer - Sensha Kouza 4 [BD 1080p FLAC] [8810BFCE].mkv",
            "Season 1/Creditless/Kaichou-wa Maid sama! -NCED2.mkv",
            "NC/[Lulu] SukaSuka - 12 [BD 1080p HEVC FLAC].mkv",
            "[WBDP] OP - 2 [BD][1080p-FLAC][HEVC] [CFAE4D16].mkv",
            "Skip Beat - Clean ED - 01.mkv",
            "[Doki-Chihiro]_Nogizaka_Haruka_no_Himitsu_-_OP_[1280x720_Blu-ray_FLAC][E9E83876].mkv",
            "[FFF] THE iDOLM@STER - ED25 [BD][1080p-FLAC][25D879A7].mkv",
            "Extras/CM/CM - 01.mkv",
            "[nonA] Sayonara Zetsubou Sensei - Extra 01 (BD 1024x576).mkv",
            "[tlacatlc6] Denpa Onna to Seishun Otoko Extra 1 (BD 1280x720 x264 AAC).mkv",
        ] {
            assert!(is_extra(path), "{path}");
        }
        for path in [
            "[SubsPlease] Mushoku Tensei S3 - 09 (1080p) [DDF202A0].mkv",
            "Season 2/[Reza] Food Wars - 05.mkv",
            "14 - Truth Illuminates Darkness.mkv",
            "Ep-06 The Land of Truth Tellers.mkv",
            "[Erai-raws] Toji no Miko - 12.5 [720p][Multiple Subtitle].mkv",
            "S01E12-A Real Hero [8D1F52E8].mkv",
            "Ending Song Collection/12 - A Happy Ending.mkv",
            "Bokura ga Ita - 23 [X264_AC3_VFR][GB_BIG5][DVDRip][5B1F11ED].mkv",
            "Bokura ga Ita - 19 [X264_AC3_ED60f][GB_BIG5][DVDRip][1FDA8DEC].mkv",
            "Hoshi no Kirby - 055 [OP_HNK].avi",
            "Season 04/Kirby - Right Back At Ya! (2001) - S04E05 - 081 - A Trashy Tale [SDTV][bit][ ]-OP HNK.mp4",
            "Unique Skill Makes Me OP Even at Level 1 - S01E10 [English Dub] [CR WEB-RIP 1080p HEVC-10 AAC] [95F2F223].mkv",
            "Prince of Stride - Alternative - 06 (1280x720 HEVC AAC).mp4",
            "K-ON!! 26 Extra Visit! (BD1080p AC3 10bit).mkv",
            "[Erai-raws] Peter Grill to Kenja no Jikan - Super Extra - 01 [1080p].mkv",
            "[Wasurenai] Yumeiro Patissiere SP Professional - 07 [XviD 704x400] [16E0E431].avi",
            "[CBM]_Michiko_&_Hatchin_-_06_-_The_Saudade_of_Fools,_Parte_2_[720p]_[2727ED5A].mkv",
            "[Baws] Attack on Titan - S04E20 - Memories of the Future (BD 1080p Hi10 FLAC) [55ED9D3E].mkv",
        ] {
            assert!(!is_extra(path), "{path}");
        }
    }

    #[test]
    fn reads_names_the_title_parser_leaves_unread() {
        let cases: &[(&str, Option<(Option<u32>, u32)>)] = &[
            ("S01E06-The Tulip Lion [96179372].mkv", Some((Some(1), 6))),
            ("S03E12-Change your desires rather than the order of the world. [89F6DBBB].mkv", Some((Some(3), 12))),
            ("[Kirika] Shinseiki Evangelion (1995) - S01E21' (Director's Cut) [BD 1080p AV1] [6C5C152A].mkv", Some((Some(1), 21))),
            ("A.Certain.Magical.Index.II.S02E10.mkv", Some((Some(2), 10))),
            ("19 - Satokowashi-hen - Village-Destroying Chapter (Part 2).mkv", Some((None, 19))),
            ("1106 - Trouble Occurs! Seek Dr. Vegapunk! [817D7138].mkv", Some((None, 1106))),
            ("12. Claymore [BD 540p Hi10p AAC dual-audio][kuchikirukia].mkv", Some((None, 12))),
            ("21.mkv", Some((None, 21))),
            ("001 Calm.Down.Big.G_Hello.Martians.mp4", Some((None, 1))),
            ("2019 - Some Movie.mkv", None),
            ("Ep-20 I'll Do My Best, Mustang!.mkv", Some((None, 20))),
            ("Da Wang Rao Ming – Spare Me, Great Lord! EP01eng.mp4", Some((None, 1))),
            ("ゼノンザード THE ANIMATION 3話　Nonoin Idle Curiosity.mp4", Some((None, 3))),
            ("[NAZOrip] 異世界はスマートフォンとともに。 第09章 オエド、そして不死の宝玉。  (BDRip.HEVC.1080P.YUV420P10.QAAC).mkv", Some((None, 9))),
            ("Kamisama Hajimemashita -04.mp4", Some((None, 4))),
            ("[NoobSubs] Megalo Box 08 (720p Blu-ray 8bit AAC).mp4", Some((None, 8))),
            ("[HRS]BtX-Neo-14.mkv", Some((None, 14))),
            ("Voltage.Fighter.Gowcaizer.FanCut.x264.[DVD2HD].mkv", None),
            ("Digital Works.mkv", None),
        ];
        for (name, want) in cases {
            assert_eq!(fallback_episode(name), *want, "{name}");
        }
    }
}
