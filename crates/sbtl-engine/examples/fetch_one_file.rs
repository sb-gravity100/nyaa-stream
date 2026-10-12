//! D1 spike (PLAN.md "AnimeTosho dataset" > in-app download): downloads ONE
//! file of the 1 TB `at-data.torrent` through `sbtl-engine`, without touching
//! the other 14,295 files.
//!
//! ```text
//! cargo run -p sbtl-engine --release --example fetch_one_file -- \
//!     <at-data.torrent> <download_dir> <out_dir> [--suffix database-dump.7z] \
//!     [--size 2943660826] [--max-mb 100] [--max-secs 120] [--mode reader|preload]
//! ```
//!
//! - `reader` (default): a foreground sequential reader on the file copies the
//!   bytes it reads to `<out_dir>/<file name>`; stops after `--max-mb` or
//!   `--max-secs` (0 = no limit).
//!   `--kind background` uses an idle-NONE reader (only its sequential window
//!   downloads: the right choice for one file of a huge torrent; a foreground
//!   reader's SEQ_AHEAD also queues every other piece at background class).
//!   `--discard` drops the bytes instead of copying (sbtl's own storage file is
//!   the result); `--start-mb N` starts the reader N MiB into the file.
//! - `preload` (do not use on a huge torrent, see above): no reader; `Torrent::set_preload_file` fetches the file in
//!   order into the torrent's own storage (`<download_dir>/<torrent path>`) and
//!   progress is polled through `Torrent::stats()` (the "download this file
//!   only" mode). Nothing is copied.
//!
//! Prints metadata/first-byte timings, speed, peers, the process working set
//! and how many bytes of OTHER files have verified data.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use sbtl_engine::{Engine, ReadKind, Source};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn mib(b: u64) -> f64 {
    b as f64 / 1_048_576.0
}

#[cfg(windows)]
fn working_set_mb() -> Option<f64> {
    // `tasklist` is enough for a spike and avoids a winapi dependency.
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).WorkingSet64", std::process::id())])
        .output()
        .ok()?;
    let b: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(mib(b))
}

