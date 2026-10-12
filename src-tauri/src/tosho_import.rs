//! "Import Tosho Data" pipeline (PLAN.md "AnimeTosho dataset" > T0, D3):
//! download only `database-dump.7z` of the 1 TB `at-data.torrent` through
//! sbtl, build the dataset from the archive stream, import it, optionally
//! delete the archive. Phases are resumable: a retry skips whatever an earlier
//! run already produced.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sbtl_engine::{Engine, FileInfo, ReadKind, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncReadExt;

use crate::AppState;

pub const TORRENT_URL: &str = "https://animetosho.org/at-data.torrent";
pub const ARCHIVE_SUFFIX: &str = "database-dump.7z";
pub const ARCHIVE_SIZE: u64 = 2_943_660_826;
/// Disk the dataset takes (500 MB) and the scratch the build and import need.
pub const DATASET_BYTES: u64 = 500_000_000;
pub const SCRATCH_BYTES: u64 = 700_000_000;
const EVENT: &str = "tosho-import-progress";
const MARKER: &str = "archive.ok";
/// No new byte for this long = stalled (longer before the first byte: the
/// resume check and peer discovery on a 1 TB torrent).
const STALL_FIRST: Duration = Duration::from_secs(300);
const STALL_LATER: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Download,
    Build,
    Import,
    Cleanup,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
    /// Bytes/s (download only).
    pub speed: u64,
    pub peers: u32,
    pub message: String,
}

impl Progress {
    fn new(phase: Phase, message: impl Into<String>) -> Self {
        Self { phase, done: 0, total: 0, speed: 0, peers: 0, message: message.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPlan {
    pub download_bytes: u64,
    pub dataset_bytes: u64,
    pub needs_bytes: u64,
    pub free_bytes: u64,
    pub archive_present: bool,
    pub dataset_present: bool,
}

/// Disk the run still needs: the download unless the archive is verified, and
/// the dataset plus build scratch unless a dataset exists.
pub fn needs_bytes(archive_present: bool, dataset_present: bool) -> u64 {
    let download = if archive_present { 0 } else { ARCHIVE_SIZE };
    let build = if dataset_present { 0 } else { DATASET_BYTES + SCRATCH_BYTES };
    download + build
}

/// Index of the archive in the torrent: path-component suffix AND exact size,
/// never an index.
pub fn find_archive(files: &[FileInfo]) -> Option<usize> {
    files.iter().position(|f| {
        let n = f.name.replace('\\', "/");
        (n == ARCHIVE_SUFFIX || n.ends_with(&format!("/{ARCHIVE_SUFFIX}"))) && f.length == ARCHIVE_SIZE
    })
}

/// Watches the download for a stall.
pub struct StallWatch {
    last_bytes: u64,
    last_change: Instant,
}

impl StallWatch {
    pub fn new(now: Instant) -> Self {
        Self { last_bytes: 0, last_change: now }
    }

    /// Feeds the current byte count; true when stalled.
    pub fn stalled(&mut self, now: Instant, downloaded: u64) -> bool {
        if downloaded > self.last_bytes {
            self.last_bytes = downloaded;
            self.last_change = now;
            return false;
        }
        let limit = if self.last_bytes == 0 { STALL_FIRST } else { STALL_LATER };
        now.duration_since(self.last_change) >= limit
    }
}

// ---------- paths and on-disk state ----------

pub fn tosho_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("nyaa-stream").join("tosho")
}

/// The verified archive: the marker written after a complete download names
/// the file, which must still have the exact size.
pub fn verified_archive(dir: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(std::fs::read_to_string(dir.join(MARKER)).ok()?.trim());
    (std::fs::metadata(&path).ok()?.len() == ARCHIVE_SIZE).then_some(path)
}

/// Newest `tosho-dataset-*.db` in `dir`.
pub fn newest_dataset(dir: &Path) -> Option<(PathBuf, std::time::SystemTime)> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("tosho-dataset-") && n.ends_with(".db")
        })
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .max_by_key(|(_, t)| *t)
}

