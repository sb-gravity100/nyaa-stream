# TITLE_ANALYSIS.md — how nyaa.si release titles are written

Ground work for PLAN.md "Season-aware source matching" (v0.4.2). Done
2026-10-04 so the matcher and `episodeParser.ts` cover every title format
in use, not just the ones met so far.

## Data and method

- **Corpus**: 8,038 unique release titles. They come from 42 franchises
  picked for awkward naming, crawled from nyaa.si (Anime - English-translated):
  newest 2 pages + largest-size page per query (the largest page is where
  BD and season batches sit). Plus the hand-labelled
  `src-tauri/tests/fixtures/nyaa_titles/franchises.json` (1,275 titles:
  Mushoku Tensei, Slime, Food Wars).
- **Franchises**: Attack on Titan, JJK, Spy x Family, Re:Zero, Oshi no Ko,
  Bleach, Demon Slayer (two queries), MHA, One Piece, Frieren, Dr. Stone,
  Kaguya-sama, Mob Psycho 100, Konosuba, Overlord, Gintama, DanMachi, SAO,
  Haikyuu, Tokyo Ghoul, Vinland Saga, Made in Abyss, Bocchi, Monogatari,
  Fate/Zero, Dungeon Meshi, Solo Leveling, Chainsaw Man, One Punch Man,
  Classroom of the Elite, Black Clover, Naruto, Hunter x Hunter,
  Steins;Gate, Code Geass, Kingdom, Eminence in Shadow, Mushoku Tensei,
  Slime, Food Wars.
- **AniList**: up to 30 search results per franchise (all entries: TV,
  movies, OVAs, specials).
- **Tools**: the current `episodeParser.ts` (bundled with esbuild, run in
  node) and a Python port of `title_match.rs`, run offline over the corpus.
  Scripts were scratch only; the labelled fixture is the kept artefact.

## 1. Episode / range / batch formats

| Pattern | Titles | Example | Today | Needed |
|---|---|---|---|---|
| `SxxEyy` | 2,430 | `Attack on Titan - S01E02` | ok | - |
| ` - 05`, ` - 05v2` | 2,084 | `[SubsPlease] One Piece - 1180` | ok | - |
| Absolute 3-4 digit | 240 | `Boku no Hero Academia - 171` | ok (season 1) | absolute offset already maps it |
| `SxxEaa-bb`, `SxxEaa-Ebb`, `SxxEaa~Ebb` | 45 | `S04E29-31v2`, `S01E01-E12`, `S04E29~E31` | **single episode** | range batch |
| `Sx - ep` | 362 | `[Raze] Jujutsu Kaisen S3 - 12`, `S05 - 02 Water Hashira` | 48 **batch** | episode (season x) |
| `Season N - ep` | (in Season N) | `Spy x Family Season 3 - 13` | ok | - |
| CJK episode `第19话/話/集` | 45 | `[Doomdos] Re:ZERO … Season 4 - 第19话` | **batch / unknown** | episode |
| `E05`, `EP05`, `Episode 5` | 201 | `[Baws] … S04E30 v3` | ok | - |
| `#23` | 1 | `Fate/zero #23 - Nicovideo` | unknown | episode |
| `.5` episodes | 2 | `Vinland Saga S2 - 18.5` | 1 batch | special; keep out of the number grid |
| Dash range `01-12`, `(01-24)` | 1,233 | `[SubsPlease] Mushoku Tensei (01-23)` | mostly ok | - |
| Tilde range `01 ~ 25`, `[1~25]`, `00~12` | 247 | `[Erai-raws] Jujutsu Kaisen - 01 ~ 24` | **113 unknown** | range batch |
| `Season 04 - 32-36` | few | `[Anime Time] … (Season 04 - 32-36)` | **single episode 32** | season 4, range 32-36 |
| Batch words: `Batch`, `Complete`, `Full Series`, `Integral`, `Collection` | 693 / 158 | `(Ultimate Collection)` | mostly ok | - |
| BD volumes `Vol.4`, `Vol 2`, `Volume 3` | 57 | `[Drake] Overlord III Vol. 1` | **29 unknown** | partial batch → view page |

## 2. Season formats

