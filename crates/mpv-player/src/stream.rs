//! Safe Rust adapters for libmpv's custom read-only stream callback API.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;

pub trait StreamReader: Send + Sync + 'static {
    fn read(&self, buffer: &mut [u8]) -> io::Result<usize>;
    fn seek(&self, offset: u64) -> io::Result<u64>;
    fn size(&self) -> u64;
    fn cancel(&self);

    /// Optional sbtl buffer snapshot supported by the custom mpv extension.
    fn sbtl_buffer(&self) -> Option<StreamBuffer> {
        None
    }

    /// `paused=true` lets sbtl continue the stream's sequential read-ahead.
    fn set_sbtl_paused(&self, _paused: bool) -> io::Result<()> {
        Ok(())
    }
}

pub trait StreamProvider: Send + Sync + 'static {
    fn open(&self, uri: &str) -> Result<Box<dyn StreamReader>>;
}

pub(crate) struct ProviderContext {
    pub(crate) provider: Arc<dyn StreamProvider>,
    pub(crate) cancel_supported: bool,
}

#[repr(C)]
pub(crate) struct StreamInfo {
    cookie: *mut c_void,
    read_fn: Option<unsafe extern "C" fn(*mut c_void, *mut c_char, u64) -> i64>,
    seek_fn: Option<unsafe extern "C" fn(*mut c_void, i64) -> i64>,
    size_fn: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    close_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    cancel_fn: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
pub(crate) struct StreamInfoEx {
    base: StreamInfo,
    get_sbtl_buffer_fn: Option<unsafe extern "C" fn(*mut c_void, *mut StreamBuffer) -> c_int>,
    set_sbtl_idle_fn: Option<unsafe extern "C" fn(*mut c_void, c_int) -> c_int>,
}

pub(crate) type OpenFn = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut StreamInfo) -> c_int;
pub(crate) type OpenFnEx = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut StreamInfoEx) -> c_int;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct StreamBuffer {
    pub ahead_ms: u32,
    pub ready_ms: u32,
    pub eta_ready_ms: u32,
    pub fill_rate: u32,
    pub stalled_ms: u32,
    pub level: u32,
    pub rate_known: c_int,
}

struct StreamCookie {
    reader: Box<dyn StreamReader>,
    opened_at: Instant,
    read_calls: AtomicU64,
    bytes_read: AtomicU64,
    seek_calls: AtomicU64,
    cancelled: AtomicBool,
}

unsafe fn create_stream_cookie(user_data: *mut c_void, uri: *mut c_char) -> Result<(Box<StreamCookie>, bool), c_int> {
    if user_data.is_null() || uri.is_null() {
        tracing::error!("libmpv called custom stream open with a null argument");
        return Err(-13); // MPV_ERROR_LOADING_FAILED
    }
    // SAFETY: the registration's context remains alive until
    // mpv_terminate_destroy returns.
    let context = unsafe { &*(user_data.cast::<ProviderContext>()) };
    let cancel_supported = context.cancel_supported;
    let opened = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv passes a NUL-terminated URI and the registration's
        // context remains alive until mpv_terminate_destroy returns.
        let uri = unsafe { CStr::from_ptr(uri) }.to_string_lossy();
        context.provider.open(&uri)
    }));
    let reader = match opened {
        Ok(Ok(reader)) => reader,
        Ok(Err(error)) => {
            tracing::warn!(%error, "libmpv custom stream open failed");
            return Err(-13);
        }
        Err(_) => {
            tracing::error!("panic while opening libmpv custom stream");
            return Err(-13);
        }
    };
    Ok((Box::new(StreamCookie {
        reader,
        opened_at: Instant::now(),
        read_calls: AtomicU64::new(0),
        bytes_read: AtomicU64::new(0),
        seek_calls: AtomicU64::new(0),
        cancelled: AtomicBool::new(false),
    }), cancel_supported))
}

