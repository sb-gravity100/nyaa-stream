//! In-process libmpv over its C client API, loaded at runtime from
//! `libmpv-2.dll` (so the app still starts - and falls back to HLS - when the
//! DLL is missing). `Mpv::command` speaks the same JSON argument arrays mpv's
//! JSON IPC does (`["set_property", "pause", true]`, `["observe_property", 1,
//! "time-pos"]`...) and events come out shaped like IPC event lines, so the
//! frontend's `mpvVideo.ts` contract is unchanged from the old spawned-mpv
//! design.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Map, Number, Value};
use tokio::sync::mpsc;

const MPV_FORMAT_NONE: c_int = 0;
const MPV_FORMAT_STRING: c_int = 1;
const MPV_FORMAT_OSD_STRING: c_int = 2;
const MPV_FORMAT_FLAG: c_int = 3;
const MPV_FORMAT_INT64: c_int = 4;
const MPV_FORMAT_DOUBLE: c_int = 5;
const MPV_FORMAT_NODE: c_int = 6;
const MPV_FORMAT_NODE_ARRAY: c_int = 7;
const MPV_FORMAT_NODE_MAP: c_int = 8;

const MPV_EVENT_SHUTDOWN: c_int = 1;
const MPV_EVENT_END_FILE: c_int = 7;
const MPV_EVENT_PROPERTY_CHANGE: c_int = 22;

const DLL_NAME: &str = "libmpv-2.dll";

#[repr(C)]
#[derive(Clone, Copy)]
union NodeValue {
    string: *mut c_char,
    flag: c_int,
    int64: i64,
    double: f64,
    list: *mut NodeList,
    byte_array: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MpvNode {
    value: NodeValue,
    format: c_int,
}

#[repr(C)]
struct NodeList {
    num: c_int,
    values: *mut MpvNode,
    keys: *mut *mut c_char,
}

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct EventProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct EventEndFile {
    reason: c_int,
    error: c_int,
}

type CreateFn = unsafe extern "C" fn() -> *mut c_void;
type InitializeFn = unsafe extern "C" fn(*mut c_void) -> c_int;
type TerminateDestroyFn = unsafe extern "C" fn(*mut c_void);
type SetOptionStringFn = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type CommandNodeFn = unsafe extern "C" fn(*mut c_void, *mut MpvNode, *mut MpvNode) -> c_int;
type PropertyFn = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type ObservePropertyFn = unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int;
type WaitEventFn = unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent;
type WakeupFn = unsafe extern "C" fn(*mut c_void);
type FreeNodeContentsFn = unsafe extern "C" fn(*mut MpvNode);
type NameFn = unsafe extern "C" fn(c_int) -> *const c_char;

/// The resolved client-API entry points. `_library` keeps the DLL mapped for
/// as long as any function pointer (or `Mpv`) is alive.
pub struct Lib {
    _library: libloading::Library,
    path: PathBuf,
    create: CreateFn,
    initialize: InitializeFn,
    terminate_destroy: TerminateDestroyFn,
    set_option_string: SetOptionStringFn,
    command_node: CommandNodeFn,
    set_property: PropertyFn,
    get_property: PropertyFn,
    observe_property: ObservePropertyFn,
    wait_event: WaitEventFn,
    wakeup: WakeupFn,
    free_node_contents: FreeNodeContentsFn,
    error_string: NameFn,
    event_name: NameFn,
}

impl Lib {
    fn open(path: &Path) -> anyhow::Result<Self> {
        tracing::debug!(path = %path.display(), "loading libmpv");
        #[cfg(windows)]
        let library: libloading::Library = {
            // LOAD_WITH_ALTERED_SEARCH_PATH: resolve the DLL's own
            // dependencies next to it, not only beside the exe.
            const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
            unsafe { libloading::os::windows::Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }
                .map(Into::into)
                .with_context(|| format!("loading {}", path.display()))?
        };
        #[cfg(not(windows))]
        let library = unsafe { libloading::Library::new(path) }.with_context(|| format!("loading {}", path.display()))?;

        macro_rules! symbol {
            ($name:literal) => {
                *unsafe { library.get(concat!($name, "\0").as_bytes()) }.with_context(|| format!("libmpv is missing {}", $name))?
            };
        }
        Ok(Self {
            create: symbol!("mpv_create"),
            initialize: symbol!("mpv_initialize"),
            terminate_destroy: symbol!("mpv_terminate_destroy"),
            set_option_string: symbol!("mpv_set_option_string"),
            command_node: symbol!("mpv_command_node"),
            set_property: symbol!("mpv_set_property"),
            get_property: symbol!("mpv_get_property"),
            observe_property: symbol!("mpv_observe_property"),
            wait_event: symbol!("mpv_wait_event"),
            wakeup: symbol!("mpv_wakeup"),
            free_node_contents: symbol!("mpv_free_node_contents"),
            error_string: symbol!("mpv_error_string"),
            event_name: symbol!("mpv_event_name"),
            path: path.to_path_buf(),
            _library: library,
        })
    }

