//! UI-thread state shared by the panel and close-button window procedures.
//!
//! Access goes through `with`, which never holds the borrow across a call that
//! can re-enter a window procedure (SetWindowPos, ShowWindow, TrackPopupMenu, ...):
//! callers copy what they need out, release the borrow, then call Win32.

use std::cell::RefCell;

use windows::Win32::Foundation::HWND;

use super::paint::Fonts;
use super::theme::Theme;
use super::view::PanelView;
use crate::metrics::Sampler;
use crate::net::Lookup;

pub struct Ui {
    pub panel: HWND,
    pub close: HWND,
    pub dpi: u32,
    pub theme: Theme,
    pub fonts: Fonts,
    pub view: PanelView,
    /// Cursor is over the panel (or its close button): hover border + close shown.
    pub hovered: bool,
    /// Cursor is over the close button itself.
    pub close_hovered: bool,
    /// The panel is the active (foreground) window.
    pub focused: bool,
    /// CPU / RAM / GPU sampler thread; `None` in demo mode or if it failed to start.
    pub sampler: Option<Sampler>,
    /// Public / LAN IP lookup thread; `None` in demo mode or if it failed to start.
    pub net: Option<Lookup>,
    pub topmost: bool,
    /// Window opacity in percent.
    pub opacity: u8,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

pub fn init(ui: Ui) {
    UI.with(|cell| *cell.borrow_mut() = Some(ui));
}

/// Drops the state (fonts) once the panel is destroyed.
pub fn clear() {
    UI.with(|cell| cell.borrow_mut().take());
}

/// Runs `f` on the state. Returns `None` before `init` or if re-entered.
pub fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UI.with(|cell| {
        let mut guard = cell.try_borrow_mut().ok()?;
        guard.as_mut().map(f)
    })
}
