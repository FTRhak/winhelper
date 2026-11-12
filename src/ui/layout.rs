//! Strip and tooltip geometry from design/WinHelper.bl ("WinHelper · Minimal"),
//! in 96-DPI pixels, plus the pure placement math for the popups.

use windows::Win32::Foundation::{POINT, RECT};

// ---- Strip -----------------------------------------------------------------

pub const STRIP_HEIGHT: i32 = 30;
pub const PAD_X: i32 = 12;
/// Space between items (metric, separator, globe, rates).
pub const ITEM_GAP: i32 = 12;

/// Metric icon box and the gap before its value.
pub const ICON: i32 = 14;
pub const ICON_GAP: i32 = 5;
/// Value slot wide enough for "100%" in Consolas 12 px, so the strip never shifts.
pub const PERCENT_SLOT: i32 = 27;

/// Separator between the system and network groups.
pub const SEPARATOR_HEIGHT: i32 = 14;

/// Status dot after the globe.
pub const DOT: i32 = 6;

/// Rate item: arrow, gap, number (right-aligned, "24.8"), gap, unit ("KB/s").
pub const ARROW: i32 = 12;
pub const RATE_GAP: i32 = 3;
pub const RATE_SLOT: i32 = 27;
pub const UNIT_SLOT: i32 = 22;

pub const METRIC_ITEM: i32 = ICON + ICON_GAP + PERCENT_SLOT;
pub const GLOBE_ITEM: i32 = ICON + ICON_GAP + DOT;
pub const RATE_ITEM: i32 = ARROW + RATE_GAP + RATE_SLOT + RATE_GAP + UNIT_SLOT;

/// CPU, RAM, GPU | globe, down, up.
pub const STRIP_WIDTH: i32 =
    2 * PAD_X + 3 * METRIC_ITEM + 1 + GLOBE_ITEM + 2 * RATE_ITEM + 6 * ITEM_GAP;

// ---- Close button ----------------------------------------------------------

/// Square, outside the strip, vertically centered on it.
pub const CLOSE_SIZE: i32 = 22;
pub const CLOSE_GAP: i32 = 6;
pub const CLOSE_GLYPH: i32 = 11;

// ---- Tooltip ---------------------------------------------------------------

pub const TIP_WIDTH: i32 = 264;
pub const TIP_PAD_X: i32 = 14;
pub const TIP_PAD_Y: i32 = 12;
/// Gap between the strip and the tooltip.
pub const TIP_GAP: i32 = 6;

pub const TIP_HEADER_H: i32 = 13;
pub const TIP_ROW_H: i32 = 16;
pub const TIP_ICON: i32 = 13;
/// Label offset from the icon's left edge.
pub const TIP_LABEL_X: i32 = 19;
pub const TIP_BAR_H: i32 = 3;
pub const TIP_BAR_GAP: i32 = 4;
pub const TIP_METRIC_GAP: i32 = 8;
pub const TIP_NET_ROW_GAP: i32 = 7;
pub const TIP_SECTION_GAP: i32 = 12;
pub const TIP_FOOTER_H: i32 = 14;
/// Gap between the RAM "used / total" detail and its percentage.
pub const TIP_DETAIL_GAP: i32 = 8;
/// Country tag: horizontal padding and gap before it.
pub const TIP_TAG_PAD: i32 = 4;
pub const TIP_TAG_GAP: i32 = 6;
pub const TIP_TAG_H: i32 = 14;

pub const TIP_METRIC_H: i32 = TIP_ROW_H + TIP_BAR_GAP + TIP_BAR_H;
pub const TIP_NET_ROWS: i32 = 5;

/// Row tops relative to the tooltip's top edge.
pub const TIP_SYS_HEADER: i32 = TIP_PAD_Y;
pub const TIP_METRIC0: i32 = TIP_SYS_HEADER + TIP_HEADER_H + TIP_METRIC_GAP;
pub const TIP_METRIC_STEP: i32 = TIP_METRIC_H + TIP_METRIC_GAP;
pub const TIP_SEP1: i32 = TIP_METRIC0 + 3 * TIP_METRIC_H + 2 * TIP_METRIC_GAP + TIP_SECTION_GAP;
pub const TIP_NET_HEADER: i32 = TIP_SEP1 + 1 + TIP_SECTION_GAP;
pub const TIP_NET_ROW0: i32 = TIP_NET_HEADER + TIP_ROW_H + TIP_NET_ROW_GAP;
pub const TIP_NET_STEP: i32 = TIP_ROW_H + TIP_NET_ROW_GAP;
pub const TIP_SEP2: i32 = TIP_NET_ROW0
    + TIP_NET_ROWS * TIP_ROW_H
    + (TIP_NET_ROWS - 1) * TIP_NET_ROW_GAP
    + TIP_SECTION_GAP;
