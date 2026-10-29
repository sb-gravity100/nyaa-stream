# Anime Filename Mapper - PLAN

## Goal
Query (title + season) -> only matching files from a large filename set (single episodes AND batches).

## Status
- [x] M1 Rule-based parser + season filter (guessit + anitopy compared on 2,250 titles)
- [x] M1b Batch rules from 2nd crawl (1,500 batch-heavy titles) -> `animebatch/` library, 42 tests
- [ ] M2 Hand-labeled hard set (~200): bare "- 03" that is really S2, season in title, absolute numbering
- [ ] M3 Alias/season mapping via AniList/TVDB

## Design
parse (own batch rules > SxxEyy > guessit > anitopy) -> aliases -> fuzzy title match -> HARD season filter.

## Debugging / Logging
Release.trace per rule; logger "animebatch"; examples/audit.py -> audit.log.

## Open questions
- Default for unknown season: 's1' (current) vs 'include'.
- Treat disc_pack (BD, no season, no episode) as complete series or S1?
