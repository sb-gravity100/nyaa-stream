//! AnimeTosho dump tool.
//!
//! `cargo run --release -p nyaa-client --example build_tosho -- --stats <database-dump.7z> [inner\path.sql ...]`
//! streams each SQL file out of the archive (never unpacked to disk) with the
//! in-process reader `nyaa_client::sevenz_stream`, or with `7z e -so` when
//! `--external-7z` is given and prints row counts per table plus the figures that decide the
//! dataset's size cut: `toto_toto` aid coverage and `resolveapproved` split,
//! and the size of the batch file lists.
//!
//! Default inner files: `data\toto_repl.sql`.
//!
//! `cargo run --release -p nyaa-client --example build_tosho -- <database-dump.7z> <out.db | out-dir> [--dump-date YYYY-MM-DD] [--no-archive] [--external-7z]`
//! builds the dataset (`tosho-dataset-<YYYYMMDD>.db` inside `out-dir`, or
//! exactly `out.db`): `toto_repl.sql` first, then `arcscrape.sql` (nyaa.si
//! archive rows the first one lacks) unless `--no-archive`. The dump date
//! defaults to the modified date of `data\toto_repl.sql` in the archive.

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use nyaa_client::sevenz_stream;
use nyaa_client::stem::{self, TrainConfig};
use nyaa_client::tosho::InsertReader;
use nyaa_client::tosho_build::{dataset_file_name, ToshoBuilder};

fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_some() {
        tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    }
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let stats = args.first().map(String::as_str) == Some("--stats");
    if stats {
        args.remove(0);
        anyhow::ensure!(!args.is_empty(), "usage: build_tosho --stats [--external-7z] <database-dump.7z> [inner.sql ...]");
        let external = take_switch(&mut args, "--external-7z");
        let archive = args.remove(0);
        let inner = if args.is_empty() { vec![r"data\toto_repl.sql".to_string()] } else { args };
        for file in inner {
            stats_for(&archive, &file, external)?;
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--holdout-only") {
        args.remove(i);
        let config = stem_config(&mut args)?;
        anyhow::ensure!(args.len() == 2, "usage: build_tosho --holdout-only <dataset.db> YYYY-MM-DD [--stem-share F] [--stem-releases N] [--stem-groups N]");
        return holdout(&PathBuf::from(&args[0]), &args[1], config);
    }
    build(args)
}

/// Removes the bare switch `name` from `args`, true if it was there.
fn take_switch(args: &mut Vec<String>, name: &str) -> bool {
    let found = args.iter().position(|a| a == name);
    if let Some(i) = found {
        args.remove(i);
    }
    found.is_some()
}

/// Removes `--name value` from `args`.
fn take_flag(args: &mut Vec<String>, name: &str) -> anyhow::Result<Option<String>> {
    let Some(i) = args.iter().position(|a| a == name) else { return Ok(None) };
    anyhow::ensure!(i + 1 < args.len(), "{name} needs a value");
    let value = args.remove(i + 1);
    args.remove(i);
    Ok(Some(value))
}

fn stem_config(args: &mut Vec<String>) -> anyhow::Result<TrainConfig> {
    let mut config = TrainConfig::default();
    if let Some(v) = take_flag(args, "--stem-share")? {
        config.min_share = v.parse()?;
    }
    if let Some(v) = take_flag(args, "--stem-releases")? {
        config.min_releases = v.parse()?;
    }
    if let Some(v) = take_flag(args, "--stem-groups")? {
        config.min_groups = v.parse()?;
    }
    Ok(config)
}

fn holdout(dataset: &std::path::Path, cutoff: &str, config: TrainConfig) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let conn = rusqlite::Connection::open_with_flags(dataset, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (stems, e) = stem::holdout(&conn, cutoff, config)?;
    println!("stem holdout (train < {cutoff}, test = trusted releases from {cutoff}), {config:?}");
    println!("  stems kept {stems}; test releases {}; resolved {}; correct {}", e.test, e.resolved, e.correct);
    println!("  coverage {:.2}% (of series seen in training: {:.2}% of {}); precision {:.3}%", e.coverage() * 100.0, e.known_series_coverage() * 100.0, e.known_series, e.precision() * 100.0);
    println!("  wrong resolutions {}; of them same TVDB show (season carry-over) {}", e.resolved - e.correct, e.wrong_same_show);
    println!("  took {:.0}s", started.elapsed().as_secs_f64());
    Ok(())
}