/// A dataset that can be reused: present and not older than the archive.
pub fn reusable_dataset(dir: &Path, archive: Option<&Path>) -> Option<PathBuf> {
    let (path, time) = newest_dataset(dir)?;
    if let Some(archive_time) = archive.and_then(|a| std::fs::metadata(a).ok()?.modified().ok()) {
        if time < archive_time {
            return None;
        }
    }
    Some(path)
}

#[cfg(windows)]
pub fn free_bytes(dir: &Path) -> u64 {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    // The dir may not exist yet: ask about its nearest existing ancestor.
    let mut probe = dir;
    while !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p,
            None => break,
        }
    }
    let wide: Vec<u16> = probe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free = 0u64;
    // SAFETY: `wide` is NUL-terminated; the other out-pointers may be null.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok == 0 {
        tracing::warn!(dir = %dir.display(), "GetDiskFreeSpaceExW failed");
        return u64::MAX;
    }
    free
}

#[cfg(not(windows))]
pub fn free_bytes(_dir: &Path) -> u64 {
    u64::MAX
}

pub fn plan(dir: &Path) -> ImportPlan {
    let archive = verified_archive(dir);
    let dataset = reusable_dataset(dir, archive.as_deref());
    let (archive_present, dataset_present) = (archive.is_some(), dataset.is_some());
    ImportPlan {
        download_bytes: ARCHIVE_SIZE,
        dataset_bytes: DATASET_BYTES,
        needs_bytes: needs_bytes(archive_present, dataset_present),
        free_bytes: free_bytes(dir),
        archive_present,
        dataset_present,
    }
}

// ---------- the phase state machine ----------

/// A failed step: a plain-language message, or a user cancel.
#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    Cancelled,
    Failed(String),
}

pub type Emit<'a> = &'a (dyn Fn(Progress) + Send + Sync);

/// The pipeline's side effects, faked in tests.
#[allow(async_fn_in_trait)]
pub trait Steps {
    fn archive_ready(&self) -> Option<PathBuf>;
    async fn download(&mut self, emit: Emit<'_>) -> Result<PathBuf, StepError>;
    fn dataset_ready(&self, archive: &Path) -> Option<PathBuf>;
    async fn build(&mut self, archive: &Path, emit: Emit<'_>) -> Result<PathBuf, StepError>;
    /// Imports the dataset; returns the counts for the final message.
    async fn import(&mut self, dataset: &Path) -> Result<String, StepError>;
    fn cleanup(&mut self) -> Result<(), StepError>;
}

fn fail(phase: &str, err: StepError, emit: Emit<'_>) {
    match err {
        StepError::Cancelled => {
            tracing::info!(phase, "Tosho import cancelled");
            emit(Progress::new(Phase::Cancelled, "Cancelled. Downloaded data is kept, so the next run resumes."));
        }
        StepError::Failed(msg) => {
            tracing::error!(phase, %msg, "Tosho import failed");
            emit(Progress::new(Phase::Error, format!("{phase} failed: {msg}")));
        }
    }
}

/// Runs download -> build -> import -> cleanup, emitting a progress line at each
/// phase start and exactly one terminal line (done / error / cancelled).
pub async fn run<S: Steps>(steps: &mut S, delete_archive: bool, emit: Emit<'_>) {
    let archive = match steps.archive_ready() {
        Some(a) => {
            tracing::info!(archive = %a.display(), "Tosho import: verified archive found, skipping download");
            a
        }
        None => {
            tracing::info!("Tosho import: download phase");
            emit(Progress::new(Phase::Download, "Connecting to the AnimeTosho torrent..."));
            match steps.download(emit).await {
                Ok(a) => a,
                Err(e) => return fail("Download", e, emit),
            }
        }
    };
    let dataset = match steps.dataset_ready(&archive) {
        Some(d) => {
            tracing::info!(dataset = %d.display(), "Tosho import: dataset found, skipping build");
            d
        }
        None => {
            tracing::info!("Tosho import: build phase");
            emit(Progress::new(Phase::Build, "Building the dataset..."));
            match steps.build(&archive, emit).await {
                Ok(d) => d,
                Err(e) => return fail("Build", e, emit),
            }
        }
    };
    tracing::info!(dataset = %dataset.display(), "Tosho import: import phase");
    emit(Progress::new(Phase::Import, "Importing into the release database..."));
    let counts = match steps.import(&dataset).await {
        Ok(c) => c,
        Err(e) => return fail("Import", e, emit),
    };
    let mut note = String::new();
    if delete_archive {
        tracing::info!("Tosho import: cleanup phase");
        emit(Progress::new(Phase::Cleanup, "Deleting the downloaded archive..."));
        if let Err(e) = steps.cleanup() {
            // The import itself succeeded: report, don't fail.
            tracing::warn!(?e, "Tosho import: cleanup failed");
            note = " (the downloaded archive could not be fully deleted)".into();
        }
    }
    tracing::info!(%counts, "Tosho import done");
    let mut done = Progress::new(Phase::Done, format!("{counts}{note}"));
    done.done = 1;
    done.total = 1;
    emit(done);
}

// ---------- real steps ----------

struct RealSteps {
    dir: PathBuf,
    app: Arc<AppState>,
    cancel: Arc<AtomicBool>,
}

fn cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}

impl Steps for RealSteps {
    fn archive_ready(&self) -> Option<PathBuf> {
        verified_archive(&self.dir)
    }

