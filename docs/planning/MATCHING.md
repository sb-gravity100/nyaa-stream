# Release matching and dataset specifications

Architecture, measurements and feature history for release matching and datasets.
The learned matcher remains planned for v0.6.1. Current roadmap: [PLAN.md](../../PLAN.md).

## Season-aware source matching (v0.4.2 parsing + db import; matching ships in v0.5.0, was v0.4.3 - patch: fix)

**Scope split (2026-10-06).** v0.4.2 ships the parsing half: the
`release-parse` crate (below), `EpisodeLabel` on every search result and
play file, and importing a nyaa.si release database. v0.4.3 does the
matching half: season sets, TVDB seasons, sibling routing, and view-page
file lists.

**Importing a nyaa.si database** (v0.4.2). Settings → "Import nyaa.si
database" picks a SQLite file with nyaa-stream's own `releases` schema
(`store.rs`), for example one built by the `build_dataset` example from
nyaasi_extractor crawl JSON. `Store::import` ATTACHes it and upserts its
`releases` rows by nyaa id. A row already stored wins unless the imported
one is fresher (`updated_at`). `words` is recomputed, not trusted.
`searches` / `search_hits` are *not* imported: they claim "this query was
fetched at T", which only holds for this app's own fetches. Imported
releases still answer `search_local` (offline/429 fallback, instant first
results). Files without a compatible `releases` table are refused with a
message.


**Problem** (measured 2026-10-04 on nyaa.si crawls made with the
`nyaasi_extractor` Chrome extension, a sibling project; matcher ported to
Python and run offline):

1. *Real releases dropped.* `matches_show` needs one of the entry's full
   names as a contiguous phrase, but groups write the short name plus a
   season marker: `[Breeze] Mushoku Tensei S03E12`, `[MiniMTBB] Mushoku
   Tensei S3 - 14 | Jobless Reincarnation`, `[DKB] Mushoku Tensei: Isekai
   Ittara Honki Dasu - S03E14`. Mushoku Tensei Season 3: **109 of 375**
   real releases dropped (all carry an explicit S3 marker). On the query
   side, the romaji `Mushoku Tensei III: Isekai…` keeps its `III` (only
   "Season/Cour/Part" are stripped), and synonyms are never stripped
   (`…3rd Season`), so the app's own queries return only 310 of the 375.
   Season batches, which are named by season and not called "batch"
   (`[SubsPlease] Mushoku Tensei S2`, `[RUDY] Mushoku Tensei S02`,
   `[MTBB] Mushoku Tensei S1`, `[Lia] Mushoku Tensei (Season 1 - Part 1)`),
   are dropped the same way.
