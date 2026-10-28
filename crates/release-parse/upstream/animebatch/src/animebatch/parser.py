"""Hybrid parser: custom batch rules first, then guessit (title/season), then anitopy (episodes)."""
import logging
import re
from typing import Iterable, List

import anitopy
from guessit import guessit

from . import rules as R
from .models import Release

log = logging.getLogger("animebatch")


def _first(v):
    return v[0] if isinstance(v, list) and v else v


def _int(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return None


def _ints(v, hi=3000):
    vals = [_int(x) for x in (v if isinstance(v, list) else [v])]
    return [x for x in vals if x is not None and 0 <= x <= hi]


def _ep_in_raw(raw, n):
    """Parser episode must appear as a real token ('- 05', '[05]', 'E05'), not inside '5.1' or 'TRUHD.51'."""
    return bool(re.search(rf"(?:(?<=[\s\-\[(])|(?<=E)|(?<=EP)|(?<=Ep)|(?<=^))0*{n}(?:v\d)?(?=[\s\])\[]|$)", raw))


def _guessit(raw):
    try:
        return dict(guessit(raw, {"type": "episode"}))
    except Exception as e:  # guessit can raise on odd input
        log.warning("guessit failed on %r: %r", raw, e)
        return {}


def _anitopy(raw):
    try:
        return anitopy.parse(raw) or {}
    except Exception as e:
        log.warning("anitopy failed on %r: %r", raw, e)
        return {}


def parse(raw: str) -> Release:
    r, t = Release(raw=raw), None
    t = r.trace
    g, a = _guessit(raw), _anitopy(raw)
    c = R.clean(raw)
    f = R.flags(raw)
    r.year = R.find_year(raw)
    r.group = a.get("release_group")
    r.resolution = str(_first(g.get("screen_size"))) if g.get("screen_size") else None
    r.source = str(_first(g.get("source"))) if g.get("source") else None
    r.has_movies, r.has_extras, r.is_complete = f["movie"], f["extra"], f["complete"]

    # ---- seasons: own rules > SxxEyy > guessit > anitopy ----
    seasons, skind, rule = R.find_seasons(c)
    sxe = R.SXXEYY.search(raw)
    if sxe and skind not in ("range",) and not R.SR_S.search(raw):
        r.seasons, r.season_source = [int(sxe[1])], "explicit"
        t.append(f"season:sxxeyy={sxe[1]} (overrides {seasons})" if seasons and seasons != [int(sxe[1])] else f"season:sxxeyy={sxe[1]}")
    elif seasons:
        r.seasons, r.season_source = seasons, skind
        t.append(f"season:{rule}={seasons}")
    elif (m := R.SXXEYY.search(raw)):
        r.seasons, r.season_source = [int(m[1])], "explicit"
        t.append(f"season:sxxeyy={m[1]}")
    else:
        gs = _ints(g.get("season"), 60) or _ints(a.get("anime_season"), 60)  # year-as-season guard
        if gs:
            r.seasons, r.season_source = sorted(set(gs)), "parser"
            t.append(f"season:parser={r.seasons}")

    # ---- episodes: own ranges > SxxEyy > anitopy > guessit ----
    pack = f["complete"] or f["box"] or f["movie_range"] or f["vol_range"] or len(r.seasons) > 1
    rng = R.find_ep_range(c, kw=f["batch"] or f["complete"], allow_bare=not R.SXXEYY.search(raw))
    if rng:
        r.ep_start, r.ep_end, r.ep_total, rule = rng
        t.append(f"ep:{rule}={rng[:3]}")
    elif (m := R.SXXEYY.search(raw)):
        r.ep_start = r.ep_end = int(m[2])
        t.append("ep:sxxeyy")
    elif not pack:
        ae = _ints(a.get("episode_number"))
        ge = _ints(g.get("episode"))
        for src, vals in (("anitopy", ae), ("guessit", ge)):
            vals = [v for v in vals if _ep_in_raw(raw, v)]
            if vals:
                r.ep_start, r.ep_end = min(vals), max(vals)
                t.append(f"ep:{src}={vals[:2]}")
                break
    else:
        t.append("ep:skipped(pack)")

    # ---- batch classification (precedence = rules.BATCH_KINDS) ----
    if len(r.seasons) > 1:
        kind = "multi_season"
    elif f["complete"]:
        kind = "complete"
    elif f["box"] or f["vol_range"]:
        kind = "box_set"
    elif rng and rng[0] < rng[1]:
        kind = "episode_range"
    elif f["movie_range"]:
        kind = "movie_pack"
    elif r.seasons and r.ep_start is None:
        kind = "season_pack"
    elif f["batch"]:
        kind = "batch_tag"
    elif r.ep_start is None and f["disc"] and not f["movie"]:
        kind = "disc_pack"
    else:
        kind = None
    r.batch_kind = kind
    t.append(f"kind:{kind}")

    # ---- title + aliases ----
    gt, at = _first(g.get("title")), a.get("anime_title")
    r.title = str(gt or at or raw)
    r.aliases = R.build_aliases(raw, [gt, at], r.group)

    r.confidence = _confidence(r, rng, kind)
    log.debug("%s -> %s", raw, t)
    return r


def _confidence(r, rng, kind):
    if kind in ("disc_pack", "batch_tag") and not r.seasons and not rng:
        return 0.4
    if rng or r.season_source in ("explicit", "range", "list"):
        return 0.9
    return 0.6


def parse_many(titles: Iterable[str]) -> List[Release]:
    return [parse(x) for x in titles]