    async fn download(&mut self, emit: Emit<'_>) -> Result<PathBuf, StepError> {
        download_archive(&self.dir, &self.cancel, emit).await
    }

    fn dataset_ready(&self, archive: &Path) -> Option<PathBuf> {
        reusable_dataset(&self.dir, Some(archive))
    }

    async fn build(&mut self, archive: &Path, emit: Emit<'_>) -> Result<PathBuf, StepError> {
        let (archive, dir, cancel) = (archive.to_path_buf(), self.dir.clone(), self.cancel.clone());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();
        let handle = tokio::task::spawn_blocking(move || {
            nyaa_client::tosho_pipeline::build_from_archive_cancellable(&archive, &dir, &cancel, |done, total| {
                let _ = tx.send((done, total));
            })
        });
        while let Some((done, total)) = rx.recv().await {
            let mut p = Progress::new(Phase::Build, "Building the dataset...");
            p.done = done;
            p.total = total;
            emit(p);
        }
        match handle.await {
            Ok(Ok(path)) => Ok(path),
            Ok(Err(err)) if nyaa_client::tosho_pipeline::is_cancelled(&err) => Err(StepError::Cancelled),
            Ok(Err(err)) => Err(StepError::Failed(format!("the archive could not be read ({err:#})"))),
            Err(err) => Err(StepError::Failed(format!("the build task crashed ({err})"))),
        }
    }

    async fn import(&mut self, dataset: &Path) -> Result<String, StepError> {
        let stats = self.app.nyaa.import_tosho(dataset).await.map_err(|e| StepError::Failed(format!("{e:#}")))?;
        Ok(format!(
            "Imported {} releases ({} new), {} id links, {} file rows, dump of {}.",
            stats.releases.total, stats.releases.added, stats.release_aid, stats.release_files, stats.dump_date
        ))
    }

    fn cleanup(&mut self) -> Result<(), StepError> {
        let mut failed = Vec::new();
        for name in ["Anime Tosho Data", ".sbtl"] {
            let p = self.dir.join(name);
            if p.exists() {
                if let Err(e) = std::fs::remove_dir_all(&p) {
                    failed.push(format!("{}: {e}", p.display()));
                }
            }
        }
        for name in ["at-data.torrent", MARKER] {
            let p = self.dir.join(name);
            if p.exists() {
                if let Err(e) = std::fs::remove_file(&p) {
                    failed.push(format!("{}: {e}", p.display()));
                }
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(StepError::Failed(failed.join("; ")))
        }
    }
}

async fn fetch_torrent(dir: &Path) -> Result<Vec<u8>, StepError> {
    let path = dir.join("at-data.torrent");
    if let Ok(bytes) = std::fs::read(&path) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    tracing::info!(url = TORRENT_URL, "fetching at-data.torrent");
    let bytes = async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(90)).build()?;
        client.get(TORRENT_URL).send().await?.error_for_status()?.bytes().await
    }
    .await
    .map_err(|e| StepError::Failed(format!("could not fetch the AnimeTosho torrent file ({e}); check the connection")))?;
    std::fs::write(&path, &bytes).map_err(|e| StepError::Failed(format!("could not save the torrent file ({e})")))?;
    Ok(bytes.to_vec())
}