    fn error_message(&self, code: c_int) -> String {
        // SAFETY: mpv_error_string returns a static NUL-terminated string.
        unsafe { cstr((self.error_string)(code)) }.unwrap_or_else(|| format!("error {code}"))
    }

    fn check(&self, code: c_int) -> anyhow::Result<()> {
        if code < 0 {
            bail!("{}", self.error_message(code));
        }
        Ok(())
    }
}

/// Reads a borrowed C string; `None` for null.
///
/// # Safety
/// `ptr` must be null or point to a NUL-terminated string.
unsafe fn cstr(ptr: *const c_char) -> Option<String> {
    (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

static OVERRIDE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static LIBRARY: Mutex<Option<Arc<Lib>>> = Mutex::new(None);

/// Overrides where `libmpv-2.dll` is loaded from (a file, or the folder that
/// holds it; `None`: the default search) - the user's Settings choice. Players
/// already running keep the library they loaded.
pub fn set_library_path(path: Option<PathBuf>) {
    tracing::info!(?path, "libmpv path override set");
    *OVERRIDE_PATH.lock().unwrap() = path;
    *LIBRARY.lock().unwrap() = None;
}

/// Where `libmpv-2.dll` may live, best first.
fn candidate_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = OVERRIDE_PATH.lock().unwrap().clone() {
        candidates.push(if path.is_dir() { path.join(DLL_NAME) } else { path });
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
        candidates.push(dir.join(DLL_NAME));
        candidates.push(dir.join("lib").join(DLL_NAME));
        candidates.push(dir.join("resources").join("lib").join(DLL_NAME));
    }
    // Bare name last: the OS search path (PATH etc.).
    candidates.push(PathBuf::from(DLL_NAME));
    candidates
}

/// The loaded libmpv (cached), or why it couldn't be found.
pub fn library() -> anyhow::Result<Arc<Lib>> {
    let mut cached = LIBRARY.lock().unwrap();
    if let Some(lib) = cached.as_ref() {
        return Ok(lib.clone());
    }
    let mut last_error = anyhow!("no libmpv candidates");
    for path in candidate_paths() {
        // Full paths that don't exist are skipped without noise; the bare
        // name is always tried.
        if path.components().count() > 1 && !path.exists() {
            continue;
        }
        match Lib::open(&path) {
            Ok(lib) => {
                tracing::info!(path = %lib.path.display(), "libmpv loaded");
                let lib = Arc::new(lib);
                *cached = Some(lib.clone());
                return Ok(lib);
            }
            Err(err) => {
                tracing::debug!(path = %path.display(), %err, "libmpv candidate failed");
                last_error = err;
            }
        }
    }
    tracing::warn!(%last_error, "libmpv not found");
    Err(last_error.context(format!("{DLL_NAME} not found (put it next to the app or set its path in Settings)")))
}

/// Whether libmpv can be loaded - the player falls back to HLS without it.
pub fn is_available() -> bool {
    library().is_ok()
}

#[derive(Clone, Copy)]
struct Handle(*mut c_void);
// SAFETY: the mpv client API is thread-safe: any thread may call it on a handle.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

/// One libmpv instance. Configure with `set_option`, then `initialize` and
/// (for events) `start_events`; dropping it stops the event thread and
/// destroys the player.
pub struct Mpv {
    lib: Arc<Lib>,
    handle: Handle,
    stop: Arc<AtomicBool>,
    events: Mutex<Option<JoinHandle<()>>>,
}

impl Mpv {
    pub fn new() -> anyhow::Result<Self> {
        let lib = library()?;
        // SAFETY: plain constructor call, no preconditions.
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            bail!("mpv_create failed");
        }
        tracing::debug!("libmpv instance created");
        Ok(Self { lib, handle: Handle(handle), stop: Arc::default(), events: Mutex::new(None) })
    }

    /// Sets a pre-initialize option (`--name=value` on mpv's command line).
    pub fn set_option(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let (n, v) = (CString::new(name)?, CString::new(value)?);
        // SAFETY: valid handle and NUL-terminated strings.
        let code = unsafe { (self.lib.set_option_string)(self.handle.0, n.as_ptr(), v.as_ptr()) };
        self.lib.check(code).with_context(|| format!("option {name}={value}"))
    }

    pub fn initialize(&self) -> anyhow::Result<()> {
        // SAFETY: valid handle.
        let code = unsafe { (self.lib.initialize)(self.handle.0) };
        self.lib.check(code).context("mpv_initialize")
    }

    /// Forwards every mpv event to `sender` as an IPC-shaped JSON object from
    /// a dedicated thread. The channel closing means mpv shut down.
    pub fn start_events(&self, sender: mpsc::UnboundedSender<Value>) {
        let (lib, handle, stop) = (self.lib.clone(), self.handle, self.stop.clone());
        let thread = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop(&lib, handle, &stop, sender))
            .expect("spawning the mpv event thread");
        *self.events.lock().unwrap() = Some(thread);
    }

