//! Light / dark palettes (design/WinHelper.bl, "Palette — Dark/Light"), load
//! thresholds and system theme detection.

use windows::core::{s, w, PCSTR};
use windows::Win32::Foundation::{COLORREF, ERROR_SUCCESS};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

/// Packs an `0xRRGGBB` literal into a GDI `COLORREF` (`0x00BBGGRR`).
pub const fn hex(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

/// Every color the panel and its close button paint.
pub struct Palette {
    pub panel: COLORREF,
    pub border: COLORREF,
    pub border_hover: COLORREF,
    /// Row labels: CPU, RAM, IP, ...
    pub label: COLORREF,
    /// Numbers, IPs, location.
    pub value: COLORREF,
    /// RAM "used / total" line.
    pub secondary: COLORREF,
    pub stale_label: COLORREF,
    /// Stale network values, "N/A", "—".
    pub stale_value: COLORREF,
    /// Bar background and section separator.
    pub track: COLORREF,
    pub track_na: COLORREF,
    pub normal: COLORREF,
    pub elevated: COLORREF,
    pub high: COLORREF,
    /// Percentage text above the high threshold.
    pub high_value: COLORREF,
    pub country: COLORREF,
    pub close_bg: COLORREF,
    pub close_border: COLORREF,
    pub close_glyph: COLORREF,
    pub close_hover: COLORREF,
    pub close_hover_glyph: COLORREF,
}

pub const DARK: Palette = Palette {
    panel: hex(0x202020),
    border: hex(0x3A3A3A),
    border_hover: hex(0x4A4A4A),
    label: hex(0x9A9A9A),
    value: hex(0xE6E6E6),
    secondary: hex(0x8A8A8A),
    stale_label: hex(0x5E5E5E),
    stale_value: hex(0x6E6E6E),
    track: hex(0x3A3A3A),
    track_na: hex(0x2C2C2C),
    normal: hex(0x3FB950),
    elevated: hex(0xE3A21A),
    high: hex(0xF85149),
    high_value: hex(0xFF7B72),
    country: hex(0x8CB4FF),
    close_bg: hex(0x202020),
    close_border: hex(0x3A3A3A),
    close_glyph: hex(0xC8C8C8),
    close_hover: hex(0xC42B1C),
    close_hover_glyph: hex(0xFFFFFF),
};

pub const LIGHT: Palette = Palette {
    panel: hex(0xF9F9F9),
    border: hex(0xD6D6D6),
    border_hover: hex(0xC4C4C4),
    label: hex(0x616161),
    value: hex(0x1B1B1B),
    secondary: hex(0x707070),
    stale_label: hex(0xA8A8A8),
    stale_value: hex(0x9A9A9A),
    track: hex(0xE1E1E1),
    track_na: hex(0xEDEDED),
    normal: hex(0x2DA44E),
    elevated: hex(0xD18E00),
    high: hex(0xCF222E),
    high_value: hex(0xCF222E),
    country: hex(0x1F5BD6),
    close_bg: hex(0xF9F9F9),
    close_border: hex(0xC4C4C4),
    close_glyph: hex(0x444444),
    close_hover: hex(0xC42B1C),
    close_hover_glyph: hex(0xFFFFFF),
};

impl Theme {
    pub fn palette(self) -> &'static Palette {
        match self {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

/// Load level of a percentage, driving bar and text colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    /// Below 60 %.
    Normal,
    /// 60 – 85 % inclusive.
    Elevated,
    /// Above 85 %.
    High,
}

impl Load {
    pub fn of(percent: u32) -> Self {
        match percent {
            0..=59 => Load::Normal,
            60..=85 => Load::Elevated,
            _ => Load::High,
        }
    }

    pub fn bar_color(self, p: &Palette) -> COLORREF {
        match self {
            Load::Normal => p.normal,
            Load::Elevated => p.elevated,
            Load::High => p.high,
        }
    }

    pub fn text_color(self, p: &Palette) -> COLORREF {
        match self {
            Load::High => p.high_value,
            _ => p.value,
        }
    }
}

/// Reads the app theme from `AppsUseLightTheme`. A missing value means light,
/// matching Windows' own default.
pub fn system_theme() -> Theme {
    #[cfg(debug_assertions)]
    if let Some(theme) = super::demo::theme_override() {
        return theme;
    }

    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: the output buffer is a u32 and `size` describes it.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS && value == 0 {
        Theme::Dark
    } else {
        Theme::Light
    }
}

/// Switches this process's popup menus to the matching dark / light rendering.
///
/// Uses the undocumented but long-stable uxtheme ordinals 135 (`SetPreferredAppMode`,
/// Windows 10 1903+) and 136 (`FlushMenuThemes`). When they are missing, menus
/// simply stay in the default light style.
pub fn apply_menu_theme(theme: Theme) {
    const FORCE_DARK: i32 = 2;
    const FORCE_LIGHT: i32 = 3;
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();

    // SAFETY: uxtheme stays loaded for the process lifetime (the module handle is
    // intentionally never freed), and the ordinals have had these signatures since 1903.
    unsafe {
        let Ok(uxtheme) = LoadLibraryExW(w!("uxtheme.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        else {
            return;
        };
        let by_ordinal = |n: usize| GetProcAddress(uxtheme, PCSTR(n as *const u8));
        // Guard against very old builds where ordinal 135 means something else.
        if GetProcAddress(uxtheme, s!("OpenThemeDataForDpi")).is_none() {
            return;
        }
        if let Some(set_mode) = by_ordinal(135) {
            let set_mode: SetPreferredAppMode = std::mem::transmute(set_mode);
            set_mode(match theme {
                Theme::Dark => FORCE_DARK,
                Theme::Light => FORCE_LIGHT,
            });
        }
        if let Some(flush) = by_ordinal(136) {
            let flush: FlushMenuThemes = std::mem::transmute(flush);
            flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_packs_as_colorref() {
        assert_eq!(hex(0x123456).0, 0x0056_3412);
        assert_eq!(hex(0xC42B1C).0, 0x001C_2BC4);
    }

    #[test]
    fn load_thresholds_match_design() {
        assert_eq!(Load::of(0), Load::Normal);
        assert_eq!(Load::of(59), Load::Normal);
        assert_eq!(Load::of(60), Load::Elevated);
        assert_eq!(Load::of(85), Load::Elevated);
        assert_eq!(Load::of(86), Load::High);
        assert_eq!(Load::of(100), Load::High);
    }

    #[test]
    fn high_load_uses_highlighted_text_only_in_high() {
        let p = &DARK;
        assert_eq!(Load::Elevated.text_color(p), p.value);
        assert_eq!(Load::High.text_color(p), p.high_value);
        assert_eq!(Load::High.bar_color(&LIGHT), LIGHT.high);
    }
}
