//! Minimal synchronous HTTPS GET over WinHTTP (PLAN.md §3, §5.5).
//!
//! Uses the system proxy settings and 5 s timeouts per stage. Only meant for
//! the small JSON / text replies of the IP and geo providers.

use std::ffi::c_void;

use windows::core::{w, Error, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

/// Generic client name; carries nothing about the user.
const USER_AGENT: PCWSTR = w!("WinHelper/0.1");

/// Resolve, connect, send and receive timeout, each.
const TIMEOUT_MS: i32 = 5_000;

/// Provider replies are a few hundred bytes; refuse anything absurd.
const MAX_BODY: usize = 64 * 1024;

/// Owned HINTERNET, closed on drop.
struct Handle(*mut c_void);

impl Handle {
    /// Wraps the result of a WinHTTP call that returns NULL on failure.
    fn new(raw: *mut c_void) -> Result<Self> {
        if raw.is_null() {
            Err(Error::from_win32())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: we own the handle and close it exactly once.
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}

/// A WinHTTP session, reused for every request of one refresh thread.
pub struct Session(Handle);

impl Session {
    pub fn open() -> Result<Self> {
        // SAFETY: plain FFI calls; the handle is owned from here on.
        unsafe {
            let session = Handle::new(WinHttpOpen(
                USER_AGENT,
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ))?;
            WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS)?;
            Ok(Self(session))
        }
    }

    /// `GET https://{host}{path}`; the body of a 200 reply, else an error.
    pub fn get(&self, host: &str, path: &str) -> Result<Vec<u8>> {
        let host = HSTRING::from(host);
        let path = HSTRING::from(path);
        // SAFETY: every handle is owned by a `Handle` and outlives its uses;
        // locals drop in reverse order, so the request closes before the connection.
        unsafe {
            let connect = Handle::new(WinHttpConnect(
                self.0 .0,
                &host,
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            ))?;
            let request = Handle::new(WinHttpOpenRequest(
                connect.0,
                w!("GET"),
                &path,
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            WinHttpSendRequest(request.0, None, None, 0, 0, 0)?;
            WinHttpReceiveResponse(request.0, std::ptr::null_mut())?;
            if status_code(&request)? != 200 {
                return Err(E_FAIL.into());
            }
            read_body(&request)
        }
    }
}

/// # Safety
/// `request` must have a received response.
unsafe fn status_code(request: &Handle) -> Result<u32> {
    let mut code = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    WinHttpQueryHeaders(
        request.0,
        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        PCWSTR::null(),
        Some(&mut code as *mut u32 as *mut c_void),
        &mut len,
        std::ptr::null_mut(),
    )?;
    Ok(code)
}

/// # Safety
/// `request` must have a received response.
unsafe fn read_body(request: &Handle) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let mut available = 0u32;
        WinHttpQueryDataAvailable(request.0, &mut available)?;
        if available == 0 {
            return Ok(body);
        }
        let start = body.len();
        let end = start + available as usize;
        if end > MAX_BODY {
            return Err(E_FAIL.into());
        }
        body.resize(end, 0);
        let mut read = 0u32;
        WinHttpReadData(
            request.0,
            body[start..].as_mut_ptr().cast(),
            available,
            &mut read,
        )?;
        body.truncate(start + read as usize);
        if read == 0 {
            return Ok(body);
        }
    }
}
