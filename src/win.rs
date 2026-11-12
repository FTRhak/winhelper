//! Thin RAII wrappers and helpers around general Win32 primitives.

use std::time::Duration;

use windows::core::{Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

/// Owned kernel handle, closed on drop.
pub struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: we own the handle and close it exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

// SAFETY: kernel handles are process-wide and may be used and closed from any thread.
unsafe impl Send for OwnedHandle {}
// SAFETY: as above; the handle value itself is never mutated.
unsafe impl Sync for OwnedHandle {}

/// Unnamed auto-reset event, used to wake a worker thread early.
pub struct Event(OwnedHandle);

impl Event {
    pub fn new() -> Result<Self> {
        // SAFETY: plain FFI call; the handle is owned from here on.
        let handle = unsafe { CreateEventW(None, false, false, PCWSTR::null())? };
        Ok(Self(OwnedHandle(handle)))
    }

    pub fn set(&self) {
        // SAFETY: the handle is a valid event for the lifetime of `self`.
        unsafe {
            let _ = SetEvent(self.0 .0);
        }
    }

    /// Waits until the event is set or `timeout` elapses. Returns `true` if it was set.
    pub fn wait(&self, timeout: Duration) -> bool {
        // INFINITE is u32::MAX, so stay below it.
        let ms = timeout.as_millis().min(u32::MAX as u128 - 1) as u32;
        // SAFETY: the handle is a valid event for the lifetime of `self`.
        unsafe { WaitForSingleObject(self.0 .0, ms) == WAIT_OBJECT_0 }
    }
}

/// Named mutex guaranteeing a single running instance per session.
pub struct SingleInstance {
    _mutex: OwnedHandle,
}

impl SingleInstance {
    /// Returns `Ok(None)` when another instance already owns the mutex.
    pub fn acquire(name: &str) -> Result<Option<Self>> {
        let name = HSTRING::from(name);
        // SAFETY: plain FFI call with a valid, NUL-terminated name.
        let handle = unsafe { CreateMutexW(None, false, &name)? };
        // CreateMutexW succeeds for an existing mutex and reports it via the last error.
        // SAFETY: reads thread-local error state set by the call above.
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let mutex = OwnedHandle(handle);
        Ok((!already_exists).then_some(Self { _mutex: mutex }))
    }
}

/// Shows a modal error message box without an owner window.
pub fn error_box(title: &str, text: &str) {
    let title = HSTRING::from(title);
    let text = HSTRING::from(text);
    // SAFETY: both strings are valid for the duration of the call.
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
