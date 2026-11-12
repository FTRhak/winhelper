//! Strip window: class, WndProc, dragging, hover / close button / detail
//! tooltip, tray, theme and DPI handling.

use std::time::Instant;

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWMWCP_ROUNDSMALL,
};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos,
    GetMessageW, GetWindowRect, IsIconic, IsWindowVisible, KillTimer, LoadCursorW, LoadIconW,
    PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer,
    SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage, CS_DBLCLKS, CS_DROPSHADOW,
    HTCAPTION, HTCLIENT, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA, MSG, SPI_GETWORKAREA,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, SW_SHOWNOACTIVATE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WA_INACTIVE, WM_ACTIVATE, WM_CONTEXTMENU, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_MOVE, WM_NCHITTEST, WM_NCMOUSEMOVE,
    WM_NCRBUTTONUP, WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::close_button::{self, WM_APP_HIDE_PANEL};
use super::gdi::{BackBuffer, PaintGuard};
use super::layout::{strip_width, STRIP_HEIGHT};
use super::menu::{self, Command, MenuState};
use super::paint::{Fonts, StripPaint};
use super::scale;
use super::state::{self, Ui};
use super::theme;
use super::tooltip;
use super::tray::{self, WM_APP_TRAY};
use super::view::PanelView;
use crate::metrics::{Sampler, WM_APP_METRICS};
use crate::net::{Lookup, WM_APP_NETINFO};

const CLASS_NAME: PCWSTR = w!("WinHelper.Panel");
const WINDOW_TITLE: PCWSTR = w!("WinHelper");

/// Margin from the work-area corner on first placement, at 96 DPI.
const SCREEN_MARGIN: i32 = 16;

/// Resource id of the application icon (res/winhelper.rc).
const IDI_APP: u16 = 1;

/// Polls the cursor while the close button is shown, to hide it once the cursor
/// leaves both windows (the gap between them makes leave events unreliable).
const HOVER_TIMER: usize = 1;
const HOVER_POLL_MS: u32 = 100;

/// One-shot timer for the first network lookup. WM_TIMER is only dispatched
/// by the running message loop, so the panel is shown and painted first.
const NET_START_TIMER: usize = 2;
const NET_START_DELAY_MS: u32 = 1000;

/// Brings an already running instance's panel to the front.
pub fn activate_existing() {
    // SAFETY: plain FFI calls on a window owned by another process.
    unsafe {
        if let Ok(hwnd) = FindWindowW(CLASS_NAME, PCWSTR::null()) {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

/// Registers the window classes and creates the (initially shown) panel.
pub fn create() -> Result<HWND> {
    // SAFETY: standard window creation on the UI thread; all pointers outlive the calls.
    unsafe {
        let instance = GetModuleHandleW(None)?.into();

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS | CS_DROPSHADOW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: LoadIconW(instance, PCWSTR(IDI_APP as usize as *const u16))?,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_win32());
        }

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TOPMOST,
            CLASS_NAME,
            WINDOW_TITLE,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )?;
        let close = close_button::create(instance, hwnd)?;
        let tip = tooltip::create(instance, hwnd)?;

        let dpi = GetDpiForWindow(hwnd);
        let theme = theme::system_theme();
        // Demo data stays fixed, so no worker thread overwrites it.
        let (view, sampler, net) = match demo_view() {
            Some(view) => (view, None, None),
            None => (
                PanelView::default(),
                Sampler::start(hwnd),
                Lookup::start(hwnd),
            ),
        };
        state::init(Ui {
            panel: hwnd,
            close,
            tip,
            dpi,
            theme,
            fonts: Fonts::new(dpi),
            view,
            hovered: false,
            hover_since: None,
            tip_shown: false,
            close_hovered: false,
            focused: false,
            sampler,
            net,
            topmost: true,
            opacity: 100,
        });
        theme::apply_menu_theme(theme);

        set_opacity(100);
        round_corners(hwnd, false);
        update_border(hwnd);
        update_border(close);
        update_border(tip);
        tray::add(hwnd, IDI_APP);

        place_initial(hwnd);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        if state::with(|ui| ui.net.is_some()) == Some(true) {
            SetTimer(hwnd, NET_START_TIMER, NET_START_DELAY_MS, None);
        }
        Ok(hwnd)
    }
}

/// Runs the message loop until WM_QUIT.
pub fn run_message_loop() {
    let mut msg = MSG::default();
    // SAFETY: standard message pump on the thread that owns the window.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn demo_view() -> Option<PanelView> {
    #[cfg(debug_assertions)]
    return super::demo::view();
    #[cfg(not(debug_assertions))]
    None
}

/// Samples every second while the panel is focused or hovered, every ten
/// seconds otherwise. Safe to call inside `state::with` (no window calls).
fn update_sampling(ui: &Ui) {
    if let Some(sampler) = &ui.sampler {
        sampler.set_active(ui.focused || ui.hovered);
    }
}

