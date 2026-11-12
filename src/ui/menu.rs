//! Context menu shared by the panel and the tray icon (design: "Right-click menu").
//!
//! Dark / light rendering comes from `theme::apply_menu_theme`. Items whose
//! features land in later milestones are shown disabled.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CheckMenuRadioItem, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW,
    SetForegroundWindow, TrackPopupMenu, HMENU, MENU_ITEM_FLAGS, MF_BYCOMMAND, MF_CHECKED,
    MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_NULL,
};

/// Opacity choices offered in the submenu, in percent.
pub const OPACITY_LEVELS: [u8; 3] = [100, 85, 70];

const CMD_TOPMOST: u32 = 1;
const CMD_REFRESH_IP: u32 = 2;
const CMD_COPY_IP: u32 = 3;
const CMD_AUTOSTART: u32 = 4;
const CMD_EXIT: u32 = 5;
const CMD_OPACITY_BASE: u32 = 100;

/// Current values the menu reflects.
pub struct MenuState {
    pub topmost: bool,
    pub opacity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    ToggleTopmost,
    Opacity(u8),
    Exit,
}

/// Owned popup menu handle.
struct Menu(HMENU);

impl Menu {
    fn new() -> Option<Self> {
        // SAFETY: plain FFI call.
        unsafe { CreatePopupMenu().ok().map(Self) }
    }

    fn item(&self, id: u32, label: PCWSTR, flags: MENU_ITEM_FLAGS) {
        // SAFETY: the menu is valid and `label` is a static wide string.
        unsafe {
            let _ = AppendMenuW(self.0, MF_STRING | flags, id as usize, label);
        }
    }

    fn separator(&self) {
        // SAFETY: as in `item`.
        unsafe {
            let _ = AppendMenuW(self.0, MF_SEPARATOR, 0, PCWSTR::null());
        }
    }

    /// Appends `sub` as a submenu; the parent then owns and destroys it.
    fn submenu(&self, sub: Menu, label: PCWSTR) {
        // SAFETY: as in `item`. On success ownership moves to the parent menu.
        unsafe {
            if AppendMenuW(self.0, MF_POPUP, sub.0 .0 as usize, label).is_ok() {
                std::mem::forget(sub);
            }
        }
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: we own the menu (and any attached submenus) and destroy it once.
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}

fn build(state: &MenuState) -> Option<Menu> {
    let menu = Menu::new()?;
    let checked = |on: bool| if on { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };

    menu.item(CMD_TOPMOST, w!("Always on top"), checked(state.topmost));

    let opacity = Menu::new()?;
    let labels = [w!("100 %"), w!("85 %"), w!("70 %")];
    for (i, (level, label)) in OPACITY_LEVELS.iter().zip(labels).enumerate() {
        opacity.item(CMD_OPACITY_BASE + i as u32, label, MENU_ITEM_FLAGS(0));
        if *level == state.opacity {
            let (first, last) = (
                CMD_OPACITY_BASE,
                CMD_OPACITY_BASE + OPACITY_LEVELS.len() as u32 - 1,
            );
            // SAFETY: the submenu is valid; ids are in range.
            unsafe {
                let _ = CheckMenuRadioItem(
                    opacity.0,
                    first,
                    last,
                    CMD_OPACITY_BASE + i as u32,
                    MF_BYCOMMAND.0,
                );
            }
        }
    }
    menu.submenu(opacity, w!("Opacity"));

    menu.separator();
    // Network actions arrive with M3, autostart with M4.
    menu.item(CMD_REFRESH_IP, w!("Refresh IP now"), MF_GRAYED);
    menu.item(CMD_COPY_IP, w!("Copy IP to clipboard"), MF_GRAYED);
    menu.separator();
    menu.item(CMD_AUTOSTART, w!("Start with Windows"), MF_GRAYED);
    menu.separator();
    menu.item(CMD_EXIT, w!("Exit"), MENU_ITEM_FLAGS(0));
    Some(menu)
}

fn command(id: u32) -> Option<Command> {
    match id {
        CMD_TOPMOST => Some(Command::ToggleTopmost),
        CMD_EXIT => Some(Command::Exit),
        _ => {
            let index = id.checked_sub(CMD_OPACITY_BASE)? as usize;
            OPACITY_LEVELS.get(index).copied().map(Command::Opacity)
        }
    }
}

/// Shows the menu at `pt` (screen coordinates), or at the cursor when `pt` is
/// `(-1, -1)` (keyboard / tray), and returns the chosen command.
pub fn show(hwnd: HWND, mut pt: POINT, state: &MenuState) -> Option<Command> {
    let menu = build(state)?;
    // SAFETY: all calls operate on handles owned by this (UI) thread.
    let id = unsafe {
        if pt.x == -1 && pt.y == -1 {
            let _ = GetCursorPos(&mut pt);
        }
        // Required so the menu closes when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let id = TrackPopupMenu(
            menu.0,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        // Documented workaround so a second tray click opens the menu again.
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        id.0 as u32
    };
    command(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ids_to_commands() {
        assert_eq!(command(CMD_TOPMOST), Some(Command::ToggleTopmost));
        assert_eq!(command(CMD_EXIT), Some(Command::Exit));
        assert_eq!(command(CMD_OPACITY_BASE), Some(Command::Opacity(100)));
        assert_eq!(command(CMD_OPACITY_BASE + 2), Some(Command::Opacity(70)));
        assert_eq!(command(CMD_OPACITY_BASE + 3), None);
        assert_eq!(command(0), None);
        assert_eq!(command(CMD_COPY_IP), None);
    }
}