async fn download_archive(dir: &Path, cancel: &AtomicBool, emit: Emit<'_>) -> Result<PathBuf, StepError> {
    std::fs::create_dir_all(dir).map_err(|e| StepError::Failed(format!("could not create the download folder ({e})")))?;
    let torrent_bytes = fetch_torrent(dir).await?;
    if cancelled(cancel) {
        return Err(StepError::Cancelled);
    }
    let engine = Engine::start(dir.to_path_buf()).map_err(|e| StepError::Failed(format!("could not start the torrent engine ({e:#})")))?;
    let torrent = engine.add(Source::Bytes(torrent_bytes)).await.map_err(|e| StepError::Failed(format!("the torrent file is not usable ({e:#})")))?;
    let files = torrent.files();
    let idx = find_archive(&files).ok_or_else(|| {
        tracing::error!(files = files.len(), "database-dump.7z with the expected size is not in the torrent");
        StepError::Failed("the torrent no longer contains database-dump.7z with the expected size (AnimeTosho may have changed it)".into())
    })?;
    let hash = torrent.info_hash().to_string();
    tracing::info!(idx, name = %files[idx].name, "Tosho download: archive located");

    // Background reader: only its own sequential window downloads. Foreground
    // or preload would also queue pieces of the other 14,000 files.
    let mut reader = torrent.open(idx, 0, ReadKind::Background).map_err(|e| StepError::Failed(format!("could not open the archive in the torrent ({e:#})")))?;
    let mut buf = vec![0u8; 1 << 20];
    let mut watch = StallWatch::new(Instant::now());
    let mut last_tick = Instant::now() - Duration::from_secs(2);
    let mut eof = false;
    let result = loop {
        if cancelled(cancel) {
            break Err(StepError::Cancelled);
        }
        match tokio::time::timeout(Duration::from_secs(1), reader.read(&mut buf)).await {
            Ok(Ok(0)) => eof = true,
            Ok(Ok(_)) => {}
            Ok(Err(e)) => break Err(StepError::Failed(format!("the download failed ({e})"))),
            Err(_) => {}
        }
        if eof || last_tick.elapsed() >= Duration::from_secs(1) {
            last_tick = Instant::now();
            let st = torrent.stats();
            let file = &st.files[idx];
            let mut p = Progress::new(Phase::Download, "Downloading database-dump.7z...");
            p.done = file.downloaded;
            p.total = ARCHIVE_SIZE;
            p.speed = st.download_rate.max(0.0) as u64;
            p.peers = st.peers as u32;
            emit(p);
            if eof {
                break if file.downloaded >= file.length { Ok(()) } else { Err(StepError::Failed("the torrent ended before the archive was complete".into())) };
            }
            if watch.stalled(Instant::now(), file.downloaded) {
                tracing::error!(peers = st.peers, known = st.peers_known, "Tosho download stalled");
                break Err(StepError::Failed(if st.peers == 0 {
                    "no peers found for the AnimeTosho torrent; try again later".into()
                } else {
                    "the download stalled; try again later".into()
                }));
            }
        }
    };
    drop(reader);
    let path = torrent.file_path(idx);
    drop(torrent);
    engine.remove(&hash);
    result?;
    let path = path.ok_or_else(|| StepError::Failed("the downloaded archive file is missing".into()))?;
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if len != ARCHIVE_SIZE {
        return Err(StepError::Failed(format!("the downloaded archive has the wrong size ({len} of {ARCHIVE_SIZE} bytes)")));
    }
    std::fs::write(dir.join(MARKER), path.to_string_lossy().as_bytes()).map_err(|e| StepError::Failed(format!("could not record the finished download ({e})")))?;
    tracing::info!(path = %path.display(), "Tosho download complete");
    Ok(path)
}

// ---------- Tauri glue ----------

