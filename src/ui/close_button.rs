//! Close button: a small popup owned by the panel, shown 6 px beside it while the
//! cursor is over the panel (design: "Close button behavior"). Clicking it hides
//! the panel to the tray.

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowRect, LoadCursorW, PostMessageW, RegisterClassExW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, CS_DROPSHADOW, IDC_ARROW, LWA_ALPHA,
    MA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WM_ERASEBKGND,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

use super::gdi::{BackBuffer, PaintGuard};
use super::layout::{close_button_origin, CLOSE_GAP, CLOSE_SIZE};
use super::{paint, scale, state, window};

const CLASS_NAME: PCWSTR = w!("WinHelper.Close");

/// Defined in `Win32_UI_Controls`, which is too large a feature to enable for one constant.
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Posted to the panel when the button is clicked.
pub const WM_APP_HIDE_PANEL: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

pub fn create(instance: HINSTANCE, owner: HWND) -> Result<HWND> {
    // SAFETY: standard window creation on the UI thread.
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!("Close WinHelper"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            owner,
            None,
            instance,
            None,
        )?;
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
        window::round_corners(hwnd, true);
        Ok(hwnd)
    }
}

/// Places the button next to the panel (flipping left near the work-area edge)
/// and shows it.
pub fn show_beside(button: HWND, panel: HWND, dpi: u32) {
    let size = scale(CLOSE_SIZE, dpi);
    // SAFETY: both windows belong to this thread.
    unsafe {
        let mut panel_rect = RECT::default();
        if GetWindowRect(panel, &mut panel_rect).is_err() {
            return;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let monitor = MonitorFromWindow(panel, MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(monitor, &mut info);
        let pt = close_button_origin(&panel_rect, &info.rcWork, size, scale(CLOSE_GAP, dpi));
        let _ = SetWindowPos(
            button,
            None,
            pt.x,
            pt.y,
            size,
            size,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(button, SW_SHOWNOACTIVATE);
    }
}

pub fn hide(button: HWND) {
    state::with(|ui| ui.close_hovered = false);
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = ShowWindow(button, SW_HIDE);
    }
}

fn set_hovered(hwnd: HWND, hovered: bool) {
    let changed = state::with(|ui| std::mem::replace(&mut ui.close_hovered, hovered) != hovered);
    if changed == Some(true) {
        window::update_border(hwnd);
        // SAFETY: the window belongs to this thread.
        unsafe {
            let _ = InvalidateRect(hwnd, None, false);
        }
    }
}

fn on_paint(hwnd: HWND) {
    let guard = PaintGuard::begin(hwnd);
    if guard.hdc.is_invalid() {
        return;
    }
    state::with(|ui| {
        let size = scale(CLOSE_SIZE, ui.dpi);
        let Some(buffer) = BackBuffer::new(guard.hdc, size, size) else {
            return;
        };
        paint::close_button(
            buffer.dc,
            size,
            ui.theme.palette(),
            &ui.fonts,
            ui.close_hovered,
        );
        buffer.blit(guard.hdc);
    });
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: called by the system on the UI thread with valid parameters for `msg`.
    unsafe {
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_MOUSEMOVE => {
                if state::with(|ui| ui.close_hovered) == Some(false) {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut track);
                    set_hovered(hwnd, true);
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                set_hovered(hwnd, false);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if state::with(|ui| ui.close_hovered) == Some(true) {
                    if let Some(panel) = state::with(|ui| ui.panel) {
                        let _ = PostMessageW(panel, WM_APP_HIDE_PANEL, WPARAM(0), LPARAM(0));
                    }
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                on_paint(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
