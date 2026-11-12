//! Panel geometry from design/WinHelper.bl, in 96-DPI pixels, plus the pure
//! placement math for the close button.

use windows::Win32::Foundation::{POINT, RECT};

pub const PANEL_WIDTH: i32 = 220;
pub const PANEL_HEIGHT: i32 = 174;

pub const PAD_X: i32 = 12;
/// Label column width; the bar / value column starts `LABEL_WIDTH + COL_GAP` in.
pub const LABEL_WIDTH: i32 = 28;
pub const COL_GAP: i32 = 8;
/// Right-aligned percentage column.
pub const VALUE_WIDTH: i32 = 38;
pub const ROW_HEIGHT: i32 = 16;

pub const BAR_HEIGHT: i32 = 6;
pub const BAR_RADIUS: i32 = 3;

/// Row tops relative to the panel's top edge.
pub const ROW_CPU: i32 = 10;
pub const ROW_RAM: i32 = 33;
/// RAM "used / total" line, 13 px tall, under the RAM bar.
pub const ROW_RAM_DETAIL: i32 = 51;
pub const RAM_DETAIL_HEIGHT: i32 = 13;
pub const ROW_GPU: i32 = 71;
pub const SEPARATOR: i32 = 94;
pub const ROW_IP: i32 = 102;
pub const ROW_LAN: i32 = 125;
pub const ROW_LOC: i32 = 148;
/// Gap between the location text and the country code.
pub const COUNTRY_GAP: i32 = 8;

/// Close button: square, outside the panel, top-aligned with it.
pub const CLOSE_SIZE: i32 = 24;
pub const CLOSE_GAP: i32 = 6;
pub const CLOSE_GLYPH: i32 = 12;

/// Filled part of a bar `track` pixels wide at `percent` (0–100).
pub fn bar_fill(track: i32, percent: u32) -> i32 {
    (track as i64 * percent.min(100) as i64 / 100) as i32
}

/// Top-left of the close button: `gap` px right of the panel, or left of it when
/// the right side would leave the monitor's work area.
pub fn close_button_origin(panel: &RECT, work: &RECT, size: i32, gap: i32) -> POINT {
    let right_x = panel.right + gap;
    let x = if right_x + size <= work.right {
        right_x
    } else {
        panel.left - gap - size
    };
    POINT { x, y: panel.top }
}

fn contains(r: &RECT, pt: POINT) -> bool {
    pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
}

/// Whether the cursor still counts as "over the panel" for keeping the close
/// button visible: on the panel, on the button, or crossing the gap between them.
pub fn in_hover_zone(pt: POINT, panel: &RECT, button: &RECT) -> bool {
    let bridge = RECT {
        left: panel.right.min(button.right),
        right: panel.left.max(button.left),
        top: button.top,
        bottom: button.bottom,
    };
    contains(panel, pt) || contains(button, pt) || contains(&bridge, pt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn track_width_matches_design() {
        let track = PANEL_WIDTH - 2 * PAD_X - LABEL_WIDTH - 2 * COL_GAP - VALUE_WIDTH;
        assert_eq!(track, 114);
    }

    #[test]
    fn bar_fill_scales_and_clamps() {
        assert_eq!(bar_fill(114, 0), 0);
        assert_eq!(bar_fill(114, 42), 47);
        assert_eq!(bar_fill(114, 100), 114);
        assert_eq!(bar_fill(114, 250), 114);
    }

    #[test]
    fn close_button_goes_right_when_it_fits() {
        let panel = rect(100, 50, 320, 224);
        let work = rect(0, 0, 1920, 1040);
        let pt = close_button_origin(&panel, &work, 24, 6);
        assert_eq!((pt.x, pt.y), (326, 50));
    }

    #[test]
    fn close_button_flips_left_near_screen_edge() {
        let panel = rect(1684, 16, 1904, 190);
        let work = rect(0, 0, 1920, 1040);
        let pt = close_button_origin(&panel, &work, 24, 6);
        assert_eq!((pt.x, pt.y), (1654, 16));
    }

    #[test]
    fn hover_zone_bridges_gap_on_either_side() {
        let panel = rect(100, 50, 320, 224);
        let right = rect(326, 50, 350, 74);
        assert!(in_hover_zone(POINT { x: 200, y: 200 }, &panel, &right));
        assert!(in_hover_zone(POINT { x: 323, y: 60 }, &panel, &right));
        assert!(in_hover_zone(POINT { x: 340, y: 60 }, &panel, &right));
        assert!(!in_hover_zone(POINT { x: 323, y: 100 }, &panel, &right));
        assert!(!in_hover_zone(POINT { x: 360, y: 60 }, &panel, &right));

        let left = rect(70, 50, 94, 74);
        assert!(in_hover_zone(POINT { x: 97, y: 60 }, &panel, &left));
        assert!(!in_hover_zone(POINT { x: 97, y: 120 }, &panel, &left));
    }
}
