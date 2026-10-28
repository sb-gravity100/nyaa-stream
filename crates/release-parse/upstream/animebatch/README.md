# animebatch

Hybrid release-title parser: **custom batch/season-range rules** + guessit (title, season) + anitopy (episodes),
with a season-aware search. Built from two nyaa.si crawls (3,750 titles).

```python
from animebatch import parse, search
r = parse("[SubsPlease] Nige Jouzu no Wakagimi S2 (01-12) (480p) [Batch]")
r.seasons, r.ep_start, r.ep_end, r.batch_kind   # ([2], 1, 12, 'episode_range')
hits = search(titles, "Nige Jouzu no Wakagimi", season=2)   # season = hard filter on parsed metadata
```
CLI: `python -m animebatch crawl.json --title "Tantei wa Mou, Shindeiru" --season 2 [--debug]`

## batch_kind (precedence order)
`multi_season` (Season 1-3, S1-S4, S1 & S2) > `complete` (Complete, 全集, 合集) > `box_set` (Box Set, Vol 1-9)
> `episode_range` (01-12, 001 ~ 167, Eps 1-169, E01-E24, [1-162 of 220], Ep.194-652) > `movie_pack`
> `season_pack` (season, no episode) > `batch_tag` ([Batch] only) > `disc_pack` (BD/DVD, no episode; low confidence) > `None` (single).

## Rules that matter
- Noise stripped before number-hunting: resolution, codecs, audio (5.1), years, hashes, `v2`.
- `Season 04-09` is a range, `Season 2 - 04` is season 2 + episode 4 (spaced dash).
- Spaced `02 - 05` is NOT a range unless 3+ digits or a batch keyword (`~` and unspaced dashes are safe).
- Explicit `SxxEyy` wins over `1-2` in titles (Ranma 1/2) and over "2nd Season" in alias parentheses.
- Parser seasons > 60 are ignored (guessit reads `(2009)` as season 2009); parser episodes must be real tokens
  (not `TRUHD.51`).
- Packs never fall back to guessit/anitopy single-episode guesses.

## Search semantics
`Release.covers_season(n)` -> True / False / None (unknown). `search(..., unknown='s1'|'include'|'exclude',
include_complete=True, batches='include'|'only'|'exclude', threshold=88)`. Title match = fuzzy ratio over aliases
(guessit title, anitopy title, `/`-split bilingual names, non-technical parentheses).

## Debugging
`Release.trace` lists the rules that fired. `logging.getLogger("animebatch")` DEBUG logs every parse.
`examples/audit.py a.json b.json` writes `audit.log` and prints class counts, SxxEyy regression, low-confidence count.

## Known limits
Season hidden in the title (`Edgerunners 2`), absolute numbering (ep 25 = S2E1), Part/Cour mapping, and alias
discovery across different series titles need a metadata source (AniList/TVDB). `disc_pack`/`batch_tag` without a
season return `None` from `covers_season`.