2. *Wrong-season releases kept.* The season-stripped base name ("That Time
   I Got Reincarnated as a Slime") matches every season. Slime **Season 3**
   accepts 658 Season 4 releases. Because episodes group by number, S4's
   episodes then sit under S3's episode numbers. The Mushoku S3 page also
   accepts S1 and S2 BD batches.
3. *Seasons named without a number.* Food Wars! names its seasons "The
   Second Plate" / "Ni no Sara" / … The S1 entry's name "Food Wars!" is a
   prefix of every sequel's, so the S1 page accepts S2-S5 batches.
   `episodeParser.ts` labels those batches `unknown` (37 of the 75 biggest
   Food Wars releases), even though the matched name pins the season.
4. *Parser gaps* (`episodeParser.ts`, same data): tilde ranges
   (`01 ~ 11`, `00~12`) come out `unknown`; `Mushoku Tensei II … 00 ~ 12`
   is labelled season 1 (roman numerals aren't read); `S02E00-E06` and
   `S00E02-E03` are labelled single episodes, not ranges.

**Fix** (`src-tauri/src/title_match.rs` + the parser - done in
`crates/release-parse` in v0.4.2, which replaced `episodeParser.ts`'s
parsing; the parser bullets below are implemented there):

**Parser design: anitopy-style, in Rust** (decided 2026-10-06). Release
titles are parsed by one Rust crate (`crates/release-parse`, a fork/port of
anitopy 2.1.1, decided 2026-10-06), modelled on [anitopy](https://github.com/igorcmoura/anitopy) (the Python
port of Anitomy). It tokenizes on brackets and delimiters, uses keyword
tables (episode/season prefixes, video/audio/source terms, type words), and
returns named elements: `title`, `seasons`, `episodes` (number or range),
`kind`, `group`, `version`. Both the matcher (`title_match.rs` compares the
extracted `title` against AniList names, not the whole string) and the
frontend (the elements ride along on `NyaaResult`, replacing the parsing
in `episodeParser.ts`) use it, so the two can't disagree.

anitopy itself, run over the 8,038-title corpus: it gets `2nd Season`,
`S04E29-31`, `S3 - 12`, `Movie`/`OVA` right. It gets wrong exactly the
TITLE_ANALYSIS cases: `Mob Psycho 100 III` (numeral stays in the title, no
season), `01 ~ 12` (episode 01 only), `第19话` (no episode), `S01-04` /
`Season 1-4` (episodes 1-4), `S04E29~E31` (nothing), and arc names left in
the title (`Jujutsu Kaisen - Kaigyoku Gyokusetsu`). Those are added on top
of its rules. The crate is a port, so it is a derived work: anitopy is
**MPL-2.0**, so the crate's files are MPL-2.0 too, with headers, a
`LICENSE` and an attribution in its README. MPL is per-file copyleft, so
the rest of the app's licensing is unaffected. Tested against the existing
fixtures (`src-tauri/tests/fixtures/nyaa_titles/`) plus the survey corpus
with expected elements.

Title formats this must handle are surveyed in `TITLE_ANALYSIS.md`
(8,038 titles, 42 franchises). Section numbers below (§) refer to it.

- *Entry season numbers are a set* (§5: groups number seasons
  differently; Demon Slayer's Swordsmith Village is `S3` on Crunchyroll
  and `S4` for Salieri/TVDB). The set holds:
  - its TVDB season from Fribb/anime-lists (`anime-list-full.json`,
    `anilist_id` → `season.tvdb`). The repo has no license, so the file is
    fetched at runtime and cached on disk, refreshed weekly, not bundled.
  - the number in its own AniList titles ("Season N", "Nth Season", "SN",
    a roman numeral II-X after the name).
  - the AniList TV prequel chain, as a **first-season flag only**
    (decided 2026-10-08): a TV entry with no TV/TV_SHORT prequel (walked
    through OVAs/movies) is season 1. The prequel *count* is not used: split
    cours ("Cour 2", "Part 2") are separate entries, so Mushoku Tensei III
    sits behind four TV entries but is season 3, and a wrong number would
    reject real releases. Entries with a prequel and no other source get no
    season set and keep today's rules.
  The chain walk is `AniListClient::franchise` (PREQUEL/SEQUEL chains, side
  stories, spin-offs, alternatives; titles and formats), cached a week per
  entry in `<cache>/nyaa-stream/franchise/<id>.json` (`franchise_cache.rs`);
  the full search loads it while nyaa.si is queried, the local and
  fansubber-only searches use a cached copy only.

  "Part/Cour N" entries share their season's number.
- *Season markers in a release* (§2): `S03`, `S3`, `Season 3`, `3rd
  Season`, a roman numeral right after a known name (`Mushoku Tensei II`,
  `Mob Psycho 100 III`, but not `Infinity Castle I`), `S04Part01`, and
  multi-season packs (`S01-S04`, `S1-3`, `S01-05`, `Season 1-4`,
  `Season 1 - 3`, `S1+S2+S3`, `Season 01 + Season 02`, `ALL SEASONS`).
  Ranges are parsed on the raw title, before `normalize` drops the
  separators: a prototype that parsed normalized text read "2nd Season -
  01 ~ 12" as seasons 1-12. `Season N - M` (Doomdos) is season N,
  episode M, not a range. **An `SxxEyy` marker outranks every other
  marker**: VARYG writes `S04E03 … (… 3rd Season)`.
- *Normalization* (§4): apostrophes (`'`, `’`, `` ` ``) are removed, not
  separators. Today `Journey's` becomes `journey s`, so `Frieren Beyond
  Journeys End` never matches; same for `God's`/`Gods` (Konosuba). Glued
  numbers on a known name (`danmachi5`) split off.
- *Matching rule*: reject a release whose markers name only other seasons.
  Otherwise keep it if it has a full name (today's rule), or if it has a
  short base name (the season-stripped name, the part before the first
  `:`, roman numeral removed; min 2 words / 5 chars) plus a marker for
  this season. A release with a base name and no marker counts as
  season 1. Releases with no marker keep today's behaviour, because
  absolute-numbered releases (Slime/Doomdos, JJK) depend on it.
- *Sibling entries*: once the prequel/sequel chain's titles are known, a
  release that matches a sibling's longer, more specific name ("Food Wars!
  The Second Plate") belongs to that sibling. The longest name wins.
- *Queries*: apply the same season stripping (with roman numerals) to
  synonyms. For season ≥2 entries, add the short base name ("Mushoku
  Tensei") as a candidate. Its words are a subset of the full names', so
  `build_candidates` drops those, and the query count doesn't grow.
- *Batches*: a matched release with no episode number is a season batch
  of the entry it matched (a season-specific name pins the season, e.g.
  "The Second Plate").
- *Parser fixes* (`episodeParser.ts`, §1-§2, with counts of titles that
  are wrong today):
  - roman-numeral seasons: 265 `Mob Psycho 100 III - 10` episodes are
    filed as season 1.
  - multi-season packs read as one episode (98).
  - `~` ranges, `01 ~ 25` / `[1~25]` / `00~12`: 113 unknown.
  - `Nth Season` titles: 43 unknown.
  - `SxxEaa-bb` / `SxxEaa-Ebb` / `SxxEaa~Ebb` read as one episode (45).
  - CJK `第19话/話/集` episodes read as batches (45).
  - `Sx - ep` read as a batch (48).
  - `Season 04 - 32-36` read as one episode.
  - `#23` episodes; `.5` episodes kept off the number grid.
- *Kinds kept off a TV season's episode grid* (§3): movies, OVA/OAD/ONA/
  specials, `S00`, recaps and re-edits (`Chronicle`, `Shin Henshuu-ban`).
  They go to their own AniList entry by name (longest name wins). A TV
  entry keeps them only inside a multi-season pack that names its season.
  Spin-offs that share the name (`Mr. Ginpachi's Zany Class`, `Gun Gale
  Online`, `Junior High`) are routed the same way.
- *Unsure → read the view page* (requested 2026-10-04). Some titles say
  nothing about their contents. Example: `[Reza] Shokugeki no Souma
  [BDRip 1080p HEVC FLAC] (Dual Audio) (Food Wars)` (157 GiB) has no
  season, episode or "batch". Its view page (`/view/1465193`) does: folders
  `Season 1`…`Season 5`, 91 files `[Reza] Food Wars S01E01.mkv` …
  `S05E13.mkv`. `get_torrent_details_batch` already scrapes view pages for
  unresolved titles, but `TorrentDetails` keeps only `file_count`. Extend
  it with the file list (path incl. folders, size) and parse each file name
  with the same season/episode rules (a folder name like `Season 2` gives
  the files under it their season). The release's seasons are the union.
  It is kept for the browsed entry if that union contains the entry's
  season, as a batch whose matching files are known (so play can pick the
  right file index), and dropped otherwise.
  *Unsure* means a release that matched but whose title gives no season
  marker and no episode number (828 titles, 10% of the survey: §6), or
  matched only via a sibling-ambiguous name, or whose season marker is in
  two siblings' sets. It also covers what the title-only rules mis-assign (`[ASW]
  Mushoku Tensei (Batch)`). Scrapes are cached per release id in the
  nyaa-client store (`store.rs`), so each view page is fetched once ever,
  under the existing throttle and bounded concurrency.

Prototype results on the crawls: Mushoku S3 266 → **375/375**. Slime S4
drops the 9 wrong-season batches it used to accept. Slime S2/S3 pages
stop accepting 600+ S4 episodes. Food Wars and the sibling rule are
specified here but not yet prototyped.

**Not covered**: telling apart split cours of one season that AniList lists
as separate entries (Mushoku S1 vs "Cour 2", S2 vs "Season 2 Part 2").
They share a season number, so both still get both cours' releases (same
as today). Groups whose numbering is in neither TVDB nor the entry's own
titles (`[Cleo] … San no Sara - Toutsuki Ressha-hen | Food Wars! S4`; TVDB
says 3) still mis-assign unless the name or file names say otherwise.

**Test data**: titles (+ sizes) from the crawls are committed as fixtures
(`src-tauri/tests/fixtures/nyaa_titles/`), with expected entry/season per
title for the Mushoku Tensei, Slime and Food Wars franchises. They're used
by `title_match.rs` unit tests. `corpus.txt` (10,151 unlabelled titles) is
the whole-corpus regression set for `release-parse`: diff
`examples/parse_lines.rs` / `examples/labels.rs` output before and after a
parser change and review every changed title.

**Mushoku S3 report (2026-10-13, v0.5.0).** Report: the Season 3 page
(AniList 178789) shows fewer sources than a manual nyaa crawl ("mushoku tensei
s03e01", 24 releases) and 14 stray season-1 "Episode N" rows next to the
"Season 3 Episode N" rows. Reproduced offline on a copy of `nyaa.db` with the
`#[ignore]` harness `src-tauri/src/mushoku_s3_tests.rs` (`NYAA_DB_COPY`,
`MUSHOKU_CRAWL`; it opens the copy through `NyaaClient::with_storage`, rebuilds the
`ShowMatcher` the way `show_matcher` does and mirrors `groupedSources`):
- *Nothing was missing.* All 24 ids are in the 1000 results nyaa.si served
  for "mushoku tensei" (`search_hits`; the oldest is id 1834436, a Season 1
  release, so the 1000-result cap does not reach Season 3), all 24 are kept by
  the title rules (no dataset row: all post-dump) and all land in
  "Season 3 Episode 1" (the E01-E14 batch in "Season 3 Batch", spread over
  the episodes). Category 1_3/1_4 are searched (`Category::AllAnime`).
  Two Season 3 releases the store holds (Breeze E12, E13) are absent from the
  1000 hits though inside the id range: unexplained, noted only.
- *The "Episode N" rows* were 49 kept releases (45 episodes, 4 batches) the title cannot place:
  (a) Season 3's own WEB/TS releases named "Mushoku Tensei III Isekai Ittara
  Honki Dasu - 07" (Diogo4D-NF93, shincaps; `release-parse` does not read a
  numeral after a name, so the label said season 1); (b) Season 1/2 BDs and
  batches named "Mushoku Tensei Isekai Ittara Honki Dasu - 07" (Diogo4D BD,
  `[EA]` 01-11 / 2nd 01-12, ReinForce batch) that match because the entry's
  synonym "... Isekai Ittara Honki Dasu 3rd Season" is season-stripped to
  Season 1's romaji. The bare-franchise "[Erai-raws] Mushoku Tensei - 01" is
  already dropped by the matcher; commit b056c24 (unmarked releases are kept on
  later-season entries) only made (a) and (b) visible.
- *Fix.* `filter_for_show` puts `ShowMatcher::roman_season_of` into the label
  of title-decided releases (a, now "Season 3 Episode N", ranges spread), and
  sets `NyaaResult.season_ambiguous` (`ShowMatcher::season_ambiguous`: later-season
  entry, no season marker or numeral, every entry name found in the title is also a
  name of a TV prequel; not set when the dataset decided). `groupedSources` drops an
  ambiguous unmarked episode/batch unless an absolute episode number resolves it,
  the AniDB id is trusted, or the scraped file list names the entry's season (the
  batch path for titles with no episode is unchanged). Page after the fix (harness): no
  "Episode N" / "Batch" rows; 14 of the 49 (S1/S2 BDs and batches) are dropped, the
  rest joined the Season 3 rows (49-65 releases per episode).
- *1000-result cap.* No extra "<base> S03" query: no id was lost to the cap.
  If it starts cutting Season 3 (oldest returned id newer than the season's first
  release), merge `search_local` into the remote answer or add the targeted query.

## AnimeTosho dataset (v0.5.0, was v0.4.4 - minor feature)

**What it is** (2026-10-12, user request: "see what we can do with anime
tosho's dump"). `database-dump.7z` (2.9 GB, 11.7 GB unpacked, taken
2026-05-09, gitignored, never committed) is a MariaDB `mysqldump` of
AnimeTosho's database: extended `INSERT INTO ... VALUES (...),(...)` per
table, three files:
- `toto_repl.sql` (5.5 GB): `toto_toto` (every release: `nyaa_id`,
  `nyaa_subdom`, `name`, `btih`, `totalsize`, `dateline`, **`aid`/`eid`/`fid`**
  = AniDB anime/episode/file ids, `resolveapproved`, `deleted`, `isdupe`,
  `torrentfiles`), `toto_files` (per-torrent file names and sizes),
  `toto_anidb_tvdb` (AniDB -> TVDB id, `defaulttvdbseason`,
  `episodeoffset`, `mapping-list`), plus tracker/attachment tables we skip.
- `toto.sql` (1 GB): queues and caches, and `toto_adb_aniname_map` /
  `toto_adb_aniname_alias` (AniDB title aliases).
- `arcscrape.sql` (5.1 GB): archive scrapes of nyaa.si (`nyaasi_torrents`),
  AniDex, nekoBT and tosho.

**Why it matters.** Title matching (`title_match.rs`) guesses which release
belongs to which show. AnimeTosho already resolved most releases to AniDB
ids, which are ground truth. The dump can replace guessing with an id lookup
for everything up to May 2026, keep title matching for newer releases, and
give per-torrent file lists and season/offset maps offline.

**Decisions** (user, 2026-10-12): build all four uses, delivered as a compact
dataset imported from Settings, like the nyaa.si database import:
1. **Exact release matching.** `release_aid(nyaa_id, aid, eid)`; AniList ->
   AniDB through Fribb's `anidb_id` (already downloaded by `anime_lists.rs`,
   extend its reduced map). A release whose `aid` is known belongs to the
   entry with that `aid` and to no other, whatever its title says; releases
   with no `aid` keep the title rules. `eid` groups releases of the same
   episode, so an episode's number can be taken from the group's parsed
   numbers even for absolute-numbered releases.
2. **Torrent file lists.** `release_files(nyaa_id, idx, path, size)` for
   batches (`torrentfiles` > 1) of anime releases, so season packs resolve
   without a view-page scrape (`TorrentDetails.files` falls back to it).
   Done (T6): `fetch_details` order is stored scrape (`details` table), disk
   cache, dataset `release_files`, network scrape. Dataset-built details are
   returned only (never written to `details`), carry submitter "Anonymous"
   (the dataset has none), and seasons from `build_files`. About 22k of the
   66,917 known-aid batches have no file rows and still scrape.
3. **Season and episode offsets.** `anidb_tvdb` kept whole: `season`,
   `episode_offset`, mapping list. Used by `get_entry_season` and for
   absolute-number conversion; replaces the Fribb season map where present.
   Done (T7): `get_entry_season` takes the first of the entry's AniDB ids with a
   positive numeric `season` (NULL, 0 or negative fall back to the Fribb/title
   season; the log names the source). `get_entry_episode_offset(anilist_id)`
   returns `episode_offset` of the first aid with a row; `App.tsx` stores it per
   entry next to `entrySeasonByMedia`. Converting absolute numbers with the
   offset is a follow-up, no behaviour change yet.
4. **Bulk nyaa history.** `nyaasi_torrents` rows (id, info hash, name, size,
   flags, created) into the existing permanent `releases` table through
   `Store::import`'s rules (a stored row wins unless fresher).

**Dataset file** (`tosho-dataset-<date>.db`, SQLite, built offline by
`cargo run -p nyaa-client --example build_tosho -- <dump.7z> <out.db>`; the
tool streams `7z e -so` through a purpose-written INSERT parser, so the 11.7
GB are never unpacked to disk). Tables: `meta(key,value)`, `release_aid`,
`release_files`, `anidb_tvdb`, `releases` (same schema as `store.rs`).
Target size under 500 MB; if `release_files` alone is too big it is limited
to batches of the last N years (measure first, phase T1).

**In-app download (T0, user request 2026-10-13).** The dump is distributed
as a torrent (`https://animetosho.org/at-data.torrent`), so Settings gets a
"Download AnimeTosho data" action instead of the manual `database-dump.7z`:
1. Fetch the `.torrent` over HTTP (it is metadata only). The torrent is
   **1.01 TB** (`Anime Tosho Data/`: `anidex_archive` 26.6 GB, `attachments`
   145 GB, **`database-dump.7z` 2.74 GB**, `nekobt_archive` 465 MB,
   `nyaasi_archive` 44.6 GB, `nyaasis_archive` 104 GB, `nzb` 24.1 GB,
   `sframes` 666 GB, `sshots` 2.56 GB, `torrents` 18.2 GB), so the app
   downloads **only `database-dump.7z`** (match the file by name/size, never
   rely on index order) and every other file stays untouched; nothing may be
   pre-allocated or fetched for them. sbtl is stream-driven (it fetches only
   pieces a reader asks for), so the download is one sequential reader over
   that file (SEQ_AHEAD) that writes the bytes to
   `<data>/tosho/database-dump.7z` (its own dir, not the streaming cache, so
   idle removal never touches it) and reports progress (percent, speed,
   peers) to Settings through a Tauri event. Spike must confirm: sbtl's
   storage does not pre-allocate the 1 TB layout, a resumed run skips pieces
   already on disk, and a 1 TB piece map is cheap in memory.
2. When complete, run the builder (T1/T2) on the downloaded archive and
   import the result (T3). The builder lives in `nyaa-client`
   (`tosho::build`) so the app and the `build_tosho` example share it;
   decoding streams the archive (a Rust 7z reader such as `sevenz-rust2`,
   falling back to an external `7z` when present) so the 11.7 GB are never
   unpacked. Spike first: confirm the reader handles the dump's 7z method
   and its streaming speed, and list the torrent's files (it may hold more
   than `database-dump.7z`; only the SQL parts we read are required).
