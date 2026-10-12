//! In-process streaming reader for one entry of a `.7z` archive (the
//! AnimeTosho `database-dump.7z`), so end users need no `7z` binary.
//!
//! `sevenz-rust2` only offers a callback API (`BlockDecoder::for_each_entries`
//! hands the entry out as `&mut dyn Read` for the duration of the call). A
//! decoder thread therefore feeds 1 MiB chunks through a bounded channel and
//! [`EntryReader`] is the `Read`/`BufRead` end of that channel, which is what
//! `tosho::InsertReader` consumes. Nothing is unpacked to disk and at most
//! `QUEUE_CHUNKS` chunks are buffered. Only the 7z *block* holding the entry is
//! decoded (the dump is 4 blocks, so reaching `arcscrape.sql` costs nothing
//! extra); earlier entries of that same block (a solid block) are decoded and
//! dropped. Dropping the reader stops the decoder thread.

use std::fs::File;
use std::io::{self, BufRead, Read};
use std::path::Path;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;

use sevenz_rust2::{Archive, BlockDecoder, Password};

const CHUNK: usize = 1 << 20;
const QUEUE_CHUNKS: usize = 4;

type Chunk = io::Result<Vec<u8>>;

fn io_err(e: sevenz_rust2::Error) -> io::Error {
    match e {
        sevenz_rust2::Error::Io(e, _) => e,
        other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
    }
}

/// Normalises `/` and `\` so `data\toto_repl.sql` matches either spelling.
fn norm(name: &str) -> String {
    name.replace('\\', "/")
}

fn find_entry(archive: &Archive, entry: &str) -> io::Result<usize> {
    let want = norm(entry);
    archive
        .files
        .iter()
        .position(|f| norm(&f.name) == want)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{entry} is not in the archive")))
}

/// `YYYY-MM-DD` (UTC) of a FILETIME (100 ns ticks since 1601-01-01).
fn filetime_date(ticks: u64) -> String {
    let days = (ticks / 10_000_000 / 86_400) as i64 - 134_774; // days since 1970-01-01
    // civil_from_days (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD` (UTC) of the entry's modified time.
pub fn entry_date(archive: &Path, entry: &str) -> io::Result<String> {
    let a = Archive::open(archive).map_err(io_err)?;
    let f = &a.files[find_entry(&a, entry)?];
    if !f.has_last_modified_date {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{entry} has no modified time")));
    }
    Ok(filetime_date(u64::from(f.last_modified_date)))
}

/// Streaming `Read`/`BufRead` over one archive entry; see the module docs.
pub struct EntryReader {
    rx: Option<Receiver<Chunk>>,
    buf: Vec<u8>,
    pos: usize,
    worker: Option<JoinHandle<()>>,
}

impl EntryReader {
    fn fill(&mut self) -> io::Result<()> {
        while self.pos >= self.buf.len() {
            let Some(rx) = &self.rx else { return Ok(()) };
            match rx.recv() {
                Ok(Ok(chunk)) => {
                    self.buf = chunk;
                    self.pos = 0;
                }
                Ok(Err(e)) => {
                    self.rx = None;
                    return Err(e);
                }
                Err(_) => self.rx = None, // decoder finished
            }
        }
        Ok(())
    }
}

impl BufRead for EntryReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.fill()?;
        Ok(&self.buf[self.pos..])
    }
    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.buf.len());
    }
}

impl Read for EntryReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = {
            let avail = self.fill_buf()?;
            let n = avail.len().min(out.len());
            out[..n].copy_from_slice(&avail[..n]);
            n
        };
        self.consume(n);
        Ok(n)
    }
}

impl Drop for EntryReader {
    fn drop(&mut self) {
        self.rx = None; // the decoder's next send fails and it stops
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

fn decode(archive: &Archive, file: &mut File, target: usize, tx: &SyncSender<Chunk>) -> io::Result<()> {
    let block = archive.stream_map.file_block_index[target]
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "entry has no data"))?;
    let threads = std::env::var("SEVENZ_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(1u32);
    let password = Password::empty();
    let mut index = archive.stream_map.block_first_file_index[block];
    let mut failure: Option<io::Error> = None;
    let res = BlockDecoder::new(threads, block, archive, &password, file).for_each_entries(&mut |_entry, reader| {
        let this = index;
        index += 1;
        if this != target {
            // Solid block: earlier entries must be decoded to reach ours.
            io::copy(reader, &mut io::sink())?;
            return Ok(this < target);
        }
        loop {
            let mut chunk = vec![0u8; CHUNK];
            let mut n = 0;
            while n < CHUNK {
                match reader.read(&mut chunk[n..]) {
                    Ok(0) => break,
                    Ok(k) => n += k,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        failure = Some(io::Error::new(e.kind(), e.to_string()));
                        return Err(e.into());
                    }
                }
            }
            if n == 0 {
                return Ok(false);
            }
            chunk.truncate(n);
            if tx.send(Ok(chunk)).is_err() {
                return Ok(false); // reader dropped
            }
            if n < CHUNK {
                return Ok(false);
            }
        }
    });
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(failure.unwrap_or_else(|| io_err(e))),
    }
}

