//! AnimeTosho dump tool.
//!
//! `cargo run --release -p nyaa-client --example build_tosho -- --stats <database-dump.7z> [inner\path.sql ...]`
//! streams each SQL file out of the archive with `7z e -so` (never unpacked
//! to disk) and prints row counts per table plus the figures that decide the
//! dataset's size cut: `toto_toto` aid coverage and `resolveapproved` split,
//! and the size of the batch file lists.
//!
//! Default inner files: `data\toto_repl.sql`.

use std::collections::{BTreeMap, HashSet};
use std::io::BufReader;
use std::process::{Command, Stdio};

use nyaa_client::tosho::InsertReader;

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let stats = args.first().map(String::as_str) == Some("--stats");
    if stats {
        args.remove(0);
    }
    anyhow::ensure!(stats && !args.is_empty(), "usage: build_tosho --stats <database-dump.7z> [inner\\path.sql ...]");
    let archive = args.remove(0);
    let inner = if args.is_empty() { vec!["data\\toto_repl.sql".to_string()] } else { args };
    for file in inner {
        stats_for(&archive, &file)?;
    }
    Ok(())
}

#[derive(Default)]
struct TotoStats {
    rows: u64,
    nyaa_si: u64,
    sukebei: u64,
    with_aid: u64,
    nyaa_si_with_aid: u64,
    deleted: u64,
    dupes: u64,
    batches: u64,
    resolve: BTreeMap<i64, u64>,
}

fn stats_for(archive: &str, inner: &str) -> anyhow::Result<()> {
    println!("== {inner}");
    let mut child = Command::new("7z").args(["e", "-so", archive, inner]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let mut reader = InsertReader::new(BufReader::with_capacity(1 << 20, child.stdout.take().expect("piped stdout")));
    let started = std::time::Instant::now();

    let mut tables: BTreeMap<String, u64> = BTreeMap::new();
    let mut toto = TotoStats::default();
    // toto ids of anime batches on nyaa.si, to size their file lists.
    let mut batch_ids: HashSet<i64> = HashSet::new();
    let (mut file_rows, mut batch_file_rows, mut batch_file_bytes, mut all_file_bytes) = (0u64, 0u64, 0u64, 0u64);
    let mut tvdb_rows = 0u64;
    let mut toto_seen_before_files = false;

    while let Some(header) = reader.next_insert()? {
        let table = header.table.clone();
        match table.as_str() {
            "toto_toto" => {
                toto_seen_before_files = true;
                let idx = |c: &str| header.index(c).unwrap_or_else(|| panic!("toto_toto has no column {c}"));
                let (id, nyaa_id, subdom, aid, resolve, deleted, isdupe, files) = (
                    idx("id"),
                    idx("nyaa_id"),
                    idx("nyaa_subdom"),
                    idx("aid"),
                    idx("resolveapproved"),
                    idx("deleted"),
                    idx("isdupe"),
                    idx("torrentfiles"),
                );
                while let Some(row) = reader.next_row()? {
                    *tables.entry(table.clone()).or_default() += 1;
                    toto.rows += 1;
                    let int = |i: usize| row[i].as_i64().unwrap_or(0);
                    let on_nyaa = int(nyaa_id) > 0 && row[subdom].as_bytes().map_or(true, |b| b.is_empty());
                    if int(nyaa_id) > 0 && !on_nyaa {
                        toto.sukebei += 1;
                    }
                    if on_nyaa {
                        toto.nyaa_si += 1;
                    }
                    let has_aid = int(aid) > 0;
                    if has_aid {
                        toto.with_aid += 1;
                        *toto.resolve.entry(int(resolve)).or_default() += 1;
                        if on_nyaa {
                            toto.nyaa_si_with_aid += 1;
                        }
                    }
                    if int(deleted) != 0 {
                        toto.deleted += 1;
                    }
                    if int(isdupe) != 0 {
                        toto.dupes += 1;
                    }
                    if on_nyaa && has_aid && int(files) > 1 && int(deleted) == 0 {
                        toto.batches += 1;
                        batch_ids.insert(int(id));
                    }
                }
            }
            "toto_files" => {
                let toto_id = header.index("toto_id").expect("toto_files.toto_id");
                let name = header.index("filename").expect("toto_files.filename");
                while let Some(row) = reader.next_row()? {
                    *tables.entry(table.clone()).or_default() += 1;
                    file_rows += 1;
                    all_file_bytes += row[name].as_bytes().map_or(0, |b| b.len() as u64) + 8;
                    if row[toto_id].as_i64().is_some_and(|id| batch_ids.contains(&id)) {
                        batch_file_rows += 1;
                        batch_file_bytes += row[name].as_bytes().map_or(0, |b| b.len() as u64) + 8;
                    }
                }
            }
            "toto_anidb_tvdb" => {
                while reader.next_row()?.is_some() {
                    *tables.entry(table.clone()).or_default() += 1;
                    tvdb_rows += 1;
                }
            }
            _ => {
                // Count rows only for the tables that matter; skipping is much faster.
                reader.skip_statement()?;
                *tables.entry(table.clone()).or_default() += 0;
            }
        }
    }
    let _ = child.wait();

    println!("took {:.0}s", started.elapsed().as_secs_f64());
    println!("rows parsed per table: {tables:#?}");
    println!(
        "toto_toto: {} rows, nyaa.si {} (+ sukebei {}), with aid {} ({} on nyaa.si), deleted {}, dupes {}",
        toto.rows, toto.nyaa_si, toto.sukebei, toto.with_aid, toto.nyaa_si_with_aid, toto.deleted, toto.dupes
    );
    println!("resolveapproved among rows with aid: {:?}", toto.resolve);
    println!("anime batches on nyaa.si (aid known, > 1 file): {}", toto.batches);
    println!("toto_files: {file_rows} rows, ~{:.0} MB of file names + sizes (all torrents, upper bound for release_files)", all_file_bytes as f64 / 1e6);
    if toto_seen_before_files && batch_file_rows > 0 {
        // Only meaningful when toto_toto is dumped before toto_files.
        println!("  batch file rows {batch_file_rows}, ~{:.0} MB", batch_file_bytes as f64 / 1e6);
    }
    println!("toto_anidb_tvdb: {tvdb_rows} rows");
    Ok(())
}