/// WM_APP_METRICS: copy the newest snapshot into the view and repaint. When
/// Windows reports internet access again after being offline, the public IP
/// is looked up again (it may have changed).
fn on_metrics(hwnd: HWND) {
    let changed = state::with(|ui| {
        let snap = ui.sampler.as_ref().and_then(Sampler::take_latest)?;
        let v = &mut ui.view;
        let before = v.clone();
        let reconnected = v.online == Some(false) && snap.online == Some(true);
        v.cpu = snap.cpu;
        v.ram = snap.ram;
        v.gpu = snap.gpu;
        v.cpu_temp = snap.cpu_temp;
        v.gpu_temp = snap.gpu_temp;
        v.disk = snap.disk;
        v.traffic = snap.traffic;
        v.online = snap.online;
        if reconnected {
            if let Some(net) = &ui.net {
                net.refresh();
            }
        }
        let resized = (before.cpu_temp.is_some(), before.gpu_temp.is_some())
            != (v.cpu_temp.is_some(), v.gpu_temp.is_some());
        (before != *v).then_some((ui.tip, resized))
    });
    if let Some(Some((tip, resized))) = changed {
        if resized {
            fit_to_content(hwnd);
        }
        invalidate(hwnd);
        invalidate(tip);
    }
}

/// WM_APP_NETINFO: merge the finished lookup into the view and repaint.
fn on_netinfo(hwnd: HWND) {
    let changed = state::with(|ui| {
        let info = ui.net.as_ref().and_then(Lookup::take_latest)?;
        let before = ui.view.net.clone();
        ui.view.net.update(info);
        (before != ui.view.net).then_some(ui.tip)
    });
    if let Some(Some(tip)) = changed {
        invalidate(hwnd);
        invalidate(tip);
    }
}

/// Requests Windows 11 rounded corners (8 px, or 4 px when `small`). No-op on Windows 10.
pub fn round_corners(hwnd: HWND, small: bool) {
    let pref = if small {
        DWMWCP_ROUNDSMALL
    } else {
        DWMWCP_ROUND
    };
    // SAFETY: the attribute value is a 4-byte enum, as the API expects.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}

/// Matches the Windows 11 DWM border of the strip, close button or tooltip to
/// the current theme and hover state. No-op on Windows 10, where the painted
/// 1 px border is the only one.
pub fn update_border(hwnd: HWND) {
    let Some(color) = state::with(|ui| {
        let p = ui.theme.palette();
        if hwnd == ui.tip {
            p.tip_border
        } else if hwnd == ui.close {
            if ui.close_hovered {
                p.close_hover
            } else {
                p.close_border
            }
        } else if ui.hovered {
            p.border_hover
        } else {
            p.border
        }
    }) else {
        return;
    };
    // SAFETY: COLORREF is a 4-byte value, as the API expects.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const COLORREF as *const _,
            std::mem::size_of::<COLORREF>() as u32,
        );
    }
}

/// Strip size in device pixels; temperature slots only for sensors that report.
fn panel_size(dpi: u32, view: &PanelView) -> (i32, i32) {
    let width = strip_width(view.cpu_temp.is_some(), view.gpu_temp.is_some());
    (scale(width, dpi), scale(STRIP_HEIGHT, dpi))
}

/// Current strip size from the UI state (zero before `state::init`).
fn current_panel_size() -> (i32, i32) {
    state::with(|ui| panel_size(ui.dpi, &ui.view)).unwrap_or_default()
}

