//! Thin RAII wrappers and helpers around general Win32 primitives.

use windows::core::{Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND};
use windows::Win32::System::Threading::CreateMutexW;
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