    /// Runs one IPC-style command: `observe_property`, `set_property`,
    /// `get_property`, or any other mpv command (`loadfile`, `seek`...).
    /// Returns the reply's `data`.
    pub fn command(&self, args: &[Value]) -> anyhow::Result<Value> {
        let name = args.first().and_then(Value::as_str).unwrap_or_default();
        match (name, args) {
            ("observe_property", [_, Value::Number(id), Value::String(property)]) => {
                let id = id.as_u64().ok_or_else(|| anyhow!("observe_property id must be a non-negative integer"))?;
                let property = CString::new(property.as_str())?;
                // SAFETY: valid handle and property string.
                let code = unsafe { (self.lib.observe_property)(self.handle.0, id, property.as_ptr(), MPV_FORMAT_NODE) };
                self.lib.check(code)?;
                Ok(Value::Null)
            }
            ("get_property", [_, Value::String(property)]) => self.get_property(property),
            ("set_property", [_, Value::String(property), value]) => self.set_property(property, value),
            _ => self.command_node(args),
        }
    }

    fn get_property(&self, property: &str) -> anyhow::Result<Value> {
        let property = CString::new(property)?;
        let mut node = MpvNode { value: NodeValue { int64: 0 }, format: MPV_FORMAT_NONE };
        // SAFETY: valid handle; `node` is a writable mpv_node.
        let code = unsafe { (self.lib.get_property)(self.handle.0, property.as_ptr(), MPV_FORMAT_NODE, &mut node as *mut _ as *mut c_void) };
        self.lib.check(code)?;
        // SAFETY: mpv filled `node`; it is freed right after the copy.
        let value = unsafe { node_to_json(&node) };
        unsafe { (self.lib.free_node_contents)(&mut node) };
        Ok(value)
    }

    fn set_property(&self, property: &str, value: &Value) -> anyhow::Result<Value> {
        let property = CString::new(property)?;
        let mut arena = Arena::default();
        let mut node = arena.build(value);
        // SAFETY: valid handle; `node` (and everything it points into) lives in `arena`.
        let code = unsafe { (self.lib.set_property)(self.handle.0, property.as_ptr(), MPV_FORMAT_NODE, &mut node as *mut _ as *mut c_void) };
        self.lib.check(code)?;
        Ok(Value::Null)
    }