#[derive(Default)]
pub struct ImportState {
    /// Cancel flag of the running import.
    running: Mutex<Option<Arc<AtomicBool>>>,
    last: Mutex<Option<Progress>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[tauri::command]
pub async fn tosho_import_plan() -> Result<ImportPlan, String> {
    let dir = tosho_dir();
    tokio::task::spawn_blocking(move || plan(&dir)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tosho_import_start(app: AppHandle, state: State<'_, ImportState>, delete_archive: bool) -> Result<(), String> {
    tracing::info!(delete_archive, "tosho_import_start invoked");
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = lock(&state.running);
        if running.is_some() {
            return Err("An AnimeTosho import is already running.".into());
        }
        *running = Some(cancel.clone());
    }
    *lock(&state.last) = None;
    let app_state: Arc<AppState> = app.state::<Arc<AppState>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
        let handle = app.clone();
        let emit = move |p: Progress| {
            let terminal = matches!(p.phase, Phase::Done | Phase::Error | Phase::Cancelled);
            let mut last = lock(&last_emit);
            // Phase-start lines (done == 0) always go out; the rest at most 2x/s.
            if !terminal && p.done != 0 && last.elapsed() < Duration::from_millis(500) {
                return;
            }
            *last = Instant::now();
            drop(last);
            *lock(&handle.state::<ImportState>().last) = Some(p.clone());
            let _ = handle.emit(EVENT, p);
        };
        let mut steps = RealSteps { dir: tosho_dir(), app: app_state, cancel };
        run(&mut steps, delete_archive, &emit).await;
        *lock(&app.state::<ImportState>().running) = None;
    });
    Ok(())
}

