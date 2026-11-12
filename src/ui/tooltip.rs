//! Detail tooltip: a popup owned by the strip, shown below it (above near the
//! bottom of the work area) after the cursor rests on the strip for `DELAY`.
//! It is click-through and never activates, so the strip keeps the hover.

use std::time::Duration;

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowRect, RegisterClassExW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, CS_DROPSHADOW, HTTRANSPARENT, LWA_ALPHA, MA_NOACTIVATE,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WM_ERASEBKGND, WM_MOUSEACTIVATE,
    WM_NCHITTEST, WM_PAINT, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use super::gdi::{BackBuffer, PaintGuard};
use super::layout::{tip_origin, TIP_GAP, TIP_HEIGHT, TIP_WIDTH};
use super::paint::TipPaint;
use super::{scale, state, window};

const CLASS_NAME: PCWSTR = w!("WinHelper.Tip");

/// Hover time before the tooltip opens.
pub const DELAY: Duration = Duration::from_millis(400);

pub fn create(instance: HINSTANCE, owner: HWND) -> Result<HWND> {
    // SAFETY: standard window creation on the UI thread.
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_win32());
        }
        // Layered + transparent: mouse input passes through to whatever is below.
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!("WinHelper details"),
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
        window::round_corners(hwnd, false);
        Ok(hwnd)
    }
}

fn size(dpi: u32) -> (i32, i32) {
    (scale(TIP_WIDTH, dpi), scale(TIP_HEIGHT, dpi))
}

/// Places the tooltip next to the strip and shows it (or just moves it).
pub fn show_near(tip: HWND, panel: HWND, dpi: u32) {
    let (w, h) = size(dpi);
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
        let pt = tip_origin(&panel_rect, &info.rcWork, w, h, scale(TIP_GAP, dpi));
        let _ = SetWindowPos(tip, None, pt.x, pt.y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        let _ = InvalidateRect(tip, None, false);
        let _ = ShowWindow(tip, SW_SHOWNOACTIVATE);
    }
}

pub fn hide(tip: HWND) {
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = ShowWindow(tip, SW_HIDE);
    }
}

fn on_paint(hwnd: HWND) {
    let guard = PaintGuard::begin(hwnd);
    if guard.hdc.is_invalid() {
        return;
    }
    state::with(|ui| {
        let (width, height) = size(ui.dpi);
        let Some(buffer) = BackBuffer::new(guard.hdc, width, height) else {
            return;
        };
        TipPaint {
            width,
            height,
            dpi: ui.dpi,
            palette: ui.theme.palette(),
            fonts: &ui.fonts,
            view: &ui.view,
        }
        .draw(buffer.dc);
        buffer.blit(guard.hdc);
    });
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: called by the system on the UI thread with valid parameters for `msg`.
    unsafe {
        match msg {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                on_paint(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
