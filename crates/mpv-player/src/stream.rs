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

pub(crate) type OpenFn = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut StreamInfo) -> c_int;

struct StreamCookie {
    reader: Box<dyn StreamReader>,
    opened_at: Instant,
    read_calls: AtomicU64,
    bytes_read: AtomicU64,
    seek_calls: AtomicU64,
    cancelled: AtomicBool,
}

pub(crate) unsafe extern "C" fn open_stream(user_data: *mut c_void, uri: *mut c_char, info: *mut StreamInfo) -> c_int {
    if user_data.is_null() || uri.is_null() || info.is_null() {
        tracing::error!("libmpv called custom stream open with a null argument");
        return -13; // MPV_ERROR_LOADING_FAILED
    }
    // SAFETY: the registration's context remains alive until
    // mpv_terminate_destroy returns.
    let cancel_supported = unsafe { (*(user_data.cast::<ProviderContext>())).cancel_supported };
    let opened = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv passes a NUL-terminated URI and the registration's
        // context remains alive until mpv_terminate_destroy returns.
        let context = unsafe { &*(user_data.cast::<ProviderContext>()) };
        let uri = unsafe { CStr::from_ptr(uri) }.to_string_lossy();
        context.provider.open(&uri)
    }));
    let reader = match opened {
        Ok(Ok(reader)) => reader,
        Ok(Err(error)) => {
            tracing::warn!(%error, "libmpv custom stream open failed");
            return -13;
        }
        Err(_) => {
            tracing::error!("panic while opening libmpv custom stream");
            return -13;
        }
    };
    let cookie = Box::into_raw(Box::new(StreamCookie {
        reader,
        opened_at: Instant::now(),
        read_calls: AtomicU64::new(0),
        bytes_read: AtomicU64::new(0),
        seek_calls: AtomicU64::new(0),
        cancelled: AtomicBool::new(false),
    })).cast::<c_void>();
    tracing::debug!("libmpv custom stream opened");
    // SAFETY: `info` is valid for this callback. Only write the cancel field
    // when supported by the loaded mpv client API (it was added in 1.106).
    unsafe {
        (*info).cookie = cookie;
        (*info).read_fn = Some(read_stream);
        (*info).seek_fn = Some(seek_stream);
        (*info).size_fn = Some(size_stream);
        (*info).close_fn = Some(close_stream);
        if cancel_supported {
            (*info).cancel_fn = Some(cancel_stream);
        }
    }
    0
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
