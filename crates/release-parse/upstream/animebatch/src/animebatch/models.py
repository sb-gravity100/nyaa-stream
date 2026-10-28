from __future__ import annotations
from dataclasses import dataclass, field, asdict
from typing import Optional


@dataclass
class Release:
    raw: str
    title: str = ""
    aliases: list = field(default_factory=list)
    group: Optional[str] = None
    seasons: list = field(default_factory=list)   # seasons explicitly covered
    season_source: str = "none"                   # explicit | range | list | parser | none
    ep_start: Optional[int] = None
    ep_end: Optional[int] = None
    ep_total: Optional[int] = None
    batch_kind: Optional[str] = None              # see rules.BATCH_KINDS
    is_complete: bool = False
    has_movies: bool = False
    has_extras: bool = False
    year: Optional[int] = None
    resolution: Optional[str] = None
    source: Optional[str] = None
    confidence: float = 0.0
    trace: list = field(default_factory=list)     # which rules fired (debug)

    @property
    def is_batch(self) -> bool:
        return self.batch_kind is not None

    def covers_season(self, n: int, include_complete: bool = True):
        """True / False, or None when the release carries no season info."""
        if self.seasons:
            return n in self.seasons
        if self.is_complete:
            return include_complete
        return None

    def covers_episode(self, n: int) -> bool:
        if self.ep_start is None:
            return True
        return self.ep_start <= n <= (self.ep_end if self.ep_end is not None else self.ep_start)

    def to_dict(self, trace: bool = False) -> dict:
        d = asdict(self)
        d["is_batch"] = self.is_batch
        if not trace:
            d.pop("trace")
        return d