fn fill_info(info: &mut StreamInfo, cookie: *mut c_void, cancel_supported: bool) {
    info.cookie = cookie;
    info.read_fn = Some(read_stream);
    info.seek_fn = Some(seek_stream);
    info.size_fn = Some(size_stream);
    info.close_fn = Some(close_stream);
    if cancel_supported {
        info.cancel_fn = Some(cancel_stream);
    }
}

pub(crate) unsafe extern "C" fn open_stream(user_data: *mut c_void, uri: *mut c_char, info: *mut StreamInfo) -> c_int {
    if info.is_null() {
        tracing::error!("libmpv called custom stream open with a null info pointer");
        return -13;
    }
    let (cookie, cancel_supported) = match unsafe { create_stream_cookie(user_data, uri) } {
        Ok(opened) => opened,
        Err(error) => return error,
    };
    let cookie = Box::into_raw(cookie).cast::<c_void>();
    tracing::debug!("libmpv custom stream opened");
    // SAFETY: `info` is valid for this callback. Only write the cancel field
    // when supported by the loaded mpv client API (it was added in 1.106).
    unsafe { fill_info(&mut *info, cookie, cancel_supported) };
    0
}

pub(crate) unsafe extern "C" fn open_stream_ex(user_data: *mut c_void, uri: *mut c_char, info: *mut StreamInfoEx) -> c_int {
    if info.is_null() {
        tracing::error!("libmpv called extended custom stream open with a null info pointer");
        return -13;
    }
    let (cookie, cancel_supported) = match unsafe { create_stream_cookie(user_data, uri) } {
        Ok(opened) => opened,
        Err(error) => return error,
    };
    let cookie = Box::into_raw(cookie).cast::<c_void>();
    tracing::debug!("libmpv extended sbtl stream opened");
    // SAFETY: `info` is valid for this callback and the cookie lives until close.
    unsafe {
        fill_info(&mut (*info).base, cookie, cancel_supported);
        (*info).get_sbtl_buffer_fn = Some(get_sbtl_buffer);
        (*info).set_sbtl_idle_fn = Some(set_sbtl_idle);
    }
    0
}

unsafe extern "C" fn get_sbtl_buffer(cookie: *mut c_void, out: *mut StreamBuffer) -> c_int {
    if cookie.is_null() || out.is_null() {
        tracing::error!("libmpv called sbtl buffer callback with a null argument");
        return -1;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: callback cookie and output pointer are valid for this call.
        let stream = unsafe { &*(cookie.cast::<StreamCookie>()) };
        let Some(buffer) = stream.reader.sbtl_buffer() else { return Err(()) };
        unsafe { out.write(buffer) };
        Ok(())
    }));
    match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(())) => -1,
        Err(_) => {
            tracing::error!("panic while querying sbtl stream buffer");
            -1
        }
    }
}

unsafe extern "C" fn set_sbtl_idle(cookie: *mut c_void, idle: c_int) -> c_int {
    if cookie.is_null() || !(0..=1).contains(&idle) {
        tracing::warn!(idle, "libmpv called sbtl idle callback with an invalid argument");
        return -1;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: callback cookie remains valid until the close callback.
        unsafe { &*(cookie.cast::<StreamCookie>()) }.reader.set_sbtl_paused(idle == 1)
    }));
    match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            tracing::warn!(idle, %error, "couldn't update sbtl idle policy");
            -1
        }
        Err(_) => {
            tracing::error!(idle, "panic while updating sbtl idle policy");
            -1
        }
    }
}