#[cfg(not(windows))]
fn working_set_mb() -> Option<f64> {
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        anyhow::bail!("usage: fetch_one_file <torrent> <download_dir> <out_dir> [--suffix S] [--size N] [--max-mb N] [--max-secs N] [--mode reader|preload]");
    }
    let torrent_path = PathBuf::from(&args[0]);
    let dl_dir = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    let suffix = arg(&args, "--suffix").unwrap_or_else(|| "database-dump.7z".into());
    let want_size: u64 = arg(&args, "--size").and_then(|s| s.parse().ok()).unwrap_or(2_943_660_826);
    let max_bytes = arg(&args, "--max-mb").and_then(|s| s.parse::<u64>().ok()).unwrap_or(100) * 1_048_576;
    let max_secs: u64 = arg(&args, "--max-secs").and_then(|s| s.parse().ok()).unwrap_or(120);
    let preload = arg(&args, "--mode").as_deref() == Some("preload");
    let kind = if arg(&args, "--kind").as_deref() == Some("background") { ReadKind::Background } else { ReadKind::Foreground };
    let discard = args.iter().any(|a| a == "--discard");
    let start_mb: u64 = arg(&args, "--start-mb").and_then(|s| s.parse().ok()).unwrap_or(0);

    let t0 = Instant::now();
    let bytes = std::fs::read(&torrent_path)?;
    println!("torrent file: {:.2} MiB", mib(bytes.len() as u64));
    println!("ws at start: {:?} MiB", working_set_mb());

    let engine = Engine::start(dl_dir.clone())?;
    let t_add = Instant::now();
    let torrent = engine.add(Source::Bytes(bytes)).await?;
    println!("add() returned in {:?} (has metadata at once; parse + resume check)", t_add.elapsed());
    println!("ws after add: {:?} MiB", working_set_mb());

    let files = torrent.files();
    println!("{} files in torrent '{}'", files.len(), torrent.name().unwrap_or_default());
    // exact path-component suffix + exact size; never index order
    let idx = files
        .iter()
        .position(|f| {
            let n = f.name.replace('\\', "/");
            (n == suffix || n.ends_with(&format!("/{suffix}"))) && f.length == want_size
        })
        .ok_or_else(|| anyhow::anyhow!("no file matching {suffix} with size {want_size}"))?;
    let file = &files[idx];
    println!("target: #{idx} {} ({} bytes = {:.2} GiB)", file.name, file.length, file.length as f64 / 1_073_741_824.0);

    let report = |label: &str| {
        let t = Instant::now();
        let st = torrent.stats();
        let stats_ms = t.elapsed().as_millis();
        let target = &st.files[idx];
        let other: u64 = st.files.iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, f)| f.downloaded).sum();
        println!(
            "[{:>6.1}s] {label} state={:?} rate={:.2} MiB/s peers={}/{} torrent_dl={:.1} MiB target_on_disk={:.1} MiB OTHER_files_verified={:.1} MiB stats()={}ms ws={:?}",
            t0.elapsed().as_secs_f64(),
            st.state,
            st.download_rate / 1_048_576.0,
            st.peers,
            st.peers_known,
            mib(st.downloaded),
            mib(target.downloaded),
            mib(other),
            stats_ms,
            working_set_mb().map(|m| m.round()),
        );
        target.downloaded
    };

    let deadline = if max_secs == 0 { None } else { Some(Instant::now() + Duration::from_secs(max_secs)) };
    let expired = || deadline.is_some_and(|d| Instant::now() >= d);

    if preload {
        torrent.set_preload_file(Some(idx))?;
        println!("preload started (no reader)");
        let started = Instant::now();
        let mut first = None;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let done = report("preload");
            if done > 0 && first.is_none() {
                first = Some(started.elapsed());
                println!("first verified data after {:?}", first.unwrap());
            }
            if done >= max_bytes || done >= file.length || expired() {
                break;
            }
        }
        println!("on-disk path: {:?}", torrent.file_path(idx));
    } else {
        std::fs::create_dir_all(&out_dir)?;
        let out_name = file.name.rsplit(['/', '\\']).next().unwrap_or("out.bin");
        let out_path = out_dir.join(if discard { "discard.bin" } else { out_name });
        let mut out = tokio::fs::OpenOptions::new().write(true).create(true).truncate(false).open(&out_path).await?;
        let resume_at = if discard { start_mb * 1_048_576 } else { out.metadata().await?.len() };
        let mut reader = torrent.open(idx, resume_at, kind)?;
        println!("reader opened at offset {resume_at}");
        use tokio::io::AsyncSeekExt;
        out.seek(std::io::SeekFrom::Start(resume_at)).await?;
        let opened = Instant::now();
        let mut buf = vec![0u8; 1 << 20];
        let mut copied: u64 = 0;
        let mut first_byte = None;
        let mut last_report = Instant::now();
        loop {
            let n = match tokio::time::timeout(Duration::from_secs(30), reader.read(&mut buf)).await {
                Ok(r) => r?,
                Err(_) => {
                    println!("no data for 30 s");
                    report("stall");
                    break;
                }
            };
            if n == 0 {
                println!("EOF");
                break;
            }
            if first_byte.is_none() {
                first_byte = Some(opened.elapsed());
                println!("time to first byte: {:?} (since reader open), {:?} since start", first_byte.unwrap(), t0.elapsed());
            }
            if !discard {
                out.write_all(&buf[..n]).await?;
            }
            copied += n as u64;
            if last_report.elapsed() >= Duration::from_secs(5) {
                last_report = Instant::now();
                println!("  copied {:.1} MiB ({:.2} MiB/s avg since first byte)", mib(copied), mib(copied) / (opened.elapsed() - first_byte.unwrap()).as_secs_f64().max(0.001));
                report("reader");
            }
            if copied >= max_bytes || expired() {
                break;
            }
        }
        out.flush().await?;
        println!("copied {:.1} MiB total in {:?} -> {}", mib(copied), opened.elapsed(), out_path.display());
        drop(reader);
    }
    report("final");
    println!("total run {:?}", t0.elapsed());
    Ok(())
}
