//! User interface: panel window, close button, painting, tray and menus.
//! Visual spec: design/WinHelper.bl.

mod close_button;
#[cfg(debug_assertions)]
mod demo;
mod gdi;
mod layout;
mod menu;
mod paint;
mod state;
mod theme;
mod tray;
mod view;
pub mod window;

/// Pixel metrics are authored at 96 DPI (100 % scaling).
pub const BASE_DPI: u32 = 96;

/// Scales a 96-DPI pixel value to `dpi`, rounding to the nearest pixel.
pub fn scale(px: i32, dpi: u32) -> i32 {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi } as i64;
    let base = BASE_DPI as i64;
    let scaled = (px as i64 * dpi + base / 2 * (px as i64).signum()) / base;
    scaled as i32
}

#[cfg(test)]
mod tests {
    use super::scale;

    #[test]
    fn identity_at_96_dpi() {
        assert_eq!(scale(220, 96), 220);
        assert_eq!(scale(0, 96), 0);
    }

    #[test]
    fn scales_common_factors() {
        assert_eq!(scale(220, 120), 275); // 125 %
        assert_eq!(scale(220, 144), 330); // 150 %
        assert_eq!(scale(150, 192), 300); // 200 %
    }

    #[test]
    fn rounds_to_nearest() {
        assert_eq!(scale(1, 144), 2); // 1.5 -> 2
        assert_eq!(scale(3, 120), 4); // 3.75 -> 4
        assert_eq!(scale(-3, 120), -4);
    }

    #[test]
    fn zero_dpi_falls_back_to_base() {
        assert_eq!(scale(10, 0), 10);
    }
}
