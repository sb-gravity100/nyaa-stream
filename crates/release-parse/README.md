# release-parse

Anime release title parser for nyaa-stream: a Rust port of
[anitopy](https://github.com/igorcmoura/anitopy) 2.1.1 by Igor Cescon de
Moura, itself a Python port of [Anitomy](https://github.com/erengy/anitomy)
by Eren Okka.

## License

This crate is a derived work of anitopy and is licensed under the
**Mozilla Public License 2.0** (`LICENSE`). Every source file carries the
MPL-2.0 header. MPL-2.0 is file-level copyleft: these files stay MPL-2.0,
and the rest of nyaa-stream is unaffected.

## Status

1. **Faithful port** (this version): the same tokenizer, keyword tables
   and number rules as anitopy. Checked by diffing
   `examples/parse_lines.rs` output against Python anitopy over 8,894 real
   nyaa.si titles (TITLE_ANALYSIS.md's corpus + the labelled fixtures):
   0 differences. That includes anitopy's own quirks: the `第` episode
   prefix is the literal string `{2C` (Python reads `'\x7B2C'` as `{` +
   `2C`), and searching back from the first token wraps around to the
   last.
2. **Improvements** (`src/extra.rs`, plus marked edits in the ported
   modules): CJK episode/season counters (`第19话`, `第14～25話`, `第3期`),
   roman-numeral seasons (`Mob Psycho 100 III`), season ranges and lists
   (`S01-04`, `S1 - S5`, `(Season 1 - 3)`, `Season 1+2+Movies`), spaced
   `~` episode ranges, 4-digit episodes (`S01E1180`, `0001-1155`),
   `SxxEyy` seasons outranking alt-title seasons, audio channels / frame
   rates / `H 264` not read as episodes, `Jujutsu Kaisen 0` not episode 0,
   and the season-relative episode kept over an `Episode 93` absolute one.
   Each was checked by reviewing every changed title in the 10,151-title
   corpus (`src-tauri/tests/fixtures/nyaa_titles/corpus.txt`).
3. **Typed result** (`src/release.rs`): `parse_release` → `Release`
   (title, seasons, episode range, kind: episode / batch / movie /
   special / unknown, extras, group, version), the shape the app uses.

## Differences from anitopy

- Tokens are referenced by id instead of object identity, and parser
  state lives in a per-call context instead of globals (thread-safe).
- Where anitopy raises (`None.category` on a missing neighbour token) or
  loops forever (a lone trailing dash in the episode title search), this
  port treats the case as "no match" and stops.
- `is_latin_char` approximates Python's "Unicode name contains LATIN" with
  the Latin letter blocks.
- `int()` / `str.isdigit()` semantics are emulated for ASCII and
  full-width digits.
