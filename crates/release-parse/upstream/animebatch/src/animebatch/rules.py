"""Regex rules. All episode/season range logic lives here (the part guessit/anitopy get wrong)."""
import re, unicodedata

SEP = r"[-~～–—]"
BATCH_KINDS = ("multi_season", "complete", "box_set", "episode_range",
               "movie_pack", "season_pack", "batch_tag", "disc_pack")  # precedence order


def _c(p, f=re.I):
    return re.compile(p.replace("SEP", SEP), f)


# ---- noise removed before looking for numbers (resolution, codecs, years, hashes...) ----
WXH = _c(r"\b\d{3,4}\s*[x×]\s*\d{3,4}\b")
NOISE = [
    WXH,
    _c(r"\b\d{3,4}[pi]\b"),
    _c(r"\b[xh]\.?26[45]\b"),
    _c(r"(?<!\d)(?:19|20)\d{2}(?!\d)"),
    _c(r"\b[0-9A-F]{8}\b", 0),
    _c(r"\b\d+\s*-?\s*bit\b"),
    _c(r"\b\d\.\d\b"),
    _c(r"\b(?:AAC|AC3|EAC3|DDP|FLAC|DTS|TrueHD|TRUHD|Opus|PCM|AV1|AVC|HEVC|VP9|Hi10)\w*"),
    _c(r"(?<=\d)v\d\b|\bv\d\b"),
]
YEAR = _c(r"(?<!\d)((?:19|20)\d{2})(?!\d)")


def clean(s: str) -> str:
    for p in NOISE:
        s = p.sub(" ", s)
    return s


def find_year(s: str):
    m = YEAR.search(WXH.sub(" ", s))
    return int(m.group(1)) if m else None


# ---- episode ranges ----
EP_OF = _c(r"[\(\[]\s*(\d{1,4})\s*SEP\s*(\d{1,4})\s+of\s+(\d{1,4})\s*[\)\]]")
EP_KW = _c(r"(?<![A-Za-z])(?:Episodes?|Eps?|E)\.?\s*(\d{1,4})\s*SEP\s*(?:Episodes?|Eps?|E)?\.?\s*(\d{1,4})(?!\d)")
EP_BRACKET = _c(r"[\(\[\{]\s*(\d{1,4})\s*SEP\s*(\d{1,4})(?!\d)")
EP_BARE = _c(r"(?<![\w.])(\d{1,4})(\s*)(SEP)(\s*)(\d{1,4})(?!\d)")
EXCL = _c(r"(?<![A-Za-z])(?:seasons?|movies?|shorts?|ovas?|oads?|vol(?:ume)?s?|parts?|discs?|box|specials?|sp|s|e|ep|eps|episodes?)[\s.]*$")
VOL = _c(r"(?<![A-Za-z])Vol(?:ume)?s?\.?\s*\d{1,3}\s*SEP\s*(?:Vol(?:ume)?s?\.?\s*)?\d{1,3}")
MOVIE_RANGE = _c(r"(?<![A-Za-z])Movies?\s*\d{1,3}\s*SEP\s*\d{1,3}")
SXXEYY = _c(r"(?<![A-Za-z])S(\d{1,2})\s*E(\d{1,4})(?!\d)")


def _ok(a, b):
    return 0 <= int(a) < int(b) <= 3000


def find_ep_range(c: str, kw: bool = False, allow_bare: bool = True):
    """Return (start, end, total, rule) or None. `kw` = a batch/complete keyword is present;
    `allow_bare` False when an explicit SxxEyy exists (then '1-2' in the title is not an episode range)."""
    m = EP_OF.search(c)
    if m and _ok(m[1], m[2]):
        return int(m[1]), int(m[2]), int(m[3]), "ep_of_total"
    for name, p in (("ep_kw", EP_KW), ("ep_bracket", EP_BRACKET)):
        if name == "ep_bracket" and not allow_bare:
            continue
        for m in p.finditer(c):
            if _ok(m[1], m[2]):
                return int(m[1]), int(m[2]), None, name
    pos = 0
    while allow_bare and (m := EP_BARE.search(c, pos)):
        pos = m.start() + 1                      # overlapping scan: "2 - 01 ~ 10" must still find "01 ~ 10"
        a, sp1, sep, sp2, b = m.groups()
        if not _ok(a, b) or EXCL.search(c[:m.start()]):
            continue
        # " 02 - 05 " is usually "title 02 - episode 05": trust spaced hyphen only with 3+ digit
        # numbers or a batch keyword; "~" and unspaced dashes are safe.
        if (sp1 or sp2) and sep == "-" and not (kw or (len(a) >= 3 and len(b) >= 3)):
            continue
        return int(a), int(b), None, "ep_bare"
    return None


