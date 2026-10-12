//! In-app build of the AnimeTosho dataset straight from the downloaded
//! `database-dump.7z` (PLAN.md "AnimeTosho dataset" > "Import Tosho Data"):
//! a thin wrapper over [`crate::sevenz_stream`] and [`crate::tosho_build`], so
//! the app does not need an external `7z`.

use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::sevenz_stream;
use crate::tosho_build::{dataset_file_name, ToshoBuilder};

/// Rough uncompressed size of the two SQL entries (`toto_repl.sql` 5.5 GB +
/// `arcscrape.sql` 5.1 GB): the progress total, the real one is not exposed.
pub const ESTIMATED_SQL_BYTES: u64 = 10_600_000_000;

const TOTO: &str = r"data\toto_repl.sql";
const ARCHIVE: &str = r"data\arcscrape.sql";

/// `BufRead` that counts consumed bytes, reports them and aborts on cancel.
struct Counting<'a, R, F: FnMut(u64)> {
    inner: R,
    cancel: &'a AtomicBool,
    base: u64,
    seen: u64,
    last_reported: u64,
    report: F,
}

impl<R, F: FnMut(u64)> Counting<'_, R, F> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        Ok(())
    }

    fn advance(&mut self, n: usize) {
        self.seen += n as u64;
        if self.seen - self.last_reported >= 8 << 20 {
            self.last_reported = self.seen;
            (self.report)(self.base + self.seen);
        }
    }
}

impl<R: Read, F: FnMut(u64)> Read for Counting<'_, R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        let n = self.inner.read(buf)?;
        self.advance(n);
        Ok(n)
    }
}

impl<R: BufRead, F: FnMut(u64)> BufRead for Counting<'_, R, F> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.check()?;
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
        self.advance(amt);
    }
}

/// True when `err` (anywhere in its chain) is the cancel marker.
pub fn is_cancelled(err: &anyhow::Error) -> bool {
    err.chain().any(|e| e.downcast_ref::<io::Error>().is_some_and(|io| io.kind() == io::ErrorKind::Interrupted && io.to_string() == "cancelled"))
}

/// Builds `tosho-dataset-<date>.db` in `out_dir` from `archive`
/// (both SQL entries), calling `progress(bytes_read, estimated_total)`.
/// Returns the dataset path. Writes to a `.part` file first, so a present
/// dataset is always complete.
pub fn build_from_archive(archive: &Path, out_dir: &Path, progress: impl FnMut(u64, u64)) -> anyhow::Result<PathBuf> {
    build_from_archive_cancellable(archive, out_dir, &AtomicBool::new(false), progress)
}

/// [`build_from_archive`] that stops with a cancel error (see [`is_cancelled`]) once `cancel` is set.
pub fn build_from_archive_cancellable(archive: &Path, out_dir: &Path, cancel: &AtomicBool, mut progress: impl FnMut(u64, u64)) -> anyhow::Result<PathBuf> {
    let date = sevenz_stream::entry_date(archive, TOTO)?;
    std::fs::create_dir_all(out_dir)?;
    let out = out_dir.join(dataset_file_name(&date));
    let part = out.with_extension("db.part");
    let _ = std::fs::remove_file(&part);
    tracing::info!(archive = %archive.display(), out = %out.display(), %date, "building the AnimeTosho dataset");
    let started = std::time::Instant::now();

    let result = (|| -> anyhow::Result<()> {
        let mut builder = ToshoBuilder::create(&part, &date)?;
        let mut report = |n: u64| progress(n.min(ESTIMATED_SQL_BYTES), ESTIMATED_SQL_BYTES);
        let mut sizes = 0u64;
        for (entry, is_toto) in [(TOTO, true), (ARCHIVE, false)] {
            let reader = sevenz_stream::open_entry(archive, entry)?;
            let mut counting = Counting { inner: reader, cancel, base: sizes, seen: 0, last_reported: 0, report: &mut report };
            if is_toto {
                builder.read_toto(&mut counting)?;
            } else {
                builder.read_archive(&mut counting)?;
            }
            sizes += counting.seen;
            tracing::info!(entry, bytes = counting.seen, secs = started.elapsed().as_secs(), "dataset build: entry done");
        }
        let counts = builder.finish()?;
        tracing::info!(?counts, secs = started.elapsed().as_secs(), "dataset build: finished");
        Ok(())
    })();
    match result {
        Ok(()) => {
            let _ = std::fs::remove_file(&out);
            std::fs::rename(&part, &out)?;
            progress(ESTIMATED_SQL_BYTES, ESTIMATED_SQL_BYTES);
            Ok(out)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&part);
            if is_cancelled(&err) {
                tracing::info!("dataset build cancelled");
            } else {
                tracing::error!(%err, "dataset build failed");
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_reader_reports_and_cancels() {
        let data = vec![7u8; 20 << 20];
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        {
            let mut c = Counting { inner: io::BufReader::new(&data[..]), cancel: &cancel, base: 100, seen: 0, last_reported: 0, report: |n| seen.push(n) };
            let mut sink = Vec::new();
            c.read_to_end(&mut sink).unwrap();
            assert_eq!(sink.len(), data.len());
            cancel.store(true, Ordering::Relaxed);
            let err = c.read(&mut [0u8; 4]).unwrap_err();
            assert!(is_cancelled(&anyhow::Error::from(err)));
        }
        assert!(!seen.is_empty() && seen.iter().all(|n| *n >= 100));
    }
}