unsafe extern "C" fn read_stream(cookie: *mut c_void, buffer: *mut c_char, length: u64) -> i64 {
    if cookie.is_null() || (buffer.is_null() && length != 0) {
        tracing::error!(length, "libmpv called custom stream read with an invalid buffer");
        return -1;
    }
    if length == 0 {
        return 0;
    }
    let Ok(length) = usize::try_from(length) else {
        tracing::warn!(length, "libmpv custom stream read length doesn't fit this platform");
        return -1;
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv owns a writable buffer of `length` bytes until this
        // callback returns, and the cookie remains valid until close.
        let reader = unsafe { &*(cookie.cast::<StreamCookie>()) };
        let buffer = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), length) };
        match reader.reader.read(buffer) {
            Ok(bytes) => {
                reader.read_calls.fetch_add(1, Ordering::Relaxed);
                reader.bytes_read.fetch_add(bytes as u64, Ordering::Relaxed);
                Ok(bytes)
            }
            Err(error) => Err(error),
        }
    }));
    match outcome {
        Ok(Ok(n)) => i64::try_from(n).unwrap_or(-1),
        Ok(Err(error)) => {
            if error.kind() == io::ErrorKind::Interrupted {
                tracing::debug!(%error, "libmpv custom stream read interrupted");
            } else {
                tracing::warn!(%error, "libmpv custom stream read failed");
            }
            -1
        }
        Err(_) => {
            tracing::error!("panic while reading libmpv custom stream");
            -1
        }
    }
}

unsafe extern "C" fn seek_stream(cookie: *mut c_void, offset: i64) -> i64 {
    if cookie.is_null() || offset < 0 {
        tracing::warn!(offset, "libmpv custom stream received an invalid seek");
        return -1;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the cookie remains valid until close.
        let stream = unsafe { &*(cookie.cast::<StreamCookie>()) };
        stream.seek_calls.fetch_add(1, Ordering::Relaxed);
        stream.reader.seek(offset as u64)
    }));
    match outcome {
        Ok(Ok(position)) => {
            tracing::debug!(offset, position, "libmpv custom stream seek completed");
            i64::try_from(position).unwrap_or(-1)
        }
        Ok(Err(error)) => {
            tracing::warn!(offset, %error, "libmpv custom stream seek failed");
            -1
        }
        Err(_) => {
            tracing::error!(offset, "panic while seeking libmpv custom stream");
            -1
        }
    }
}

unsafe extern "C" fn size_stream(cookie: *mut c_void) -> i64 {
    if cookie.is_null() {
        return -1;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the cookie remains valid until close.
        unsafe { &*(cookie.cast::<StreamCookie>()) }.reader.size()
    }));
    match outcome {
        Ok(size) => match i64::try_from(size) {
            Ok(size) => {
                tracing::debug!(size, "libmpv custom stream size queried");
                size
            }
            Err(error) => {
                tracing::warn!(%error, "libmpv custom stream size doesn't fit its API");
                -1
            }
        },
        Err(_) => {
            tracing::error!("panic while querying libmpv custom stream size");
            -1
        }
    }
}

unsafe extern "C" fn close_stream(cookie: *mut c_void) {
    if cookie.is_null() {
        return;
    }
    if catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv invokes close exactly once for each successful open.
        let stream = unsafe { Box::from_raw(cookie.cast::<StreamCookie>()) };
        tracing::debug!(
            read_calls = stream.read_calls.load(Ordering::Relaxed),
            bytes_read = stream.bytes_read.load(Ordering::Relaxed),
            seek_calls = stream.seek_calls.load(Ordering::Relaxed),
            lifetime_ms = stream.opened_at.elapsed().as_millis() as u64,
            "libmpv custom stream closed"
        );
        drop(stream);
    })).is_err() {
        tracing::error!("panic while closing libmpv custom stream");
    }
}

unsafe extern "C" fn cancel_stream(cookie: *mut c_void) {
    if cookie.is_null() {
        return;
    }
    if catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv's stream lifetime includes its cancel callback.
        let stream = unsafe { &*(cookie.cast::<StreamCookie>()) };
        if !stream.cancelled.swap(true, Ordering::Relaxed) {
            tracing::debug!("libmpv custom stream cancelled");
        }
        stream.reader.cancel();
    })).is_err() {
        tracing::error!("panic while cancelling libmpv custom stream");
    }
}
