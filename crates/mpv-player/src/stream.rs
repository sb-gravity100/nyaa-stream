//! Safe Rust adapters for libmpv's custom read-only stream callback API.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;
use std::sync::Arc;

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
}

pub(crate) unsafe extern "C" fn open_stream(user_data: *mut c_void, uri: *mut c_char, info: *mut StreamInfo) -> c_int {
    if user_data.is_null() || uri.is_null() || info.is_null() {
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
    let cookie = Box::into_raw(Box::new(StreamCookie { reader })).cast::<c_void>();
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
        return -1;
    }
    if length == 0 {
        return 0;
    }
    let Ok(length) = usize::try_from(length) else { return -1 };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv owns a writable buffer of `length` bytes until this
        // callback returns, and the cookie remains valid until close.
        let reader = unsafe { &*(cookie.cast::<StreamCookie>()) };
        let buffer = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), length) };
        reader.reader.read(buffer)
    }));
    match outcome {
        Ok(Ok(n)) => i64::try_from(n).unwrap_or(-1),
        Ok(Err(error)) => {
            tracing::debug!(%error, "libmpv custom stream read failed");
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
        return -1;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the cookie remains valid until close.
        unsafe { &*(cookie.cast::<StreamCookie>()) }.reader.seek(offset as u64)
    }));
    match outcome {
        Ok(Ok(position)) => i64::try_from(position).unwrap_or(-1),
        Ok(Err(error)) => {
            tracing::debug!(%error, "libmpv custom stream seek failed");
            -1
        }
        Err(_) => -1,
    }
}

unsafe extern "C" fn size_stream(cookie: *mut c_void) -> i64 {
    if cookie.is_null() {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the cookie remains valid until close.
        unsafe { &*(cookie.cast::<StreamCookie>()) }.reader.size()
    }))
    .ok()
    .and_then(|size| i64::try_from(size).ok())
    .unwrap_or(-1)
}

unsafe extern "C" fn close_stream(cookie: *mut c_void) {
    if cookie.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv invokes close exactly once for each successful open.
        drop(unsafe { Box::from_raw(cookie.cast::<StreamCookie>()) });
    }));
}

unsafe extern "C" fn cancel_stream(cookie: *mut c_void) {
    if cookie.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: libmpv's stream lifetime includes its cancel callback.
        unsafe { &*(cookie.cast::<StreamCookie>()) }.reader.cancel();
    }));
}