| Pattern | Titles | Example | Today | Needed |
|---|---|---|---|---|
| `S04`, `S4` (no episode) | 1,503 | `Attack.on.Titan.S04.1080p…` | ok (batch) | - |
| `Season N` | 1,040 | `Overlord Season 4 - 13` | ok | - |
| `Nth Season` | 466 | `Boku no Hero Academia 4th Season - 01 ~ 25` | **43 unknown** | season N (+ range) |
| Roman numeral after the name | 400 | `Mob Psycho 100 III - 10`, `Overlord IV`, `Mushoku Tensei II` | **265 episodes filed as season 1**, 74 unknown | season from numeral; not `Infinity Castle I` (movie part) |
| Plain number after the name | ~115 | `My Hero Academia 2 + Special`, `Kaguya-sama … Renai Zunousen 2 - 1 ~ 12` | season 1 | only via AniList names ("… 2" is an entry name); `Mob Psycho 100`, `Jujutsu Kaisen 0` are names, not seasons |
| Glued number | 23 | `danmachi5` | no match | split trailing digits off a known name |
| Multi-season `S01-S04`, `S1-3`, `S01-05`, `Season 1-4`, `Season 1 - 3`, `S1+S2+S3+S4`, `Season 01 + Season 02` | 250 | `[Suki Desu] … (Season 1 - 3)` | **98 single episode** | multi-season batch |
| `Final Season`, `The Final Season Part 2/3`, `Final, Part 1` | 87 | `Attack on Titan - S04 - The Final Season - Part 2` | ok (sentinel) | map to entries by name |
| `Part N`, `Cour N`, `pt3`, `P1`, `S04Part01` | 255 / 7 | `Spy x Family Part 2 - 01 ~ 13` | 41 unknown | cour → entry by name; `S04Part01` scene style |
| Season named by arc / subtitle | 264+ | `Kaigyoku Gyokusetsu`, `Ultra Romantic`, `Stone Wars`, `Swordsmith Village Arc`, `Sennen Kessen-hen`, `To the Top`, `Alicization`, `The Second Plate`, `Gintama'` / `Gintama°` / `Gintama.` | unknown | entry by its AniList name (longest name wins) |
| `S00`, `S00Exx`, `(S03E18.5)` | 55 | `[Flugel] Attack on Titan S00 (Specials)` | episode season 0 | specials: keep off TV seasons' episode grid |

## 3. Kind formats (not a TV season episode)

| Kind | Titles | Example | Needed |
|---|---|---|---|
| Movie | 335 | `Shingeki no Kyojin: Kanketsu-hen - The Last Attack - Movie`, `Gekijouban …`, `Jujutsu Kaisen 0` | belongs to the AniList MOVIE entry (longest name); dropped from TV entries unless a multi-season pack also names the season |
| OVA / OAD / ONA / Special / SP | 285 | `Shingeki no Kyojin OAD - 08`, `Mob Psycho 100 II - OVA` | OVA/SPECIAL entry; off the TV grid |
| Recap / re-edit | few | `Attack on Titan Chronicle`, `Re:Zero … Shin Henshuu-ban - 01 ~ 13`, `Frieren Henshu` | own AniList entry when one exists, else unsure |
| Spin-off sharing the name | many | `Gintama - Mr. Ginpachi's Zany Class`, `Attack on Titan - Junior High`, `Sword Art Online Alternative: Gun Gale Online`, `Vigilante - Boku no Hero Academia Illegals`, `Kono Subarashii Sekai ni Bakuen wo!` | longest name wins puts them on their own entry (verified: 139 Ginpachi, 129 GGO titles) |
| Other show from the same search | 392 | Hunter x Hunter → `Shangri-La Frontier`, `City Hunter`; Kingdom → `The Twelve Kingdoms` | no name match → dropped (works today) |

## 4. Name formats (matching)

Matching against *all* of a franchise's AniList names leaves 392 of 8,342
titles unmatched, almost all of them other shows (§3). The real misses:

- **Short name before the colon** (base name): `Kaguya-sama wa Kokurasetai`
  ×13, `Code Geass` ×43, `Mushoku Tensei` ×49. Covered by the planned base
  name rule.