pub const TIP_FOOTER: i32 = TIP_SEP2 + 1 + TIP_SECTION_GAP;
pub const TIP_HEIGHT: i32 = TIP_FOOTER + TIP_FOOTER_H + TIP_PAD_Y;

// ---- Math ------------------------------------------------------------------

/// Filled part of a bar `track` pixels wide at `percent` (0–100).
pub fn bar_fill(track: i32, percent: u32) -> i32 {
    (track as i64 * percent.min(100) as i64 / 100) as i32
}

/// Top-left of the close button: `gap` px right of the strip, or left of it
/// when the right side would leave the monitor's work area; centered vertically.
pub fn close_button_origin(panel: &RECT, work: &RECT, size: i32, gap: i32) -> POINT {
    let right_x = panel.right + gap;
    let x = if right_x + size <= work.right {
        right_x
    } else {
        panel.left - gap - size
    };
    let y = panel.top + (panel.bottom - panel.top - size) / 2;
    POINT { x, y }
}

/// Top-left of the tooltip: below the strip, or above it when it would leave
/// the work area at the bottom; left-aligned with the strip but kept inside
/// the work area horizontally.
pub fn tip_origin(panel: &RECT, work: &RECT, width: i32, height: i32, gap: i32) -> POINT {
    let below = panel.bottom + gap;
    let y = if below + height <= work.bottom {
        below
    } else {
        panel.top - gap - height
    };
    let x = panel.left.min(work.right - width).max(work.left);
    POINT { x, y }
}

fn contains(r: &RECT, pt: POINT) -> bool {
    pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
}

/// Whether the cursor still counts as "over the strip" for keeping the close
/// button visible: on the strip, on the button, or crossing the gap between them.
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
    fn strip_and_tip_sizes_match_design() {
        assert_eq!(STRIP_WIDTH, 394);
        assert_eq!(TIP_SEP1, 130);
        assert_eq!(TIP_NET_HEADER, 143);
        assert_eq!(TIP_SEP2, 286);
        assert_eq!(TIP_HEIGHT, 325);
    }

    #[test]
    fn bar_fill_scales_and_clamps() {
        assert_eq!(bar_fill(236, 0), 0);
        assert_eq!(bar_fill(236, 42), 99);
        assert_eq!(bar_fill(236, 100), 236);
        assert_eq!(bar_fill(236, 250), 236);
    }

    #[test]
    fn close_button_goes_right_when_it_fits() {
        let panel = rect(100, 50, 494, 80);
        let work = rect(0, 0, 1920, 1040);
        let pt = close_button_origin(&panel, &work, 22, 6);
        assert_eq!((pt.x, pt.y), (500, 54));
    }

    #[test]
    fn close_button_flips_left_near_screen_edge() {
        let panel = rect(1510, 16, 1904, 46);
        let work = rect(0, 0, 1920, 1040);
        let pt = close_button_origin(&panel, &work, 22, 6);
        assert_eq!((pt.x, pt.y), (1482, 20));
    }

    #[test]
    fn tip_goes_below_then_flips_above() {
        let work = rect(0, 0, 1920, 1040);
        let top = rect(1510, 16, 1904, 46);
        let pt = tip_origin(&top, &work, 264, 325, 6);
        assert_eq!((pt.x, pt.y), (1510, 52));

        let bottom = rect(100, 990, 494, 1020);
        let pt = tip_origin(&bottom, &work, 264, 325, 6);
        assert_eq!((pt.x, pt.y), (100, 659));
    }

    #[test]
    fn tip_stays_inside_work_area_horizontally() {
        let work = rect(0, 0, 1920, 1040);
        let off_right = rect(1800, 16, 2194, 46);
        assert_eq!(tip_origin(&off_right, &work, 264, 325, 6).x, 1656);
        let off_left = rect(-50, 16, 344, 46);
        assert_eq!(tip_origin(&off_left, &work, 264, 325, 6).x, 0);
    }

    #[test]
    fn hover_zone_bridges_gap_on_either_side() {
        let panel = rect(100, 50, 494, 80);
        let right = rect(500, 54, 522, 76);
        assert!(in_hover_zone(POINT { x: 200, y: 70 }, &panel, &right));
        assert!(in_hover_zone(POINT { x: 497, y: 60 }, &panel, &right));
        assert!(in_hover_zone(POINT { x: 510, y: 60 }, &panel, &right));
        assert!(!in_hover_zone(POINT { x: 497, y: 78 }, &panel, &right));
        assert!(!in_hover_zone(POINT { x: 530, y: 60 }, &panel, &right));

        let left = rect(72, 54, 94, 76);
        assert!(in_hover_zone(POINT { x: 97, y: 60 }, &panel, &left));
        assert!(!in_hover_zone(POINT { x: 97, y: 52 }, &panel, &left));
    }
}