    fn command_node(&self, args: &[Value]) -> anyhow::Result<Value> {
        let mut arena = Arena::default();
        let mut command = arena.build(&Value::Array(args.to_vec()));
        let mut result = MpvNode { value: NodeValue { int64: 0 }, format: MPV_FORMAT_NONE };
        // SAFETY: valid handle; `command` lives in `arena`, `result` is writable.
        let code = unsafe { (self.lib.command_node)(self.handle.0, &mut command, &mut result) };
        self.lib.check(code)?;
        // SAFETY: mpv filled `result`; it is freed right after the copy.
        let value = unsafe { node_to_json(&result) };
        unsafe { (self.lib.free_node_contents)(&mut result) };
        Ok(value)
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        tracing::debug!("destroying libmpv instance");
        self.stop.store(true, Ordering::Relaxed);
        // SAFETY: valid handle; wakes the event thread out of its wait.
        unsafe { (self.lib.wakeup)(self.handle.0) };
        if let Some(thread) = self.events.lock().unwrap().take() {
            let _ = thread.join();
        }
        // SAFETY: the event thread is gone, so nothing else uses the handle.
        unsafe { (self.lib.terminate_destroy)(self.handle.0) };
        tracing::info!("libmpv instance destroyed");
    }
}

fn event_loop(lib: &Lib, handle: Handle, stop: &AtomicBool, sender: mpsc::UnboundedSender<Value>) {
    tracing::debug!("mpv event thread started");
    while !stop.load(Ordering::Relaxed) {
        // SAFETY: valid handle; the event is only read before the next wait.
        let event = unsafe { &*(lib.wait_event)(handle.0, 1.0) };
        let event_id = event.event_id;
        if event_id == 0 {
            continue;
        }
        if event_id == MPV_EVENT_SHUTDOWN {
            tracing::info!("mpv shut down");
            break;
        }
        // SAFETY: `event` came from mpv_wait_event just above.
        let Some(message) = (unsafe { event_to_json(lib, event) }) else { continue };
        if sender.send(message).is_err() {
            // Nobody listens any more (headless captures never read events);
            // keep draining so mpv's queue doesn't overflow.
            continue;
        }
    }
    tracing::debug!("mpv event thread ended");
}

/// The event as an IPC-style JSON line (`{"event": "...", ...}`), or `None`
/// for events the app doesn't use.
///
/// # Safety
/// `event` must be a live event from `mpv_wait_event`.
unsafe fn event_to_json(lib: &Lib, event: &MpvEvent) -> Option<Value> {
    let name = cstr((lib.event_name)(event.event_id))?;
    let mut message = Map::new();
    message.insert("event".into(), Value::String(name));
    match event.event_id {
        MPV_EVENT_PROPERTY_CHANGE => {
            let property = &*(event.data as *const EventProperty);
            message.insert("id".into(), json!(event.reply_userdata));
            message.insert("name".into(), Value::String(cstr(property.name)?));
            // Unavailable properties carry no data (the IPC omits `data` too).
            if property.format == MPV_FORMAT_NODE && !property.data.is_null() {
                message.insert("data".into(), node_to_json(&*(property.data as *const MpvNode)));
            }
        }
        MPV_EVENT_END_FILE => {
            let end = &*(event.data as *const EventEndFile);
            let reason = match end.reason {
                0 => "eof",
                2 => "stop",
                3 => "quit",
                4 => "error",
                5 => "redirect",
                _ => "unknown",
            };
            message.insert("reason".into(), json!(reason));
            if end.reason == 4 {
                message.insert("file_error".into(), Value::String(lib.error_message(end.error)));
            }
        }
        _ => {}
    }
    Some(Value::Object(message))
}

/// Copies an mpv node tree into JSON.
///
/// # Safety
/// `node` must be a valid, fully initialised mpv node.
unsafe fn node_to_json(node: &MpvNode) -> Value {
    match node.format {
        MPV_FORMAT_STRING | MPV_FORMAT_OSD_STRING => {
            cstr(node.value.string as *const c_char).map(Value::String).unwrap_or(Value::Null)
        }
        MPV_FORMAT_FLAG => Value::Bool(node.value.flag != 0),
        MPV_FORMAT_INT64 => Value::Number(node.value.int64.into()),
        MPV_FORMAT_DOUBLE => Number::from_f64(node.value.double).map(Value::Number).unwrap_or(Value::Null),
        MPV_FORMAT_NODE_ARRAY => {
            let list = &*node.value.list;
            Value::Array((0..list.num as usize).map(|i| node_to_json(&*list.values.add(i))).collect())
        }
        MPV_FORMAT_NODE_MAP => {
            let list = &*node.value.list;
            let mut map = Map::new();
            for i in 0..list.num as usize {
                if let Some(key) = cstr(*list.keys.add(i) as *const c_char) {
                    map.insert(key, node_to_json(&*list.values.add(i)));
                }
            }
            Value::Object(map)
        }
        _ => Value::Null,
    }
}