- **Apostrophes split words**: `normalize` turns `Journey's` into
  `journey s`, so `Frieren Beyond Journeys End` ×7 never matches. Apostrophes
  (`'`, `’`, `` ` ``) must be *removed*, not treated as separators.
  `God's` / `Gods` / ``God`s`` (Konosuba) is the same case.
- Separators already handled by `normalize`: dots (`Attack.on.Titan.S04`,
  198 scene-style titles), underscores (`[Coalgirls]_Shingeki_no_Kyojin_`),
  `/`, `:`, `-…-` (`Re:ZERO -Starting Life in Another World-`), `Re.Zero`.
- Alt titles after `|`, `/` or in parentheses (962 with `|`) often carry the
  season too: `| Season 3/S03`, `(Tensei Shitara Slime Datta Ken 3rd
  Season, …)`. That second one is *wrong*: VARYG writes `S04E03` with a
  3rd-season alt title. **An `SxxEyy` marker outranks every other season
  marker.**
- Brackets that are part of the name: `[DB] [Oshi no Ko] 3rd Season`.
  `normalize` drops the brackets, so the name matches. The group tag is
  read from the first bracket (`DB`), which is right. No title in the
  corpus *starts* with `[Oshi no Ko]`, so a name taken as a group tag
  wasn't observed. Not planned.

**Longest name wins** (each title goes to the entry whose matched name is
longest): routes arc-named seasons and spin-offs correctly when the arc or
spin-off name appears (`Swordsmith Village Arc`, `Science Future Part 3`,
`Overlord III`, `Mr. Ginpachi's Zany Class`, `Gun Gale Online`, movies).
It ties when two entries share the name (Kaguya-sama ×54, Gintama ×64):
then the season marker decides.

## 5. Season numbers disagree between groups

This is the biggest finding: **there is no single season number**. Demon
Slayer's Swordsmith Village Arc is `S3` on Crunchyroll, but Salieri writes
`S4 - Swordsmith Village`. Hashira Training is `S05` for Yameii, `S4` by
Crunchyroll. Same for Gintama (`S10`, `Season 9`), Bleach (TYBW as `S17E46`
continuing the 2004 series, or `S01E42` as its own show), Dr. Stone
(`S04E37` vs `Science Future`), Attack on Titan (`S04E29` = Final Season
Part 3). AniList itself has no season numbers.

Most scene/WEB groups (VARYG, ToonsHub, Yameii, Sonarr-driven uploads)
number by **TVDB**. The community dataset
[Fribb/anime-lists](https://github.com/Fribb/anime-lists)
(`anime-list-full.json`, 39,577 entries, 7.5 MB, updated weekly) maps
each `anilist_id` to its `season.tvdb`. Checked against the corpus: Demon
Slayer Mugen Train TV = 2, Entertainment District = 3, Swordsmith = 4,
Hashira = 5 (exactly Salieri's / Yameii's numbers). Mushoku S3 = 3, Slime
S3/S4 = 3/4, Food Wars Second Plate = 2, Totsuki Train Arc = 3, Fourth
Plate = 4. The repo declares **no license**, so it should be fetched at
runtime and cached (like AniList data), not bundled.

So an entry gets a **set** of season numbers: its TVDB season (dataset), the
number in its own AniList titles, and its TV-chain position as a fallback
(not in the dataset, or offline). A release season marker matches if it is
in the set. Two sibling entries can share a number (Crunchyroll vs TVDB
numbering). Then the release's name (longest name wins), its episode
numbers against each entry's episode count, and finally the view page
decide.

## 6. Titles with no signal at all

828 titles (10%) carry no season, episode, range, batch, volume, movie or
OVA marker. Examples: `[Kametsu] Attack on Titan (Shingeki no Kyojin) (BD
1080p Hi10 FLAC)` 41 GiB, `[BDMV][USA] Attack on Titan` 166 GiB, `Attack on
Titan/Shingeki no Kyojin ALL SEASONS`, `[SubsPlease] Jujutsu Kaisen -
Kaigyoku Gyokusetsu (1080p)` (an arc title = an AniList entry name),
`Jujutsu Kaisen [BD Remux][Part -1]`. Big size means a batch, but not
*which* seasons. These are the **unsure** releases: if the name pins an
entry, use it; otherwise use the view page (file names/folders per PLAN.md).
`ALL SEASONS` / `Complete Series` mark a multi-season pack.

## What changed in the plan because of this

See PLAN.md "Season-aware source matching" (updated the same day):
TVDB season sets (Fribb dataset), `SxxEyy` precedence, apostrophe
normalization, the full parser list from §1-§2, and kinds from §3 kept
off TV episode grids.