# ---- seasons ----
SR_W = _c(r"(?<![A-Za-z])Seasons?\s*(\d{1,2})(?:SEP|\s*~\s*)(?:Seasons?\s*)?(\d{1,2})(?!\d)")
SR_S = _c(r"(?<![A-Za-z])S(\d{1,2})\s*SEP\s*S(\d{1,2})(?!\d)")
S_TOK = _c(r"(?<![A-Za-z])(?:S|Seasons?\s*)(\d{1,2})(?![\dA-Za-z])")
S_ORD = _c(r"(\d{1,2})(?:st|nd|rd|th)\s+Season")
S_PART = _c(r"(?<![A-Za-z])S(\d{1,2})\s*Part\s*\d")


def find_seasons(c: str):
    """Return (sorted seasons, kind, rule). kind: explicit | range | list | None."""
    seasons, kind, rule, spans = set(), None, None, []
    for name, p in (("season_range_word", SR_W), ("season_range_s", SR_S)):
        for m in p.finditer(c):
            a, b = int(m[1]), int(m[2])
            if 0 <= a < b <= 60:
                seasons.update(range(a, b + 1)); kind, rule = "range", name; spans.append(m.span())
    for a, b in spans:
        c = c[:a] + " " * (b - a) + c[b:]
    for name, p in (("season_token", S_TOK), ("season_ordinal", S_ORD), ("season_part", S_PART)):
        for m in p.finditer(c):
            seasons.add(int(m[1])); rule = rule or name
    if seasons and kind is None:
        kind = "list" if len(seasons) > 1 else "explicit"
    return sorted(seasons), kind, rule


# ---- keyword flags ----
_F = {
    "complete": _c(r"\bcomplete(?:\s+series)?\b|全集|合集|\bcollection\b|\bfull\s+series\b"),
    "batch": _c(r"\bbatch\b"),
    "box": _c(r"\bbox\s*set\b|\bboxset\b|\bBD-?BOX\b|\b(?:BD|DVD|Blu-?ray|Dragon|Complete)\s*Box\b|\bBox\s*(?:\d+|[IVX]+)\b"),
    "disc": _c(r"\bBD\b|BDRip|BDMV|Blu-?ray|\bREMUX\b|\bDVD(?:ISO|Rip)?\b|\bUSBD\b|\bBDISO\b"),
    "movie": _c(r"\bMovies?\b|\bFilms?\b|劇場版|剧场版|\d+Movies?\b"),
    "extra": _c(r"\bSpecials?\b|\bOVAs?\b|\bOADs?\b|\bExtras?\b|\bShorts?\b|\bSP\b|特典|\bNCOP\b|\bNCED\b"),
}


def flags(raw: str) -> dict:
    d = {k: bool(p.search(raw)) for k, p in _F.items()}
    d["vol_range"] = bool(VOL.search(raw))
    d["movie_range"] = bool(MOVIE_RANGE.search(raw))
    return d


# ---- aliases ----
TECH = _c(r"\b(?:\d{3,4}[pi]|BD|BDRip|BDMV|WEB|WEB-?DL|Dual|Multi|Subs?|Audio|HEVC|AAC|FLAC|x26\d|Batch|Complete|"
          r"Remux|Blu-?ray|DVD|MKV|MP4|Eng(?:lish)?|Dub(?:bed)?|JP|US|CR|NF|Fin|Weekly|Hi10P?|v\d|Season|Seasons|Part|"
          r"Vol|Movies?|Specials?|Extras?|OVA|Series|Edited|Content|Chinese|ITA|Esp|4K|NCOP|NCED)\b"
          r"|^\W*\d+(?:\W+\d+)?\W*$|^S\d|字幕|内嵌|内封|简|繁|合集|全集|特典|映像|[0-9A-F]{8}")
_SEG = re.compile(r"[\[\(【{]([^\]\)】}]{2,120})[\]\)】}]")
_SEASON_TXT = _c(r"\bS\d{1,2}\b|\bSeasons?\s*\d+(?:\s*SEP\s*\d+)?")


def norm(s: str) -> str:
    s = unicodedata.normalize("NFKC", s).casefold()
    s = re.sub(r"[^\w\s]|_", " ", s)
    return re.sub(r"\s+", " ", s).strip()


def build_aliases(raw: str, titles, group=None, limit: int = 8):
    cands = []
    for t in titles:
        if t:
            cands += re.split(r"\s+/\s+|／", str(t))
    for m in _SEG.finditer(raw):
        seg = m.group(1).strip()
        if group and seg == group:
            continue
        if TECH.search(seg) or not re.search(r"[^\W\d_]", seg):
            continue
        cands += re.split(r"\s*/\s*", seg) if re.search(r"[^\x00-\x7F]", seg) else [seg]
    out, seen = [], set()
    for t in cands:
        t = _SEASON_TXT.sub(" ", t)
        t = re.sub(r"\s*[-–~]\s*$", "", re.sub(r"\s+", " ", t)).strip(" -–~._|")
        n = norm(t)
        if len(n) < 2 or n.isdigit() or n in seen:
            continue
        seen.add(n); out.append(t)
    return out[:limit]
