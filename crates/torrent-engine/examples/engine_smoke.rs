//! Smoke test of the streaming path mpv uses: TorrentEngine's loopback
//! HTTP server (`/stream/<id>/<file>`) over whichever backend this build
//! uses (libtorrent by default, tl with `--no-default-features --features
//! tl`). Adds a torrent, then issues Range GETs like a player: the head,
//! the tail (container index), the middle, and a few random seeks. Every
//! response is timed and, with REF_FILE, compared byte for byte.
//!
//!   cargo run -p torrent-engine --example engine_smoke [--no-default-features --features tl] -- \
//!       MAGNET_OR_URL [REF_FILE] [--dir DOWNLOAD_DIR]

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Instant;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// One HTTP/1.1 Range GET on a fresh connection; returns the body.
async fn get_range(url: &str, start: u64, end_incl: u64) -> anyhow::Result<Vec<u8>> {
    let rest = url.strip_prefix("http://").ok_or_else(|| anyhow::anyhow!("http only"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut sock = tokio::net::TcpStream::connect(host).await?;
    let req = format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\nRange: bytes={start}-{end_incl}\r\nConnection: close\r\n\r\n");
    sock.write_all(req.as_bytes()).await?;
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).await?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| anyhow::anyhow!("no header end"))?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status = head.lines().next().unwrap_or("");
    anyhow::ensure!(status.contains(" 206 ") || status.contains(" 200 "), "unexpected status: {status}");
    Ok(raw[split + 4..].to_vec())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.iter().position(|a| a == "--dir") {
        Some(i) => {
            let d = PathBuf::from(args.remove(i + 1));
            args.remove(i);
            d
        }
        None => std::env::temp_dir().join("engine-smoke"),
    };
    let source = args.first().cloned().ok_or_else(|| anyhow::anyhow!("usage: engine_smoke MAGNET_OR_URL [REF_FILE] [--dir DIR]"))?;
    let mut reference = args.get(1).map(std::fs::File::open).transpose()?;

    let t0 = Instant::now();
    let engine = torrent_engine::TorrentEngine::start(dir.join("downloads")).await?;
    let added = engine.add(&source).await?;
    let files = engine.files(&added.id).await?;
    let (idx, file) = files.iter().enumerate().max_by_key(|(_, f)| f.length).unwrap();
    let len = file.length;
    println!("metadata in {:.2}s; file {idx}: {} ({len} bytes)", t0.elapsed().as_secs_f64(), file.name);
    let url = engine.stream_url(&added.id, idx);

    let mut plan: Vec<(&str, u64, u64)> = vec![
        ("head", 0, 1 << 20),
        ("tail", len.saturating_sub(2 << 20), 2 << 20),
        ("middle", len / 2, 4 << 20),
    ];
    let mut rng = 0x9E3779B97F4A7C15u64;
    for _ in 0..3 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        plan.push(("seek", rng % len, 2 << 20));
    }

    let mut mismatches = 0;
    for (what, off, n) in plan {
        let n = n.min(len - off);
        let t = Instant::now();
        let body = get_range(&url, off, off + n - 1).await?;
        let secs = t.elapsed().as_secs_f64();
        let ok = match &mut reference {
            Some(r) => {
                let mut want = vec![0u8; body.len()];
                r.seek(SeekFrom::Start(off))?;
                r.read_exact(&mut want)?;
                want == body
            }
            None => true,
        };
        mismatches += (!ok) as u32;
        println!(
            "{what:>6} @ {off:>11}: {:>5.2} MiB in {secs:>5.2}s{}",
            body.len() as f64 / 1048576.0,
            if ok { "" } else { "  MISMATCH" }
        );
        anyhow::ensure!(body.len() as u64 == n, "short body: {} of {n}", body.len());
    }
    println!("RESULT mismatches={mismatches} total={:.1}s", t0.elapsed().as_secs_f64());
    anyhow::ensure!(mismatches == 0, "byte mismatches");
    Ok(())
}
