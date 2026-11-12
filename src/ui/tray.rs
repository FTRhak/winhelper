//! Notification-area icon. Right-click opens the context menu, double-click
//! shows / hides the panel (handled in `window`).

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, LoadImageW, RegisterWindowMessageW, HICON, IMAGE_ICON, LR_SHARED,
    SM_CXSMICON, SM_CYSMICON, WM_APP,
};

/// Callback message the shell sends to the panel for tray mouse events.
pub const WM_APP_TRAY: u32 = WM_APP + 1;

const ICON_ID: u32 = 1;

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..Default::default()
    }
}

/// Adds the icon; call again after `taskbar_created_message` (Explorer restart).
pub fn add(hwnd: HWND, icon_resource: u16) {
    let mut nid = data(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    let tip: Vec<u16> = "WinHelper".encode_utf16().collect();
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    // SAFETY: plain FFI calls; LR_SHARED icons are owned by the system.
    unsafe {
        if let Ok(module) = GetModuleHandleW(None) {
            if let Ok(icon) = LoadImageW(
                module,
                PCWSTR(icon_resource as usize as *const u16),
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_SHARED,
            ) {
                nid.hIcon = HICON(icon.0);
            }
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

pub fn remove(hwnd: HWND) {
    let nid = data(hwnd);
    // SAFETY: plain FFI call.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// Message broadcast when Explorer (re)creates the taskbar.
pub fn taskbar_created_message() -> u32 {
    // SAFETY: plain FFI call with a static string.
    unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }
}
