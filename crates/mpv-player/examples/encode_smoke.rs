//! Scratch tool (`cargo run -p mpv-player --example encode_smoke -- <libmpv-2.dll> <video> <out.mp4> [subtitle-id]`):
//! encodes seconds 1-4 of a video with libmpv's encode mode, optionally with
//! subtitle track `id` burned in.

use mpv_player::{encode_clip, set_library_path, EncodeClip};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("mpv_player=debug").init();
    let mut args = std::env::args().skip(1);
    set_library_path(Some(args.next().expect("libmpv path").into()));
    let input = args.next().expect("input video");
    let out = args.next().expect("output path");
    let subtitle_id = args.next().and_then(|id| id.parse().ok());
    encode_clip(EncodeClip { input, out: out.into(), start_seconds: 1.0, end_seconds: 4.0, audio_id: None, subtitle_id, options: Vec::new() }).await?;
    println!("done");
    Ok(())
}