/// Resizes the strip to its content, keeping the right edge in place (the
/// panel starts in the top-right corner), and moves the close button along.
fn fit_to_content(hwnd: HWND) {
    let (w, h) = current_panel_size();
    let mut r = RECT::default();
    // SAFETY: hwnd is a valid window owned by this thread.
    unsafe {
        if GetWindowRect(hwnd, &mut r).is_err() || r.right - r.left == w {
            return;
        }
        let _ = SetWindowPos(
            hwnd,
            None,
            r.right - w,
            r.top,
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    invalidate(hwnd);
    follow_strip(hwnd);
}

/// Sizes the panel for its monitor DPI and puts it in the top-right corner of the
/// primary work area. Saved positions replace this in M4.
fn place_initial(hwnd: HWND) {
    // SAFETY: hwnd is a valid window owned by this thread.
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let (w, h) = current_panel_size();
        let margin = scale(SCREEN_MARGIN, dpi);

        let mut work = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = work.right - w - margin;
        let y = work.top + margin;
        let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

fn invalidate(hwnd: HWND) {
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

fn on_paint(hwnd: HWND) {
    let guard = PaintGuard::begin(hwnd);
    if guard.hdc.is_invalid() {
        return;
    }
    state::with(|ui| {
        let (width, height) = panel_size(ui.dpi, &ui.view);
        let Some(buffer) = BackBuffer::new(guard.hdc, width, height) else {
            return;
        };
        StripPaint {
            width,
            height,
            dpi: ui.dpi,
            palette: ui.theme.palette(),
            fonts: &ui.fonts,
            view: &ui.view,
            hovered: ui.hovered,
        }
        .draw(buffer.dc);
        buffer.blit(guard.hdc);
    });
}

/// Cursor entered the strip: hover border on, close button shown; the hover
/// timer opens the tooltip after `tooltip::DELAY`.
fn on_hover(hwnd: HWND) {
    let Some((close, dpi)) = state::with(|ui| {
        (!ui.hovered).then(|| {
            ui.hovered = true;
            ui.hover_since = Some(Instant::now());
            update_sampling(ui);
            (ui.close, ui.dpi)
        })
    })
    .flatten() else {
        return;
    };
    update_border(hwnd);
    invalidate(hwnd);
    close_button::show_beside(close, hwnd, dpi);
    // SAFETY: the window belongs to this thread.
    unsafe {
        SetTimer(hwnd, HOVER_TIMER, HOVER_POLL_MS, None);
    }
}

fn end_hover(hwnd: HWND) {
    let Some((close, tip)) = state::with(|ui| {
        ui.hovered = false;
        ui.hover_since = None;
        ui.tip_shown = false;
        update_sampling(ui);
        (ui.close, ui.tip)
    }) else {
        return;
    };
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = KillTimer(hwnd, HOVER_TIMER);
    }
    tooltip::hide(tip);
    close_button::hide(close);
    update_border(hwnd);
    update_border(close);
    invalidate(hwnd);
}

fn on_hover_timer(hwnd: HWND) {
    let Some(close) = state::with(|ui| ui.close) else {
        return;
    };
    // SAFETY: both windows belong to this thread.
    let inside = unsafe {
        let (mut pt, mut panel, mut button) = (POINT::default(), RECT::default(), RECT::default());
        GetCursorPos(&mut pt).is_err()
            || GetWindowRect(hwnd, &mut panel).is_err()
            || GetWindowRect(close, &mut button).is_err()
            || super::layout::in_hover_zone(pt, &panel, &button)
    };
    if !inside {
        end_hover(hwnd);
        return;
    }
    // Open the tooltip once the cursor has rested long enough.
    let due = state::with(|ui| {
        let due = !ui.tip_shown
            && ui
                .hover_since
                .is_some_and(|t| t.elapsed() >= tooltip::DELAY);
        if due {
            ui.tip_shown = true;
        }
        due.then_some((ui.tip, ui.dpi))
    });
    if let Some(Some((tip, dpi))) = due {
        tooltip::show_near(tip, hwnd, dpi);
    }
}

fn set_opacity(percent: u8) {
    let Some((panel, close)) = state::with(|ui| {
        ui.opacity = percent;
        (ui.panel, ui.close)
    }) else {
        return;
    };
    let alpha = (percent.min(100) as u32 * 255 / 100) as u8;
    // SAFETY: both windows are layered and belong to this thread.
    unsafe {
        let _ = SetLayeredWindowAttributes(panel, COLORREF(0), alpha, LWA_ALPHA);
        let _ = SetLayeredWindowAttributes(close, COLORREF(0), alpha, LWA_ALPHA);
    }
}

fn toggle_topmost(hwnd: HWND) {
    let Some(topmost) = state::with(|ui| {
        ui.topmost = !ui.topmost;
        ui.topmost
    }) else {
        return;
    };
    let after = if topmost {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Hides the panel to the tray, or shows it again.
fn set_visible(hwnd: HWND, visible: bool) {
    if visible {
        // SAFETY: the window belongs to this thread.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    } else {
        end_hover(hwnd);
        // SAFETY: as above.
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

fn show_menu(hwnd: HWND, pt: POINT) {
    // The tooltip would cover the menu; it comes back after a fresh hover delay.
    let Some((menu_state, tip)) = state::with(|ui| {
        ui.hover_since = None;
        ui.tip_shown = false;
        let menu_state = MenuState {
            topmost: ui.topmost,
            opacity: ui.opacity,
            can_refresh_ip: ui.net.is_some(),
        };
        (menu_state, ui.tip)
    }) else {
        return;
    };
    tooltip::hide(tip);
    let command = menu::show(hwnd, pt, &menu_state);
    state::with(|ui| {
        if ui.hovered {
            ui.hover_since = Some(Instant::now());
        }
    });
    match command {
        Some(Command::ToggleTopmost) => toggle_topmost(hwnd),
        Some(Command::Opacity(level)) => set_opacity(level),
        Some(Command::RefreshIp) => {
            state::with(|ui| ui.net.as_ref().map(Lookup::refresh));
        }
        Some(Command::Exit) => {
            // SAFETY: the window belongs to this thread.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        None => {}
    }
}

fn on_theme_change(hwnd: HWND) {
    let theme = theme::system_theme();
    let Some((close, tip)) = state::with(|ui| {
        ui.theme = theme;
        (ui.close, ui.tip)
    }) else {
        return;
    };
    theme::apply_menu_theme(theme);
    for w in [hwnd, close, tip] {
        update_border(w);
        invalidate(w);
    }
}

/// Keeps the close button and an open tooltip next to the strip after it
/// moved or changed size.
fn follow_strip(hwnd: HWND) {
    let Some((hovered, tip_shown, close, tip, dpi)) =
        state::with(|ui| (ui.hovered, ui.tip_shown, ui.close, ui.tip, ui.dpi))
    else {
        return;
    };
    if hovered {
        close_button::show_beside(close, hwnd, dpi);
    }
    if tip_shown {
        tooltip::show_near(tip, hwnd, dpi);
    }
}

/// `lparam` of WM_SETTINGCHANGE names the changed area; theme switches send
/// "ImmersiveColorSet".
fn is_theme_change(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    // SAFETY: for WM_SETTINGCHANGE a non-null lparam is a NUL-terminated wide string.
    let area = unsafe { PCWSTR(lparam.0 as *const u16).to_string() };
    area.is_ok_and(|s| s == "ImmersiveColorSet")
}

fn point_from_lparam(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xffff) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    thread_local! {
        static TASKBAR_CREATED: u32 = tray::taskbar_created_message();
    }

    // SAFETY: called by the system on the UI thread with valid parameters for `msg`.
    unsafe {
        match msg {
            WM_NCHITTEST => {
                // Let the user drag the panel from anywhere in the client area.
                let hit = DefWindowProcW(hwnd, msg, wparam, lparam);
                if hit.0 as u32 == HTCLIENT {
                    LRESULT(HTCAPTION as isize)
                } else {
                    hit
                }
            }
            // The whole client area reports HTCAPTION, so moves arrive as NC messages.
            WM_NCMOUSEMOVE => {
                on_hover(hwnd);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_TIMER if wparam.0 == HOVER_TIMER => {
                on_hover_timer(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == NET_START_TIMER => {
                let _ = KillTimer(hwnd, NET_START_TIMER);
                state::with(|ui| ui.net.as_ref().map(Lookup::refresh));
                LRESULT(0)
            }
            WM_MOVE => {
                follow_strip(hwnd);
                LRESULT(0)
            }
            // Right-clicks on the HTCAPTION client area arrive as NC messages and do not
            // reliably produce WM_CONTEXTMENU, so handle the button-up directly.
            WM_NCRBUTTONUP if wparam.0 as u32 == HTCAPTION => {
                show_menu(hwnd, point_from_lparam(lparam));
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                show_menu(hwnd, point_from_lparam(lparam));
                LRESULT(0)
            }
            WM_APP_TRAY => {
                match (lparam.0 & 0xffff) as u32 {
                    WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd, POINT { x: -1, y: -1 }),
                    WM_LBUTTONDBLCLK => set_visible(hwnd, !IsWindowVisible(hwnd).as_bool()),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_APP_HIDE_PANEL => {
                set_visible(hwnd, false);
                LRESULT(0)
            }
            WM_APP_METRICS => {
                on_metrics(hwnd);
                LRESULT(0)
            }
            WM_APP_NETINFO => {
                on_netinfo(hwnd);
                LRESULT(0)
            }
            WM_ACTIVATE => {
                let focused = (wparam.0 & 0xffff) as u32 != WA_INACTIVE;
                state::with(|ui| {
                    ui.focused = focused;
                    update_sampling(ui);
                });
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETTINGCHANGE => {
                if is_theme_change(lparam) {
                    on_theme_change(hwnd);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                on_paint(hwnd);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as u32;
                state::with(|ui| {
                    ui.dpi = dpi;
                    ui.fonts = Fonts::new(dpi);
                });
                // lparam points to the suggested new window rect; keep its origin
                // and use the exact layout size for the new DPI.
                let r = &*(lparam.0 as *const RECT);
                let (w, h) = current_panel_size();
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                invalidate(hwnd);
                follow_strip(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = KillTimer(hwnd, HOVER_TIMER);
                let _ = KillTimer(hwnd, NET_START_TIMER);
                tray::remove(hwnd);
                // Stop the worker threads (outside the state borrow).
                drop(state::with(|ui| (ui.sampler.take(), ui.net.take())));
                state::clear();
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ if msg != 0 && msg == TASKBAR_CREATED.with(|m| *m) => {
                tray::add(hwnd, IDI_APP);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