3. After a successful import, offer to delete the archive (keep the small
   dataset); resumable: a partial download continues, a finished one skips
   straight to the build. Cancel stops the torrent and keeps partial data.
Rules: explicit user click only (2.9 GB), shows size before starting, disk
space check (archive + dataset), no automatic updates. The dump is a snapshot;
re-downloading replaces the dataset (`meta.dump_date`).

**Measured (T1, 2026-10-13, `build_tosho --stats`).** `toto_repl.sql` streams
in 67 s (release build, `7z e -so`). `toto_toto` 763,361 rows: 609,805 on
nyaa.si (+491 sukebei), 682,819 with an `aid` (561,350 of them on nyaa.si),
20,910 deleted, 99,712 dupes; `resolveapproved` among rows with an `aid`:
0 = 381,709 (56%), 1 = 250,901, 2 = 101, 3 = 50,108; 66,917 nyaa.si batches
with a known `aid` and more than one file. `toto_files` 1,376,362 rows,
about 99 MB of names and sizes for ALL torrents, so no size cut: keep every
file list of an anime (`aid` > 0) release. `toto_anidb_tvdb` 8,597 rows.
Open question for T2: 56% of resolved rows are `resolveapproved = 0`
(AnimeTosho's unconfirmed guesses); decide in T8 by checking them against
`franchises.json` whether they are trusted or only a hint.

**Built (T2, 2026-10-13, `build_tosho <dump.7z> <out>` = both SQL files, 109 s,
release build).** `tosho-dataset-20260509.db` is 500 MB: `releases` 986,012
(593,161 from `toto_toto` - nyaa.si, not deleted, with a 20-byte btih - plus
392,851 anime rows only `arcscrape.sql` has; `--no-archive` skips that pass),
`release_aid` 531,419 (nyaa.si, `aid` > 0, not deleted, not a dupe),
`release_files` 596,166 rows for 44,621 multi-file releases, `anidb_tvdb`
8,597 (1,250 with no numeric season). Releases store `words` empty
(`Store::import` recomputes it) and `updated_at` = the dump date, so rows the
app scraped itself win. AnimeTosho's `nyaa_cat` mixes nyaa.si codes with
nyaa.se's legacy ones; the builder maps `1_37`/empty -> `1_2`, `1_11` ->
`1_3`, `1_38` -> `1_4`, `1_32` -> `1_1` so the app's category searches find
them.
Importing it into the app store (T3, `Store::import_tosho`) took 40 s for the
500 MB file and grows the store to about 570 MB.

**"Import Tosho Data" button (T0 refined, user 2026-10-13).** Most users have
no `tosho-dataset.db`, so Settings > AnimeTosho dataset gets a primary
**Import Tosho Data** button that does the whole pipeline in one click; the
existing ".db" picker stays as "Import from file" (advanced). Phases, each
with its own progress line and cancel:
1. *Confirm:* dialog with the download size (2.74 GB), the dataset size
   (~500 MB, ~1.2 GB of disk during the build), and the free-space check.
2. *Download:* fetch `at-data.torrent`, download only `database-dump.7z`
   through sbtl-engine (see above) into `<data>/tosho/`; resumable.
3. *Build:* compile the dataset (`tosho_build::build`) straight from the
   archive stream, no external tools: users do not have `7z` installed, so
   the archive is read by a pure-Rust 7z decoder (spike D2: `sevenz-rust2`
   or equivalent; must stream LZMA2 entries `data\toto_repl.sql` and
   `data\arcscrape.sql` without unpacking to disk, at a speed comparable to
   `7z e -so`, ~2 min total; if no crate qualifies, bundle `7zr.exe`
   (LGPL, ~600 KB) from `src-tauri/lib/` and spawn it). The build writes
   `<data>/tosho/tosho-dataset-<date>.db`.
   **D2 result (2026-10-13): pure-Rust `sevenz-rust2` 0.24 qualifies; no
   `7zr.exe` needed.** `nyaa_client::sevenz_stream::open_entry(archive,
   entry)` returns a `BufRead` fed by a decoder thread (1 MiB chunks, 4
   queued) and `entry_date` replaces `7z l`. The dump is 4 LZMA2 blocks
   (`arcscrape`, `toto`, `toto_repl`, schema+timebase), so only the block of
   the wanted entry is decoded and reaching `arcscrape.sql` costs nothing
   extra. Measured (release, same machine, installed 7-Zip): `--stats toto_repl`
   78 s vs 64 s with `7z e -so` (1.2x), peak 102 MB; full build 148 s vs
   125 s (toto_repl at 81 s, +50 s arcscrape vs +34 s), peak 406 MB for both
   (the builder); identical row counts (toto_toto 763,361, toto_files
   1,376,362, toto_anidb_tvdb 8,597) and identical dataset tables. Decoding
   must stay single-threaded: sevenz-rust2's multi-threaded LZMA2 reader
   queues compressed input without a bound and ran out of memory (4.5 GB
   working set, then OOM); `SEVENZ_THREADS=n` exists only for experiments.
   Licences: sevenz-rust2 and lzma-rust2 are Apache-2.0, crc32fast
   MIT/Apache; no C code or system deps (`default-features = false` drops
   AES/bzip2/ppmd/writer), so 32-bit Windows builds like any pure-Rust
   crate (not compiled here: no i686 target installed). `build_tosho`
   uses it by default; `--external-7z` keeps the `7z` path.
4. *Import:* `Store::import_tosho` as today.
5. *Clean up:* offer to delete the 2.74 GB archive (default yes), keep the
   dataset db for a re-import. A failed phase keeps the previous phase's
   output so a retry resumes (downloaded archive, built db).
Progress events: `tosho-import-progress {phase, done, total, speed, message}`
shown inline in the Settings row; the button becomes Cancel while running.
Never starts without the confirm dialog; offline or tracker failure shows a
clear error and the manual ".db" path still works.

**Stem mapper (T9, decided 2026-10-13: static, trained only on the dump;
newly collected data never feeds back).** Releases newer than the dump have
no `aid`. The mapper generalises the dump's ground truth to them:
- *Training (offline, in `build_tosho`):* for every dataset release with a
  trusted `aid` (`approved` 1-3, plus 0 only when consistent with its
  group), parse the title (`release-parse`), normalise the show name (the
  same `normalize` as `title_match.rs`, season markers and episode numbers
  removed) into a **stem**, and count `stem -> aid` votes. Keep a stem only
  when one `aid` holds at least 90% of its votes and at least 3 releases (or
  2 groups); conflicting stems are dropped, not guessed. Table
  `stem_aid(stem TEXT PRIMARY KEY, aid INTEGER, votes INTEGER)` in the
  dataset, imported with it (wholesale replace like the other tables).
- *Use:* a release with no dataset `aid` looks up its stem; a hit gives an
  `aid` with `approved = 0` weight (a hint under the T5 rule), a miss uses the
  title rules. Nothing is written back: the table changes only when a newer
  dump is built and imported, so there is no feedback loop.
- *Evaluation (holdout):* train on releases before 2025-01-01, test on the
  releases after it (they all have true `aid`s): report coverage (share of
  test releases the stems resolve) and precision (share correct); ship only
  if precision >= 99% on resolved releases, otherwise raise the vote
  thresholds. Series that debuted after the dump have no stems and fall to
  title rules by design.

**Stem mapper result (T9, measured 2026-10-13 on the real dump).** Built:
`nyaa-client/src/stem.rs` (`parse_stem`/`stem_of` = `release-parse` show name
through a copy of `normalize`, `train`, `evaluate`, `holdout`),
`ToshoBuilder::finish` writes `stem_aid` (10,284 stems from 531,419 aid rows;
dataset still 500 MB, build 118 s), `Store::import_tosho` replaces `stem_aid`
(older datasets without the table import and clear it), `NyaaClient::
aid_for_stem` / `aids_for_stems`. `build_tosho ... --holdout 2025-01-01` (and
`--holdout-only <dataset.db> <date>` on a built dataset) prints the evaluation;
`--stem-share/--stem-releases/--stem-groups` set the thresholds (default 0.90 /
3 releases / 2 groups). A guess (`approved` 0) votes only when every release
of its group for that stem agrees on one `aid`.
- Holdout (train < 2025-01-01: 449,946 releases, 9,040 stems; test = 47,366
  trusted releases from 2025-01-01): coverage 18.99%, **precision 38.3%**
  (8,994 resolved, 3,446 correct). Of the 5,548 wrong, 5,395 name another
  AniDB entry of the *same TVDB show* (a returning series' new season/cour has
  a new `aid`, the stem still maps to the old one); only 153 are another show.
  Counting a same-show answer as right gives 98.3%, still under 99%.
- Raising the thresholds makes it worse, not better: share 0.99 / 20 releases
  / 4 groups gives 16.9% coverage, 36.3% precision; share 1.0 / 50 / 6 gives
  12.7%, 22.3%. Strict thresholds favour long-running stems, which are exactly
  the ones that gain new seasons. Cutoff 2024-07-01: 17.7%, 32.4%.
- **Verdict: the hook is NOT enabled.** The 99% gate cannot be met by vote
  thresholds because the error is structural (a stem cannot name a season).
  `filter_for_show` / `ShowMatcher::decide` are unchanged, nothing reads
  `stem_aid` yet, and the table ships unused. Possible later fixes, each
  needing its own plan entry: stems only for shows whose TVDB group has a
  single AniDB entry, or season-aware stems (stem + season marker).

**Matching rule (T5).** `ShowMatcher::decide(title, known, entry_aids)` ->
`Accept | Reject | UseTitleRules`; `entry_aids` = `AnimeLists::anidb_ids` of
the entry's AniList id, `known` = `NyaaClient::aids_of` for the result ids
(one batch query per search; commands `search_torrents_for_anime`,
`search_local_releases`, `search_fansubber_releases` via `filter_for_show`).
- No `known` row, `aid` = 0, or the entry has no AniDB mapping: title rules,
  unchanged (post-dump releases and unmapped entries do not regress).
- `approved` 1/2/3 (trusted): `aid` in the entry's set -> Accept, else Reject,
  whatever the title says (season markers, sibling rules, missing names).
- `approved` 0 (AnimeTosho's guess) is only a hint, never overriding both
  ways: `aid` in the set -> Accept unless the title explicitly names only
  other seasons than the entry's (then the title rules decide); `aid` not in
  the set -> title rules decide (a guess never rejects by itself).
- `NyaaResult.episode_group` = the `eid` (when non-zero) of kept releases the
  dataset knows. `src/App.tsx` `groupedSources`: a release that parsed no
  episode takes the number its same-eid siblings agree on
  (`episodeNumbersByGroup`, groups with disagreeing numbers are ignored), as an
  episode of the entry's season; releases that parsed an episode are untouched.
- `NyaaResult.aid_trusted` = `true` when `decide` returned Accept on a trusted
  (`approved` 1-3) aid. `groupedSources` files such a release under the entry's
  season when its title names only other seasons (season 0 excepted), instead
  of letting the frontend season filters drop it (T8, below).
- `title_match_fixture_tests.rs` has no release ids and keeps measuring the
  title rules only.

**Verification (T8, 2026-10-13, real dump 2026-05-09, Fribb map fetched the same
day).** Harness: `src-tauri/src/tosho_verify_tests.rs`, `#[ignore]`, run with
`TOSHO_DATASET=<db> TOSHO_SCRATCH=<dir> [SYMPHO_CRAWL=<json>] cargo test -p
nyaa-stream --lib tosho_verify -- --ignored --nocapture` (imports the dataset
through `NyaaClient::import_tosho`, so the app's own lookups are measured; needs
network for Fribb and AniList; datasets are not committed). `franchises.json`
has no AniDB ids: the ground truth for seasons is its hand-labelled entry
season, the AniDB ids come from Fribb.
1. *Season truth* (23 TV entries: Food Wars S1-S5 incl. the Totsuki Train
   entry, Mushoku S1-S3 incl. Cour 2 / Season 2 Part 2, Slime S1-S4,
   Symphogear S1-S5 incl. XV). `decide_entry_season` = expected for **23/23**.
   The dataset decided 19 (every one right, 0 disagreements with Fribb); 4 fell
   back (Mushoku S3 `178789`: its `anidb_tvdb` row has no numeric season;
   `217434` and Slime `217331`: no Fribb AniDB mapping at all), all right
   through the title logic. Fribb alone is right on 21/23 (absent for those
   two). No case of the dataset overriding a right Fribb/title season; the
   1,250 dataset rows without a numeric season keep the fallback.
2. *Release attribution* (dataset releases whose trusted aid (`approved` 1-3)
   is in the franchise's aid set or whose title has the franchise words:
   3,116 trusted, 3,449 hint, 5,818 without an aid; 15,927 entry x release
   pairs). Trusted aid is the truth. Title rules alone: recall 85.2% (1,716 of
   2,015), precision 36.4% (3,004 wrongly kept, 299 wrongly dropped - the
   absolute-numbered / unmarked releases the title rules keep on every season
   on purpose, split cours, "Shokugeki no Soma S3" for Totsuki Train, HorribleSubs
   "Shokugeki no Soma - 01"). `decide` + title fallback: 100% / 100% by
   construction (an Accept/Reject on the trusted aid), so it cannot regress on
   trusted releases unless the aid itself is wrong; per entry the gain is in
   the output of the harness. Hint rows (`approved` 0, aid in the entry's set,
   2,602 pairs): 199 (7.6%) accepted beyond the title rules (HorribleSubs "Shokugeki
   no Soma S2 - 04", OVAs and specials of the same aid), 32 left to the title
   rules because they name only other seasons.
   *Independent check against the hand labels* (the fixture titles that the
   dataset knows by exact title: 226 of 1,275, 103 trusted): title rules alone
   recall 97.4% / precision 94.0%, `decide` + title 96.1% / 96.4%; on trusted
   rows only 100% / 93.6% -> 96.2% / 99.2%. The aid changed 16 decisions: 10
   fixed, 6 "regressions", and all 6 are on the split cour: Erai-raws "Mushoku
   Tensei - 01 ~ 11" (cour 1, aid 14758) is now rejected on the Cour 2 page
   (the hand label groups both cours as season 1; this is the "split cours" gap
   under "Not covered" above, now closed for dataset releases), and Erai-raws
   "... 2nd Season - 01 ~ 12" (2021-12-21, trusted aid 15954 = Cour 2) is
   hand-labelled season 2 in the fixture, which is wrong (it aired as cour 2 in
   Oct-Dec 2021), so the aid is right and the label is not. Not a code
   regression; the fixture is left as it is (it measures the title rules).
3. *Frontend filters.* Emulating `groupedSources`' season filters on the 2,015
   trusted accepts: 40 (2.0%) would be dropped, all of them Erai-raws "2nd
   Season" releases of the Mushoku Cour 2 entry (season 1). Fixed:
   `NyaaResult.aid_trusted` (set by `filter_for_show`, `src/types.ts`) and
   `groupedSources` snaps such a release to the entry's season before the
   filters.
4. *Symphogear XV crawl* (`download (1).json`, 75 results, 48 known to the
   dataset): XV entry (`21673`, aid 11959) keeps 72 of 75, the same 72 the title
   rules alone keep: episodes 1-13 all present, 10 batches (VCB S5, Kirika S05,
   PMR S05, Punisher S05, Erai 01 ~ 13 x2, the `S01-05 + Specials` pack, Commie
   unofficial batch, BDMV vol.1-6). The 3 dropped are the "Symphogear + G + GX +
   AXZ + XV" packs with no season marker, which dataset aid 8709 (season 1, hint)
   does not vouch for; the title rules drop them too (a pre-existing gap, not
   a regression). Six non-video items (Bonus CD, ED single) pass on the title rules
   as before. The frontend keeps them all (unmarked titles are not season
   filtered).
5. *`search_local(.., Category::AllAnime)`.* Confirmed: `1_0` was compared by
   equality, so it matched nothing. Now `1_0` means `1_1`..`1_4` (test
   `all_anime_category_covers_every_anime_subcategory`; manga `3_1` stays out).
   Post-dump releases are untouched by any of this: without a `known` row
   `decide` returns `UseTitleRules`.

**Freshness.** The dump ends 2026-05-09. Newer releases have no `aid`, so
title matching stays the fallback and must not regress. The live AnimeTosho
feed (`feed.animetosho.org/json?aid=`) cannot fill the gap (checked 2026-10-15):
AnimeTosho stopped ingesting torrents on 2026-05-09 (animetosho.org/about/shutdown2),
the feed has nothing newer than the dump, and its feed/storage server expires in
October 2026. The gap is permanent, which is why the learned matcher exists.

**Risks.** AniDB splits entries differently from AniList (one AniList entry
can span several AniDB entries, e.g. split cours): AniList -> AniDB is one to
many, so an entry's `aid` is a set. Fribb covers ~80% of entries; the rest
use title rules. `resolveapproved` = 0 rows are guesses by AnimeTosho's own
matcher: only 1, 2, 3 are trusted. The dump is third-party data; the dataset
is a local derived file, never bundled in the installer.

## Learned release-to-entry matcher (planned, v0.6.1 - minor feature; user 2026-10-15)

**Request** (2026-10-15): map releases newer than the AnimeTosho dump to their
anime entry without an `aid`, using rules learned from the dump's ground truth.
Replaces the stem route (T9: 38.3% precision, structural - a stem cannot name a
season; see "Stem mapper result"). Slots after the v0.6.0 magnet milestone.

**IPO.**
- *Input:* an anime entry (AniList/AniDB/Fribb data: titles + synonyms, season,
  format, episode count, air window, `anidb_tvdb` season/offset/mapping, entry
  aid set) and unmapped nyaa releases (title, publish date, size, file list,
  group).
- *Process:* a small model trained offline on the dump scores each (entry,
  release) pair as "belongs to this entry". The entry is an input, so an entry
  the dump never saw (a new season) is scored like any other. Below a
  confidence threshold it abstains and `ShowMatcher::decide` title rules
  decide, as today.
- *Output:* per entry, its releases with a score, an episode number and a
  pack/episode label; feeds `filter_for_show` as one more signal after trusted
  `aid`s (a trusted `aid` still wins).

**Training data (trusted rows only, user 2026-10-15).** Positives: dataset
releases with `approved` 1-3, `aid` -> AniList entry through Fribb. Negatives:
the same release against (a) sibling entries of the same TVDB show (other
seasons/cours - the case that sank the stems) and (b) other shows, both built
from trusted rows only; an `approved` 0 guess is never a positive or a negative.
~224k trusted rows. The pre-2017 / big-group skew of trusted rows is accepted
for training (user), but evaluation is sliced by year and group size.

**Features** (all relative, never identity): best title/synonym similarity
(normalised string, optionally an embedding score for CJK/Russian names);
season marker equals the entry's season; publish date inside the air window;
episode number inside range after `anidb_tvdb` offset; batch vs single;
file-count vs episode count; group's agreement history. Not features: `aid`,
stem, show id (the model must not memorise ids).

**Baseline and gold set (measured 2026-10-15).** Mushoku Tensei crawl (1,000
nyaa titles, S3 entry 178789): the app's rules keep all 780 true S3 releases and
read every episode number (750/750), but 36 S3 releases are labelled season 1
(roman `III` / unmarked: Diogo4D, shincaps) and 17 non-S3 releases are kept
(unmarked S1/S2 Blu-ray rips, `2nd 01-12` batches, a 16.5 special). Across
217,738 trusted Tosho releases with a plain TVDB season: 69.1% unmarked on S1
entries (fine), 19.1% marked and right, **10.1% unmarked on S2+ entries (the
title carries no season; only entry context can decide)**, 0.8% marked wrong
(part is TVDB-vs-title noise), 0.6% unparsed. Harnesses: `mushoku_crawl_titles`
and `tosho_label_coverage` (ignored tests). The reviewed Mushoku crawl (the
user's per-row check) is the first gold set; the mapper is scored against it
and against the trusted rows. Extra features from that analysis: **peer
consistency** (a release group's naming template confirmed by its marked
releases propagates to its unmarked ones) and whole-set sanity (episode counts
per season).

**File-name coverage (L1d, measured 2026-10-15, harness
`tosho_file_label_coverage`).** `EpisodeLabel::of_path` over the 44,621 Tosho
torrents with a file list (569,328 video files): 79% of torrents fully read,
14% partly, 7% not at all; 50,900 files unread. Families of unread files:
`S01E06-Title` with no space after the number (9,556 files, 687 torrents),
number-first names `12 - Title` / `12. Title` (8,961) and `12 Title` (1,358),
bare `21.mkv` (3,974), `Ep-12` / `Ep12` prefixes (1,714), dotted scene names
`Show.S02E10.mkv` (963), trailing `Show -04.mkv` (708), `12.5` (267), CJK
counters (144); ~14k are extras (correctly unread). Duplicates: 4,618
torrents have two files on one (season, episode); the largest cause is extras
read as episodes (`NCOP03`, `NCED 4`, `OP01`, `Menu 2`, `SP10`, `Mini Ep11`
-> episode N), then franchise packs whose folders are different shows
(Oregairu / Zoku, Haruhi 2006 / 2009) and dub/sub or TV/BD duplicates.
Fixes, in release-parse (file names only where noted) and checked against
`corpus.txt` for title regressions. **Done (L1d, 2026-10-15):** the rules live
in `nyaa-client/src/file_name.rs` and apply to file paths only (`of_path`,
`build_files`), so release-title parsing (`EpisodeLabel::of`) is unchanged and
the corpus needs no diff. Re-measured: torrents fully read 34,421 -> 40,068
(79% -> 92%), partly 6,141 -> 2,281, none 2,983 -> 1,196; unread video files
50,900 -> 10,655 (0 newly unread); torrents with a duplicate (season, episode)
4,618 -> 1,828. ~29k files now read as episodes (a 40-file random sample: all
numbers right), ~11k extras now `Special`. False positives fixed during the
pass: CRC tags (`[5B1F11ED]` read as "ED"), `[OP_HNK]` group tags, titles
with the word ("Makes Me OP Even", "SP Professional"). The remaining
duplicates are mostly franchise packs whose folders are different shows
(Seitokai Yakuindomo / Bleep, Gundam Zeta / ZZ, Oregairu / Zoku): no file-name
rule fixes those; they need the per-file show name matched to an entry. Rules
planned: (1) creditless/menu/preview/SP tokens
with a number are extras, never episodes; (2) `S##E##-Title`; (3) a leading
1-4 digit number in a *file name* is the episode (`12 - Title`, `12. Title`,
`21.mkv`, `001 Title`); (4) `Ep`/`EP`/`Episode` + number; (5) batch
self-verification: a batch whose files read as a gap-free run matching its
title range is accepted without review, anything else is flagged.

**Batch self-check (L1e, done 2026-10-15).** `nyaa_client::check_batch(title,
file_paths)`: a batch title's episode range against its files'
`EpisodeLabel::of_path` numbers (extras ignored); verified when the files are
exactly that run, each once, none unread; a later-season pack numbered from 01
verifies with an offset (`Naruto Shippuden 219-231`, files 01-13). Harness
`tosho_batch_check` over the 44,620 Tosho releases with file lists: 9,754 have
a title range; **8,216 verify automatically (84%)** (19 of them with an
offset); 1,538 are flagged (Missing 1,033, Unread 457, OutOfRange 278,
Duplicates 241; several per release possible). Flags checked by hand are real
(`Kuroko no Basket 1-25` holds both `- 01` and `- 1`; `Psycho Pass 1-3` two
releases of ep 2). Known title-side false flags: episode lists read as one
range (`01,02,05,07-13`, `41+48+50-52`), `x365` read as episode 365
(Hidamari Sketch x365), `20.5-21` read as 5-21. Verified batches need no
manual review; flagged ones are the review/training queue.

**Torrent-file mapping follow-up (done, v0.5.6 patch; measured 2026-10-26).**
On the imported 2026-05-09 database, `tosho_batch_check` now verifies 8,249 of
9,754 title-range packs (up 33 from 8,216); 1,505 remain flagged (down 33).
The gain is 36 formerly flagged packs verified, with 3 previously verified
packs now flagged after their sparse title lists were read correctly. Split by
`release_aid.approved`, trusted approval 1-3 batches verify 165/195 (was
164/195), while approval-0 rows verify 8,084/9,559. Approval-0 is an audit
population, not trusted source attribution ground truth. Batch checking now
honors explicit sets such as `01,02,05,07-13` and `41+48+50-52`, and reports
only explicitly named episodes as missing. A title with an extra listed range
is now flagged when its files omit that range.

`tosho_file_label_coverage` now maps 40,069/44,621 torrents fully (was
40,068), 2,280 partly (was 2,281), and 1,196 not at all; unknown video files
fell from 10,655 to 10,654. Duplicate (season, episode) labels fell from 1,828
to 1,823 torrents. File-name mapping recognizes numbered `Extra 01` names,
while tests protect show/episode titles containing `Alternative` or `Extra`.
The measured file-label gain is small; sparse-list verification is the larger
batch-check improvement. Approval-0 mappings must remain separated in future
evaluation.

**Peer consistency (L1c, measured 2026-10-15; NOT enabled).**
`nyaa_client::peer_seasons` gives an unmarked release the season that *other*
groups' marked releases of the same show stem use in the same airing window
(>= 3 marked releases from >= 2 groups, window +-14 days, episode number at most
3 past the window's highest marked episode). A first, within-one-group version
barely fired (158 of 172k unmarked; a group's template is consistent:
SubsPlease always marks, HorribleSubs never does). Cross-group, on the 217,738
trusted Tosho rows (`tosho_peer_season`): 20,609 propagations, but 19,484 of
them only confirm season 1, which unmarked titles default to anyway. The cases
that matter, propagations to season 2+: **1,071, 93.2% precision**, fixing
998 of the 21,966 unmarked-on-S2+ releases (4.5% recall); errors are season-1
tails overlapping a season-2 run (Diogo4D `Magi 18-25`, `Aldnoah Zero - 14`).
On the Mushoku crawl it changed one release, wrongly (`[ReinForce] Mushoku
Tensei ... batch`, an S1 Blu-ray, -> season 2), and none of the 36 `III`
releases. Fails the 99% gate and the recall is too low to matter: the code and
harnesses stay for the model (as a feature, not a rule), nothing calls it.

**Live RSS check (L1f, 2026-10-15, `nyaa.si/?page=rss&c=1_0`, 75 items).**
Every item but one has a real AniList entry (`我是不白吃 第1092话`, a Chinese
web short, has none). A naive release -> AniList mapper (release-parse title +
alternate titles in `(..|..)` / `CJK / Romaji`, AniList search, normalised
exact name match, season entry by name) got 45 fully right, 17 half right, 2
wrong, 11 unmapped. The failure classes, each a feature the mapper needs:
spelling variants (`KURUMICHAN`, `Dekite Iru` vs `Dekiteiru`, `Linck Click`,
AniList's own Cyrillic `о` in `Juuоu Mujin Dandivine`); a year in the title
picking a remake (`Magic Knight Rayearth (2026)` went to the 1994 entry);
absolute numbering into a later season (`Kikansha no Mahou ... - 14` = 2nd
Season ep 2); packs spanning two entries (`Sonic X 01-78` = Sonic X 1-52 +
Sonic X Season 2); groups disagreeing on the season number for the same
episode (`Link Click S04E11` vs `第三季 第11话`); movie subtitles
(`~Soukai no Namida-hen~`). 30 of the 75 were one uploader's Sonic X dump, so
the sample is skewed; repeat on several days before using it as a rate.

**Model (decided 2026-10-15, user): XGBoost, trained in Python, scored in
Rust.** Binary classifier (`binary:logistic`) over (entry, release) pair
features; offline Python + `xgboost` trains on the Tosho dataset (scratch
`ml/` scripts, never shipped, no Python at runtime). The booster is exported
with `Booster.save_model("model.json")` (XGBoost's JSON tree dump) and a small
Rust evaluator in `crates/nyaa-client` walks the trees (sum leaf values, add
the base score, sigmoid), so the app carries only the model file (tens to
hundreds of KB), no native ML dependency. A parity test pins the Rust scores to
the Python `predict` output on a fixture of pairs (tolerance 1e-6). Missing
features are NaN and follow each tree's default direction, same as XGBoost.
Probability is calibrated on the 2025+ split before the abstain threshold is
chosen. No shipped LLM: size, latency, and online dependency; an LLM is at most
an offline auditor.

**Pipeline** (what the user asked for: new anime data + nyaa sources -> model
trained on old trusted matches -> new anime data with the correct sources):
1. *Training (offline):* trusted dataset rows -> pairs + features -> XGBoost ->
   `model.json` (+ threshold, feature order, version) written next to the
   dataset and imported with it.
2. *Inference (in app):* for an anime entry (new season included), candidate
   releases come from the usual nyaa search / local release database; each
   pair is featurised and scored; releases above the threshold are the entry's
   sources, below it the title rules decide as today. A trusted `aid` always
   wins.
3. *Retraining:* when a newer dump is imported, or when the user's reviewed
   rows (gold sets) are added; the model is replaced wholesale, never patched.

**Evaluation and gate.** Time split: train on releases before 2025-01-01, test
on 2025-01-01 onward; also a held-out set of entries (whole shows unseen in
training) to prove new-entry generalisation. Report precision/coverage overall
and by year, group size, and sibling-vs-other-show negatives. Ship only if
precision >= 99% on answered pairs; abstain below the threshold.

**L1 prototype result (2026-10-16, `ml/`: `export_rows.py`, `fetch_anilist.py`,
`train.py`, parser via `release-parse` example `ml_parse`).** Trained on the
153,030 trusted (`approved` 1-3) releases published 2021 -> 2026-05-09 (151,869
with an AniList entry through Fribb; 5,701 entries, AniList titles/format/
episodes/air dates fetched live). Pairs = release x candidate entries (top 8 by
char-n-gram title similarity plus the TVDB siblings of each); 28 relative
features; XGBoost `binary:logistic`, train < 2024-09, validate to 2025-01,
test 2025-01-01+ (47,144 releases); 10% of entries (544) never seen in training.
Test top-1: **99.1%** (title similarity alone 76.1%); at p>=0.95 coverage 95.8%
precision 99.93%; held-out entries 99.6% top-1, 99.9% precision at 96.5%
coverage. Top feature by gain: release date inside the entry's air window, then
title similarity rank/gap, then season match. **Not yet proof of the 99% gate**,
because the test is easier than production: (1) every test release's true entry
is in the candidate pool, so "none of these" (non-anime, entry without trusted
rows, no Fribb mapping) was never tested or trained; (2) the pool holds only
entries that have trusted releases; (3) no slice yet by release kind (late
BD/batch releases outside the air window), group size or sibling-vs-other-show
negatives; (4) labels are Tosho trusted rows, which skew toward groups it
resolves well. Next: add no-match negatives and the Mushoku S3 gold set (L1b),
then the slices.

**L1 hardening run (2026-10-16, same split; pool widened to all 9,028 dataset
entries incl. ones with no trusted rows; 8% "true entry removed" groups added to
training).** At p>=0.95, margin>=0.3: present-entry coverage 88.6%, precision
99.97%; with the true entry removed from the pool the model wrongly answered
0.06% (4,686 releases); on 87 non-anime releases (category not 1_x, since 2021)
it wrongly answered **17%** (small sample, weak); mixed precision 99.92%.
Held-out entries 99.6% top-1. Probabilities never reach 0.98 (no coverage at
0.98/0.99), so the 0.95 threshold is the top usable one. Slices: precision stays
>= 99.8% everywhere, but coverage is the cost: episodes 89.9%, batches 24%,
movies 5%, specials 1%, unknown kind 17%, groups with <50 training releases 54%
and releases outside the entry's air window 0.4% (top-1 there only 68%): the
model leans on "release date inside the air window" (top gain feature) and
abstains on late BD/batch releases, where title rules still decide. Jujutsu
Kaisen (1,602 releases with no `aid`): 1,223 answered (S1 721, S2 313, S3 190),
379 abstained (batches, `Vol.` BD sets, 2160p S01 packs); against Tosho's guess
rows it agrees on 790 of 803 answered, and the 13 it changes are S2 titles the
guess had put on S1 (guess accuracy ~95.7%, see the Haiku sample). Mushoku
Tensei crawl (1,000): only 123 answered, all S2 Part 2 (the S2 ones); the S3
releases score ~0.003 and abstain **because the S3 entry is not in the pool**
(the pool is built from the dump, which ends 2026-05-09; S3 has no dataset rows).
That is the production case: the pool must come from AniList/Fribb, not from the
dump. Next (L1b): pool = all Fribb anime entries with AniList metadata, then
re-run the Mushoku crawl against the S3 entry; the crawl has no stored per-row
verdicts, so it is compared with the app's rules, not graded.

**Risks.** Fribb covers ~80% of entries (labels exist only for those); trusted
rows skew old; new-season entries are exactly the low-signal case, so the entry
hold-out is the real test; the model must never reject a release a trusted
`aid` accepts.

## Search flow debugger (developer tool, local only)

**ML trial (unversioned debugger work).** The debugger can score its raw nyaa
results with the locally trained XGBoost prototype and show the predicted
entry, confidence, margin, and abstention beside the existing rule decision.
The trial uses the training artifacts under the user's app-data Tosho folder;
normal source filtering and playback continue to use trusted AniDB ids and
title rules. This keeps ML errors inspectable before any production gate.
The 2026-10-28 local retrain on the 2026-05-09 dump measured 99.90% mixed
precision at p>=0.95 and margin>=0.3 on its test mixture, but confidently
assigned 26.4% of 87 non-anime releases. The debugger labels the scores
experimental; this model is not suitable for production filtering.
Live check on 2026-10-28: 321 nyaa results for `Mushoku Tensei III` produced
265 confident S3 assignments and 56 abstentions with the new S3 entry present
in the AniList relation pool (it is absent from the older Tosho dump).

A dedicated `#/debug` web route observes the real anime selection, backend
source matching, and frontend episode mapping path. The page removes the
generic procedure/log timeline and static process explainer. Instead it shows
the selected anime's actual aliases and season context, query candidate
coverage, raw/duplicate/matched/rejected counts, per-release matching
criteria and reasons, and the final episode or pack group for every kept
source. Unmatched releases stay visible with the exact mapping criterion
they failed or the missing information that left them unresolved. Backend
diagnostics must come from the same search/matcher decisions used by normal
playback, and frontend mapping evidence must be produced by the same grouping
logic used by the media page. This is a local development tool and makes no
production UI changes.