/// Owns the memory behind an `MpvNode` tree built from JSON, so mpv can read
/// it for the duration of one call. Every allocation is boxed/heap-backed, so
/// the raw pointers inside stay valid while the arena is alive.
#[derive(Default)]
struct Arena {
    strings: Vec<CString>,
    lists: Vec<Box<NodeList>>,
    values: Vec<Vec<MpvNode>>,
    keys: Vec<Vec<*mut c_char>>,
}

impl Arena {
    fn string(&mut self, text: &str) -> *mut c_char {
        // Interior NULs can't cross the C boundary; drop them.
        let cleaned: Vec<u8> = text.bytes().filter(|b| *b != 0).collect();
        let text = CString::new(cleaned).expect("NULs were filtered out");
        let ptr = text.as_ptr() as *mut c_char;
        self.strings.push(text);
        ptr
    }

    fn build(&mut self, value: &Value) -> MpvNode {
        match value {
            Value::Null => MpvNode { value: NodeValue { int64: 0 }, format: MPV_FORMAT_NONE },
            Value::Bool(flag) => MpvNode { value: NodeValue { flag: *flag as c_int }, format: MPV_FORMAT_FLAG },
            Value::Number(number) => match number.as_i64() {
                Some(int64) => MpvNode { value: NodeValue { int64 }, format: MPV_FORMAT_INT64 },
                None => MpvNode { value: NodeValue { double: number.as_f64().unwrap_or(0.0) }, format: MPV_FORMAT_DOUBLE },
            },
            Value::String(text) => MpvNode { value: NodeValue { string: self.string(text) }, format: MPV_FORMAT_STRING },
            Value::Array(items) => {
                let mut children: Vec<MpvNode> = items.iter().map(|item| self.build(item)).collect();
                let list = self.list(&mut children, std::ptr::null_mut());
                self.values.push(children);
                MpvNode { value: NodeValue { list }, format: MPV_FORMAT_NODE_ARRAY }
            }
            Value::Object(entries) => {
                let mut keys: Vec<*mut c_char> = entries.keys().map(|key| self.string(key)).collect();
                let mut children: Vec<MpvNode> = entries.values().map(|item| self.build(item)).collect();
                let list = self.list(&mut children, keys.as_mut_ptr());
                self.values.push(children);
                self.keys.push(keys);
                MpvNode { value: NodeValue { list }, format: MPV_FORMAT_NODE_MAP }
            }
        }
    }

    fn list(&mut self, children: &mut Vec<MpvNode>, keys: *mut *mut c_char) -> *mut NodeList {
        let mut list = Box::new(NodeList { num: children.len() as c_int, values: children.as_mut_ptr(), keys });
        let ptr: *mut NodeList = &mut *list;
        self.lists.push(list);
        ptr
    }
}

/// Splits a command-line style option (`--name=value`, `--no-name`,
/// `--name`) into the name/value pair `mpv_set_option_string` takes.
pub fn split_option(option: &str) -> (String, String) {
    let option = option.trim_start_matches("--");
    match option.split_once('=') {
        Some((name, value)) => (name.to_string(), value.to_string()),
        // `--no-foo` is `foo=no`; a bare `--foo` is `foo=yes` (mpv's rules).
        None => match option.strip_prefix("no-") {
            Some(name) => (name.to_string(), "no".to_string()),
            None => (option.to_string(), "yes".to_string()),
        },
    }
}

/// Applies `options` (mpv command-line style) to a fresh instance.
pub fn apply_options(mpv: &Mpv, options: &[String]) -> anyhow::Result<()> {
    for option in options {
        let (name, value) = split_option(option);
        tracing::trace!(name, value, "mpv option");
        mpv.set_option(&name, &value)?;
    }
    Ok(())
}
