//! Scratch tool (`cargo run -p mpv-player --example smoke -- <libmpv-2.dll> [media-file]`):
//! loads libmpv, round-trips properties (node conversion both ways) and prints
//! the events a load produces, without launching the full app.

use serde_json::{json, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("mpv_player=debug").init();
    let mut args = std::env::args().skip(1);
    if let Some(dll) = args.next() {
        mpv_player::set_library_path(Some(dll.into()));
    }
    println!("available: {}", mpv_player::is_available());

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let mpv = mpv_player::Mpv::new()?;
    for option in ["--vo=null", "--ao=null", "--idle=yes", "--keep-open=yes", "--no-config"] {
        let (name, value) = mpv_player::split_option(option);
        mpv.set_option(&name, &value)?;
    }
    mpv.initialize()?;
    mpv.start_events(tx);

    println!("version: {}", mpv.command(&[json!("get_property"), json!("mpv-version")])?);
    mpv.command(&[json!("set_property"), json!("volume"), json!(50)])?;
    println!("volume: {}", mpv.command(&[json!("get_property"), json!("volume")])?);
    mpv.command(&[json!("set_property"), json!("sid"), json!("no")])?;
    println!("sid: {}", mpv.command(&[json!("get_property"), json!("sid")])?);
    for (i, name) in ["time-pos", "duration", "pause", "track-list", "video-params"].iter().enumerate() {
        mpv.command(&[json!("observe_property"), json!(i + 1), json!(name)])?;
    }
    let file = args.next().unwrap_or_else(|| "does-not-exist.mkv".into());
    mpv.command(&[json!("loadfile"), json!(file), json!("replace")])?;

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(4);
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        println!("event: {event}");
    }
    Ok(())
}