#[tauri::command]
pub fn tosho_import_cancel(state: State<'_, ImportState>) {
    tracing::info!("tosho_import_cancel invoked");
    if let Some(flag) = lock(&state.running).as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn tosho_import_status(state: State<'_, ImportState>) -> Option<Progress> {
    lock(&state.last).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fi(name: &str, length: u64) -> FileInfo {
        FileInfo { name: name.into(), length }
    }

    #[test]
    fn finds_archive_by_suffix_and_size() {
        let files = vec![
            fi(r"Anime Tosho Data\nzb\database-dump.7z", 5),
            fi(r"Anime Tosho Data\xdatabase-dump.7z", ARCHIVE_SIZE),
            fi(r"Anime Tosho Data\database-dump.7z", ARCHIVE_SIZE),
        ];
        assert_eq!(find_archive(&files), Some(2));
        assert_eq!(find_archive(&[fi("database-dump.7z", ARCHIVE_SIZE)]), Some(0));
        assert_eq!(find_archive(&[fi("a/database-dump.7z", ARCHIVE_SIZE - 1)]), None);
    }

    #[test]
    fn needs_math() {
        assert_eq!(needs_bytes(false, false), ARCHIVE_SIZE + DATASET_BYTES + SCRATCH_BYTES);
        assert_eq!(needs_bytes(true, false), DATASET_BYTES + SCRATCH_BYTES);
        assert_eq!(needs_bytes(true, true), 0);
        assert_eq!(needs_bytes(false, true), ARCHIVE_SIZE);
    }

    #[test]
    fn stall_watch() {
        let t0 = Instant::now();
        let mut w = StallWatch::new(t0);
        assert!(!w.stalled(t0 + Duration::from_secs(299), 0));
        assert!(w.stalled(t0 + Duration::from_secs(301), 0));
        assert!(!w.stalled(t0 + Duration::from_secs(302), 10));
        assert!(!w.stalled(t0 + Duration::from_secs(400), 10));
        assert!(w.stalled(t0 + Duration::from_secs(483), 10));
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tosho_import_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn verified_archive_and_dataset_reuse() {
        let dir = scratch("verify");
        assert!(verified_archive(&dir).is_none());
        let f = dir.join("a.7z");
        std::fs::write(&f, b"short").unwrap();
        std::fs::write(dir.join(MARKER), f.to_string_lossy().as_bytes()).unwrap();
        assert!(verified_archive(&dir).is_none(), "wrong size must not verify");
        assert!(reusable_dataset(&dir, Some(&f)).is_none());
        std::fs::write(dir.join("tosho-dataset-20260509.db"), b"x").unwrap();
        assert!(reusable_dataset(&dir, Some(&f)).is_some());
        assert!(reusable_dataset(&dir, None).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn free_space_is_reported() {
        assert!(free_bytes(&std::env::temp_dir().join("does").join("not").join("exist")) > 0);
    }

    #[derive(Default)]
    struct Fake {
        archive: bool,
        dataset: bool,
        fail_at: Option<&'static str>,
        cancel_at: Option<&'static str>,
        calls: Vec<&'static str>,
    }

    impl Fake {
        fn outcome(&self, step: &'static str) -> Result<(), StepError> {
            if self.cancel_at == Some(step) {
                return Err(StepError::Cancelled);
            }
            if self.fail_at == Some(step) {
                return Err(StepError::Failed(format!("{step} broke")));
            }
            Ok(())
        }
    }

    impl Steps for Fake {
        fn archive_ready(&self) -> Option<PathBuf> {
            self.archive.then(|| PathBuf::from("a.7z"))
        }
        async fn download(&mut self, _: Emit<'_>) -> Result<PathBuf, StepError> {
            self.calls.push("download");
            self.outcome("download").map(|_| PathBuf::from("a.7z"))
        }
        fn dataset_ready(&self, _: &Path) -> Option<PathBuf> {
            self.dataset.then(|| PathBuf::from("d.db"))
        }
        async fn build(&mut self, _: &Path, _: Emit<'_>) -> Result<PathBuf, StepError> {
            self.calls.push("build");
            self.outcome("build").map(|_| PathBuf::from("d.db"))
        }
        async fn import(&mut self, _: &Path) -> Result<String, StepError> {
            self.calls.push("import");
            self.outcome("import").map(|_| "1 releases".into())
        }
        fn cleanup(&mut self) -> Result<(), StepError> {
            self.calls.push("cleanup");
            self.outcome("cleanup")
        }
    }

    async fn drive(mut fake: Fake, delete: bool) -> (Fake, Vec<Phase>, String) {
        let seen = Mutex::new(Vec::new());
        let last = Mutex::new(String::new());
        run(&mut fake, delete, &|p: Progress| {
            *last.lock().unwrap() = p.message.clone();
            seen.lock().unwrap().push(p.phase);
        })
        .await;
        let phases = seen.into_inner().unwrap();
        (fake, phases, last.into_inner().unwrap())
    }

    #[tokio::test]
    async fn full_run_in_order() {
        let (f, phases, msg) = drive(Fake::default(), true).await;
        assert_eq!(f.calls, ["download", "build", "import", "cleanup"]);
        assert_eq!(phases, [Phase::Download, Phase::Build, Phase::Import, Phase::Cleanup, Phase::Done]);
        assert_eq!(msg, "1 releases");
    }

    #[tokio::test]
    async fn resumes_and_keeps_archive() {
        let (f, phases, _) = drive(Fake { archive: true, dataset: true, ..Default::default() }, false).await;
        assert_eq!(f.calls, ["import"]);
        assert_eq!(phases, [Phase::Import, Phase::Done]);
    }

    #[tokio::test]
    async fn failure_stops_and_reports_once() {
        let (f, phases, msg) = drive(Fake { fail_at: Some("build"), ..Default::default() }, true).await;
        assert_eq!(f.calls, ["download", "build"]);
        assert_eq!(*phases.last().unwrap(), Phase::Error);
        assert!(msg.starts_with("Build failed"));
        assert_eq!(phases.iter().filter(|p| matches!(p, Phase::Error | Phase::Done)).count(), 1);
    }

    #[tokio::test]
    async fn cancel_is_distinct_and_cleanup_failure_is_soft() {
        let (_, phases, _) = drive(Fake { cancel_at: Some("download"), ..Default::default() }, true).await;
        assert_eq!(*phases.last().unwrap(), Phase::Cancelled);
        let (_, phases, msg) = drive(Fake { fail_at: Some("cleanup"), ..Default::default() }, true).await;
        assert_eq!(*phases.last().unwrap(), Phase::Done);
        assert!(msg.contains("could not be fully deleted"));
    }
}