fn spawn_7z(archive: &str, inner: &str) -> anyhow::Result<Child> {
    Ok(Command::new("7z").args(["e", "-so", archive, inner]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?)
}

/// One SQL file of the archive as a buffered stream: in-process by default,
/// the external `7z` when `external` (kept so the two can be compared).
struct Source {
    reader: Box<dyn BufRead>,
    child: Option<Child>,
}

impl Source {
    fn open(archive: &str, inner: &str, external: bool) -> anyhow::Result<Self> {
        if external {
            let mut child = spawn_7z(archive, inner)?;
            let reader = Box::new(BufReader::with_capacity(1 << 20, child.stdout.take().expect("piped stdout")));
            Ok(Self { reader, child: Some(child) })
        } else {
            Ok(Self { reader: Box::new(sevenz_stream::open_entry(std::path::Path::new(archive), inner)?), child: None })
        }
    }

    fn finish(self) {
        drop(self.reader);
        if let Some(mut c) = self.child {
            let _ = c.wait();
        }
    }
}

/// `YYYY-MM-DD` of `inner` in the archive (`7z l` when `external`).
fn dump_date_of(archive: &str, inner: &str, external: bool) -> anyhow::Result<String> {
    if !external {
        return Ok(sevenz_stream::entry_date(std::path::Path::new(archive), inner)?);
    }
    let listing = Command::new("7z").args(["l", archive, inner]).output()?;
    let text = String::from_utf8_lossy(&listing.stdout);
    text.lines()
        .find(|l| l.contains(inner))
        .and_then(|l| l.get(..10))
        .filter(|d| d.len() == 10 && d.as_bytes()[4] == b'-')
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("could not read the date of {inner} from `7z l`; pass --dump-date"))
}

fn build(mut args: Vec<String>) -> anyhow::Result<()> {
    let mut date = None;
    if let Some(i) = args.iter().position(|a| a == "--dump-date") {
        anyhow::ensure!(i + 1 < args.len(), "--dump-date needs a value");
        date = Some(args.remove(i + 1));
        args.remove(i);
    }
    let external = take_switch(&mut args, "--external-7z");
    let archive_too = if let Some(i) = args.iter().position(|a| a == "--no-archive") {
        args.remove(i);
        false
    } else {
        true
    };
    let holdout_date = take_flag(&mut args, "--holdout")?;
    let config = stem_config(&mut args)?;
    anyhow::ensure!(args.len() == 2, "usage: build_tosho <database-dump.7z> <out.db | out-dir> [--dump-date YYYY-MM-DD] [--no-archive] [--external-7z] [--holdout YYYY-MM-DD] [--stem-share F] [--stem-releases N] [--stem-groups N]  |  build_tosho --stats [--external-7z] <database-dump.7z> [inner.sql ...]");
    let (archive, out) = (args.remove(0), PathBuf::from(args.remove(0)));
    let date = match date {
        Some(d) => d,
        None => dump_date_of(&archive, r"data\toto_repl.sql", external)?,
    };
    let out = if out.is_dir() || out.extension().is_none() { out.join(dataset_file_name(&date)) } else { out };
    let started = std::time::Instant::now();
    let mut builder = ToshoBuilder::create(&out, &date)?;
    builder.set_stem_config(config);

    let mut src = Source::open(&archive, r"data\toto_repl.sql", external)?;
    builder.read_toto(&mut src.reader)?;
    src.finish();
    println!("toto_repl.sql done after {:.0}s", started.elapsed().as_secs_f64());
    if archive_too {
        let mut src = Source::open(&archive, r"data\arcscrape.sql", external)?;
        builder.read_archive(&mut src.reader)?;
        src.finish();
        println!("arcscrape.sql done after {:.0}s", started.elapsed().as_secs_f64());
    }
    let counts = builder.finish()?;
    println!("wrote {} ({:.0} MB) in {:.0}s: {counts:?}", out.display(), std::fs::metadata(&out)?.len() as f64 / 1e6, started.elapsed().as_secs_f64());
    if let Some(cutoff) = holdout_date {
        holdout(&out, &cutoff, config)?;
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

fn stats_for(archive: &str, inner: &str, external: bool) -> anyhow::Result<()> {
    println!("== {inner}");
    let mut src = Source::open(archive, inner, external)?;
    let mut reader = InsertReader::new(&mut src.reader);
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
    drop(reader);
    src.finish();

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
