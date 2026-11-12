//! Panel and close-button drawing, following design/WinHelper.bl.

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_RIGHT, FW_NORMAL, FW_SEMIBOLD, HDC,
};

use super::gdi::{fill, frame, round_rect, text, text_width, Font};
use super::layout::*;
use super::scale;
use super::theme::{Load, Palette};
use super::view::{memory_text, percent_text, round_percent, PanelView};

/// Fonts for one DPI; recreated on WM_DPICHANGED.
pub struct Fonts {
    /// Segoe UI 9 pt semibold: row labels.
    label: Font,
    /// Segoe UI 9 pt: location.
    text: Font,
    /// Segoe UI 10 px semibold: country code.
    tag: Font,
    /// Consolas 12 px: percentages and IPs.
    value: Font,
    /// Consolas 11 px: RAM detail line.
    detail: Font,
    /// Segoe MDL2 Assets: the Windows close glyph.
    glyph: Font,
}

impl Fonts {
    pub fn new(dpi: u32) -> Self {
        let px = |v| scale(v, dpi);
        let (normal, semibold) = (FW_NORMAL.0, FW_SEMIBOLD.0);
        Self {
            label: Font::new(w!("Segoe UI"), px(12), semibold),
            text: Font::new(w!("Segoe UI"), px(12), normal),
            tag: Font::new(w!("Segoe UI"), px(10), semibold),
            value: Font::new(w!("Consolas"), px(12), normal),
            detail: Font::new(w!("Consolas"), px(11), normal),
            glyph: Font::new(w!("Segoe MDL2 Assets"), px(CLOSE_GLYPH), normal),
        }
    }
}

/// Converts 96-DPI design coordinates to device pixels.
struct Px {
    dpi: u32,
}

impl Px {
    fn v(&self, v: i32) -> i32 {
        scale(v, self.dpi)
    }

    /// Horizontal span [left, right) and the design row starting at `top`, `h` tall.
    fn row(&self, left: i32, right: i32, top: i32, h: i32) -> RECT {
        RECT {
            left,
            right,
            top: self.v(top),
            bottom: self.v(top + h),
        }
    }
}

pub struct PanelPaint<'a> {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub palette: &'a Palette,
    pub fonts: &'a Fonts,
    pub view: &'a PanelView,
    pub hovered: bool,
}

