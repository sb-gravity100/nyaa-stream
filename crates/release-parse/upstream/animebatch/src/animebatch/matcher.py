from dataclasses import dataclass
from typing import Iterable, List

from . import rules as R
from .models import Release
from .parser import parse

try:
    from rapidfuzz import fuzz

    def _ratio(a, b):
        return fuzz.ratio(a, b)
except ImportError:  # fallback keeps the lib dependency-light
    from difflib import SequenceMatcher

    def _ratio(a, b):
        return 100 * SequenceMatcher(None, a, b).ratio()


@dataclass
class Match:
    release: Release
    score: float
    reason: str


def search(releases: Iterable, title: str, season: int = None, episode: int = None, *,
           include_complete: bool = True, unknown: str = "s1", threshold: float = 88,
           batches: str = "include") -> List[Match]:
    """Season is a HARD filter on parsed metadata; title is a fuzzy match on aliases.

    unknown : release has no season info ->  's1' (treat as season 1) | 'include' | 'exclude'
    batches : 'include' | 'only' | 'exclude'
    """
    q, out = R.norm(title), []
    for rel in releases:
        rel = parse(rel) if isinstance(rel, str) else rel
        score = max((_ratio(q, R.norm(x)) for x in (rel.aliases or [rel.title])), default=0)
        if score < threshold:
            continue
        if (batches == "exclude" and rel.is_batch) or (batches == "only" and not rel.is_batch):
            continue
        reason = "title"
        if season is not None:
            cs = rel.covers_season(season, include_complete)
            if cs is None:
                if unknown == "include":
                    reason = "unknown_season"
                elif unknown == "s1" and season == 1:
                    reason = "implicit_s1"
                else:
                    continue
            elif not cs:
                continue
            else:
                reason = ("complete" if not rel.seasons else
                          "multi_season" if len(rel.seasons) > 1 else "season")
        if episode is not None and not rel.covers_episode(episode):
            continue
        out.append(Match(rel, score, reason))
    return sorted(out, key=lambda m: (-m.score, m.release.is_batch))