/// Opens `entry` (e.g. `data\toto_repl.sql`; `/` also works) of the 7z at `archive`.
pub fn open_entry(archive: &Path, entry: &str) -> io::Result<EntryReader> {
    let parsed = Archive::open(archive).map_err(io_err)?;
    let target = find_entry(&parsed, entry)?;
    let mut file = File::open(archive)?;
    let (tx, rx) = sync_channel(QUEUE_CHUNKS);
    let worker = std::thread::Builder::new().name("sevenz-decode".into()).spawn(move || {
        tracing::debug!("7z decoder thread started for entry #{target}");
        if let Err(e) = decode(&parsed, &mut file, target, &tx) {
            tracing::warn!("7z decode failed: {e}");
            let _ = tx.send(Err(e));
        }
    })?;
    Ok(EntryReader { rx: Some(rx), buf: Vec::new(), pos: 0, worker: Some(worker) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};

    fn payload(seed: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| ((i * 31 + seed as usize) % 251) as u8).collect()
    }

    /// Solid block (a, b, c; `b` spans several chunks) plus a separate block (d).
    fn fixture(path: &Path) -> Vec<(&'static str, Vec<u8>)> {
        let files = vec![
            ("data/a.sql", payload(1, 1000)),
            ("data/b.sql", payload(2, 2_500_000)),
            ("data/c.sql", payload(3, CHUNK)), // exactly one chunk
            ("data/d.sql", payload(4, 77)),
        ];
        let mut w = ArchiveWriter::create(path).unwrap();
        let solid: Vec<_> = files[..3].iter().map(|(n, _)| ArchiveEntry::new_file(n)).collect();
        let readers: Vec<_> = files[..3].iter().map(|(_, d)| SourceReader::new(d.as_slice())).collect();
        w.push_archive_entries(solid, readers).unwrap();
        w.push_archive_entry(ArchiveEntry::new_file("data/d.sql"), Some(files[3].1.as_slice())).unwrap();
        w.finish().unwrap();
        files
    }

    #[test]
    fn streams_each_entry_of_solid_and_separate_blocks() {
        let dir = std::env::temp_dir().join(format!("sevenz_stream_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.7z");
        let files = fixture(&path);
        for (name, data) in &files {
            let mut got = Vec::new();
            open_entry(&path, &name.replace('/', "\\")).unwrap().read_to_end(&mut got).unwrap();
            assert!(got == *data, "{name} differs ({} vs {} bytes)", got.len(), data.len());
        }
        assert_eq!(open_entry(&path, "nope").err().unwrap().kind(), io::ErrorKind::NotFound);
        // Early drop must not hang or panic.
        let mut r = open_entry(&path, "data/b.sql").unwrap();
        let mut head = [0u8; 10];
        r.read_exact(&mut head).unwrap();
        drop(r);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn filetime_to_date() {
        // 2026-05-09 23:27:50 UTC.
        let unix_secs: u64 = 20_582 * 86_400 + 23 * 3600 + 27 * 60 + 50;
        assert_eq!(filetime_date((unix_secs + 11_644_473_600) * 10_000_000), "2026-05-09");
        assert_eq!(filetime_date(11_644_473_600 * 10_000_000), "1970-01-01");
    }

    /// Real dump: `TOSHO_DUMP=<database-dump.7z> cargo test -p nyaa-client -- --ignored`.
    #[test]
    #[ignore]
    fn real_dump_small_entry() {
        let path = std::env::var("TOSHO_DUMP").expect("TOSHO_DUMP");
        let mut s = String::new();
        open_entry(Path::new(&path), r"schema\toto_repl.sql").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s.len(), 7668);
        assert_eq!(entry_date(Path::new(&path), r"data\toto_repl.sql").unwrap().len(), 10);
    }
}