impl PanelPaint<'_> {
    pub fn draw(&self, dc: HDC) {
        let p = self.palette;
        let px = Px { dpi: self.dpi };
        let client = RECT {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        };
        fill(dc, &client, p.panel);
        frame(
            dc,
            &client,
            if self.hovered {
                p.border_hover
            } else {
                p.border
            },
        );

        let cols = Columns::new(&px, self.width);
        let v = self.view;
        self.metric_row(dc, &px, &cols, ROW_CPU, "CPU", v.cpu);
        self.metric_row(dc, &px, &cols, ROW_RAM, "RAM", v.ram.map(|m| m.percent()));
        if let Some(mem) = v.ram {
            let r = px.row(cols.content, cols.right, ROW_RAM_DETAIL, RAM_DETAIL_HEIGHT);
            text(
                dc,
                &self.fonts.detail,
                p.secondary,
                &r,
                &memory_text(mem),
                DT_LEFT,
            );
        }
        self.metric_row(dc, &px, &cols, ROW_GPU, "GPU", v.gpu);

        let sep = RECT {
            left: cols.left,
            right: cols.right,
            top: px.v(SEPARATOR),
            bottom: px.v(SEPARATOR) + px.v(1).max(1),
        };
        fill(dc, &sep, p.track);

        let net = &v.net;
        self.net_row(dc, &px, &cols, ROW_IP, "IP", net.public_ip.as_deref());
        self.net_row(dc, &px, &cols, ROW_LAN, "LAN", net.lan_ip.as_deref());
        self.location_row(dc, &px, &cols);
    }

    fn label(&self, dc: HDC, px: &Px, cols: &Columns, top: i32, label: &str, color: COLORREF) {
        let r = px.row(cols.left, cols.label_right, top, ROW_HEIGHT);
        text(dc, &self.fonts.label, color, &r, label, DT_LEFT);
    }

    fn metric_row(
        &self,
        dc: HDC,
        px: &Px,
        cols: &Columns,
        top: i32,
        label: &str,
        value: Option<f32>,
    ) {
        let p = self.palette;
        self.label(dc, px, cols, top, label, p.label);

        let row = px.row(cols.content, cols.bar_right, top, ROW_HEIGHT);
        let bar_h = px.v(BAR_HEIGHT);
        let bar_top = row.top + (row.bottom - row.top - bar_h) / 2;
        let track = RECT {
            top: bar_top,
            bottom: bar_top + bar_h,
            ..row
        };
        let radius = px.v(BAR_RADIUS);
        let value_rect = px.row(cols.value_left, cols.right, top, ROW_HEIGHT);

        match value {
            Some(v) => {
                let pct = round_percent(v);
                let load = Load::of(pct);
                round_rect(dc, &track, radius, p.track);
                let filled = RECT {
                    right: track.left + bar_fill(track.right - track.left, pct),
                    ..track
                };
                round_rect(dc, &filled, radius, load.bar_color(p));
                let s = percent_text(Some(v));
                text(
                    dc,
                    &self.fonts.value,
                    load.text_color(p),
                    &value_rect,
                    &s,
                    DT_RIGHT,
                );
            }
            None => {
                round_rect(dc, &track, radius, p.track_na);
                text(
                    dc,
                    &self.fonts.value,
                    p.stale_value,
                    &value_rect,
                    "N/A",
                    DT_RIGHT,
                );
            }
        }
    }

    /// (label color, value color) for network rows.
    fn net_colors(&self) -> (COLORREF, COLORREF) {
        let p = self.palette;
        if self.view.net.stale {
            (p.stale_label, p.stale_value)
        } else {
            (p.label, p.value)
        }
    }

    fn net_row(
        &self,
        dc: HDC,
        px: &Px,
        cols: &Columns,
        top: i32,
        label: &str,
        value: Option<&str>,
    ) {
        let (label_color, value_color) = self.net_colors();
        self.label(dc, px, cols, top, label, label_color);
        let r = px.row(cols.content, cols.right, top, ROW_HEIGHT);
        let (s, color) = match value {
            Some(s) => (s, value_color),
            None => ("—", self.palette.stale_value),
        };
        text(
            dc,
            &self.fonts.value,
            color,
            &r,
            s,
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }

    fn location_row(&self, dc: HDC, px: &Px, cols: &Columns) {
        let net = &self.view.net;
        let (label_color, value_color) = self.net_colors();
        self.label(dc, px, cols, ROW_LOC, "LOC", label_color);

        let row = px.row(cols.content, cols.right, ROW_LOC, ROW_HEIGHT);
        let Some(location) = net.location.as_deref() else {
            text(
                dc,
                &self.fonts.text,
                self.palette.stale_value,
                &row,
                "—",
                DT_LEFT,
            );
            return;
        };

        // The country code follows the location; it is hidden while stale.
        let code = net.country_code.as_deref().filter(|_| !net.stale);
        let code_w = code.map_or(0, |c| text_width(dc, &self.fonts.tag, c));
        let gap = if code.is_some() { px.v(COUNTRY_GAP) } else { 0 };
        let avail = (row.right - row.left - code_w - gap).max(0);
        let loc_w = text_width(dc, &self.fonts.text, location).min(avail);
        let loc_rect = RECT {
            right: row.left + loc_w,
            ..row
        };
        let flags = DT_LEFT | DT_END_ELLIPSIS;
        text(
            dc,
            &self.fonts.text,
            value_color,
            &loc_rect,
            location,
            flags,
        );

        if let Some(code) = code {
            let code_rect = RECT {
                left: loc_rect.right + gap,
                ..row
            };
            text(
                dc,
                &self.fonts.tag,
                self.palette.country,
                &code_rect,
                code,
                DT_LEFT,
            );
        }
    }
}

/// Horizontal columns of a panel row, in device pixels.
struct Columns {
    left: i32,
    label_right: i32,
    /// Start of the bar / value column.
    content: i32,
    bar_right: i32,
    value_left: i32,
    right: i32,
}

impl Columns {
    fn new(px: &Px, width: i32) -> Self {
        let left = px.v(PAD_X);
        let right = width - px.v(PAD_X);
        let value_left = right - px.v(VALUE_WIDTH);
        Self {
            left,
            label_right: left + px.v(LABEL_WIDTH),
            content: left + px.v(LABEL_WIDTH + COL_GAP),
            bar_right: value_left - px.v(COL_GAP),
            value_left,
            right,
        }
    }
}

/// Close button: idle shows a bordered square in the panel colors, hover the
/// Windows 11 close red with a white glyph.
pub fn close_button(dc: HDC, size: i32, palette: &Palette, fonts: &Fonts, hovered: bool) {
    let r = RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    let glyph_color = if hovered {
        fill(dc, &r, palette.close_hover);
        palette.close_hover_glyph
    } else {
        fill(dc, &r, palette.close_bg);
        frame(dc, &r, palette.close_border);
        palette.close_glyph
    };
    // U+E8BB ChromeClose: the glyph Windows uses on caption close buttons.
    text(dc, &fonts.glyph, glyph_color, &r, "\u{E8BB}", DT_CENTER);
}
