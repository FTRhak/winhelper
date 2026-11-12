//! Strip, tooltip and close-button drawing, following design/WinHelper.bl
//! ("WinHelper · Minimal").

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_RIGHT, FW_NORMAL, FW_SEMIBOLD, HDC,
};

use super::gdi::{fill, frame, round_rect, text, text_width, Font};
use super::layout::*;
use super::scale;
use super::theme::{Heat, Load, Palette};
use super::view::{
    compact_percent, compact_temp, disk_rates_text, footer_text, memory_text, percent_text,
    rate_parts, rate_text, round_celsius, round_percent, temp_text, PanelView, Status,
};

/// Segoe MDL2 Assets code points (also present in Segoe Fluent Icons).
mod glyph {
    pub const CPU: &str = "\u{E950}"; // Processing
    pub const RAM: &str = "\u{E964}"; // Memory
    pub const GPU: &str = "\u{E7F4}"; // TVMonitor
    pub const DISK: &str = "\u{EDA2}"; // HardDrive
    pub const GLOBE: &str = "\u{E774}";
    pub const DOWN: &str = "\u{E74B}";
    pub const UP: &str = "\u{E74A}";
    /// ChromeClose: the glyph Windows uses on caption close buttons.
    pub const CLOSE: &str = "\u{E8BB}";
}

/// Fonts for one DPI; recreated on WM_DPICHANGED.
pub struct Fonts {
    /// MDL2 14 px: strip metric and globe icons.
    icon: Font,
    /// MDL2 13 px: tooltip metric icons.
    tip_icon: Font,
    /// MDL2 12 px: rate arrows.
    arrow: Font,
    /// MDL2: the close glyph.
    close: Font,
    /// Consolas 12 px: percentages, rates, IPs.
    value: Font,
    /// Consolas 11 px: RAM "used / total".
    detail: Font,
    /// Consolas 10 px: rate units on the strip.
    unit: Font,
    /// Segoe UI 12 px: tooltip labels and location.
    text: Font,
    /// Segoe UI 11 px: status and footer.
    small: Font,
    /// Segoe UI 10 px semibold: section headers and the country tag.
    caption: Font,
    /// Segoe UI sized so "●" is a `DOT`-wide status dot.
    dot: Font,
}

impl Fonts {
    pub fn new(dpi: u32) -> Self {
        let px = |v| scale(v, dpi);
        let (normal, semibold) = (FW_NORMAL.0, FW_SEMIBOLD.0);
        let mdl2 = w!("Segoe MDL2 Assets");
        Self {
            icon: Font::new(mdl2, px(ICON), normal),
            tip_icon: Font::new(mdl2, px(TIP_ICON), normal),
            arrow: Font::new(mdl2, px(ARROW), normal),
            close: Font::new(mdl2, px(CLOSE_GLYPH), normal),
            value: Font::new(w!("Consolas"), px(12), normal),
            detail: Font::new(w!("Consolas"), px(11), normal),
            unit: Font::new(w!("Consolas"), px(10), normal),
            text: Font::new(w!("Segoe UI"), px(12), normal),
            small: Font::new(w!("Segoe UI"), px(11), normal),
            caption: Font::new(w!("Segoe UI"), px(10), semibold),
            dot: Font::new(w!("Segoe UI"), px(DOT + 4), normal),
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

    /// Device rect for design coordinates `[left, left + w) × [top, top + h)`.
    fn rect(&self, left: i32, top: i32, w: i32, h: i32) -> RECT {
        RECT {
            left: self.v(left),
            top: self.v(top),
            right: self.v(left + w),
            bottom: self.v(top + h),
        }
    }
}

fn client(width: i32, height: i32) -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    }
}

/// Status dot centered in the device-pixel `box_` (DOT wide). Drawn as a text
/// glyph because GDI shapes are not anti-aliased and a 6 px circle turns into
/// a diamond.
fn dot(dc: HDC, fonts: &Fonts, box_: &RECT, color: COLORREF) {
    let pad = (box_.right - box_.left) / 2;
    let r = RECT {
        left: box_.left - pad,
        right: box_.right + pad,
        ..*box_
    };
    text(dc, &fonts.dot, color, &r, "\u{25CF}", DT_CENTER);
}

fn status_colors(p: &Palette, status: Status) -> (COLORREF, COLORREF) {
    // (globe / text, dot)
    match status {
        Status::Online => (p.label, p.normal),
        Status::Offline => (p.high_value, p.high),
        Status::Unknown => (p.stale_label, p.stale_value),
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Online => "Online",
        Status::Offline => "Offline",
        Status::Unknown => "Checking…",
    }
}

/// The always-visible one-line strip.
pub struct StripPaint<'a> {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub palette: &'a Palette,
    pub fonts: &'a Fonts,
    pub view: &'a PanelView,
    pub hovered: bool,
}

impl StripPaint<'_> {
    pub fn draw(&self, dc: HDC) {
        let p = self.palette;
        let px = Px { dpi: self.dpi };
        let client = client(self.width, self.height);
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

        let v = self.view;
        let mut x = PAD_X;
        for (icon, value, temp) in [
            (glyph::CPU, v.cpu, v.cpu_temp),
            (glyph::RAM, v.ram.map(|m| m.percent()), None),
            (glyph::GPU, v.gpu, v.gpu_temp),
            (glyph::DISK, v.disk.map(|d| d.active), None),
        ] {
            self.metric(dc, &px, x, icon, value);
            x += METRIC_ITEM;
            if let Some(celsius) = temp {
                let r = px.rect(x + TEMP_GAP, 0, TEMP_SLOT, STRIP_HEIGHT);
                let color = Heat::of(round_celsius(celsius)).text_color(p);
                text(
                    dc,
                    &self.fonts.unit,
                    color,
                    &r,
                    &compact_temp(celsius),
                    DT_LEFT,
                );
                x += TEMP_ITEM;
            }
            x += ITEM_GAP;
        }

        let sep = px.rect(
            x,
            (STRIP_HEIGHT - SEPARATOR_HEIGHT) / 2,
            1,
            SEPARATOR_HEIGHT,
        );
        fill(
            dc,
            &RECT {
                right: sep.left + px.v(1).max(1),
                ..sep
            },
            p.border,
        );
        x += 1 + ITEM_GAP;

        let status = v.status();
        let (globe_color, dot_color) = status_colors(p, status);
        let r = px.rect(x, 0, ICON, STRIP_HEIGHT);
        text(
            dc,
            &self.fonts.icon,
            globe_color,
            &r,
            glyph::GLOBE,
            DT_CENTER,
        );
        let r = px.rect(x + ICON + ICON_GAP, 0, DOT, STRIP_HEIGHT);
        dot(dc, self.fonts, &r, dot_color);
        x += GLOBE_ITEM + ITEM_GAP;

        let offline = status == Status::Offline;
        let traffic = v.traffic;
        self.rate(
            dc,
            &px,
            x,
            glyph::DOWN,
            p.normal,
            traffic.map(|t| t.down),
            offline,
        );
        x += RATE_ITEM + ITEM_GAP;
        self.rate(
            dc,
            &px,
            x,
            glyph::UP,
            p.country,
            traffic.map(|t| t.up),
            offline,
        );
    }

    fn metric(&self, dc: HDC, px: &Px, x: i32, icon: &str, value: Option<f32>) {
        let p = self.palette;
        let (icon_color, value_color) = match value {
            Some(v) => {
                let load = Load::of(round_percent(v));
                let icon = if load == Load::High {
                    p.high_value
                } else {
                    p.label
                };
                (icon, load.text_color(p))
            }
            None => (p.stale_label, p.stale_value),
        };
        let r = px.rect(x, 0, ICON, STRIP_HEIGHT);
        text(dc, &self.fonts.icon, icon_color, &r, icon, DT_CENTER);
        let r = px.rect(x + ICON + ICON_GAP, 0, PERCENT_SLOT, STRIP_HEIGHT);
        text(
            dc,
            &self.fonts.value,
            value_color,
            &r,
            &compact_percent(value),
            DT_LEFT,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn rate(
        &self,
        dc: HDC,
        px: &Px,
        x: i32,
        arrow: &str,
        arrow_color: COLORREF,
        rate: Option<f64>,
        offline: bool,
    ) {
        let p = self.palette;
        let dim = offline || rate.is_none();
        let r = px.rect(x, 0, ARROW, STRIP_HEIGHT);
        let color = if dim { p.stale_label } else { arrow_color };
        text(dc, &self.fonts.arrow, color, &r, arrow, DT_CENTER);

        let (number, unit) = match rate {
            Some(bps) => rate_parts(bps),
            None => ("–".to_owned(), ""),
        };
        let (number_color, unit_color) = if dim {
            (p.stale_value, p.stale_label)
        } else {
            (p.value, p.secondary)
        };
        let nx = x + ARROW + RATE_GAP;
        let r = px.rect(nx, 0, RATE_SLOT, STRIP_HEIGHT);
        text(dc, &self.fonts.value, number_color, &r, &number, DT_RIGHT);
        let r = px.rect(nx + RATE_SLOT + RATE_GAP, 0, UNIT_SLOT, STRIP_HEIGHT);
        text(dc, &self.fonts.unit, unit_color, &r, unit, DT_LEFT);
    }
}

/// The detail tooltip shown while hovering the strip.
pub struct TipPaint<'a> {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub palette: &'a Palette,
    pub fonts: &'a Fonts,
    pub view: &'a PanelView,
}

impl TipPaint<'_> {
    pub fn draw(&self, dc: HDC) {
        let p = self.palette;
        let px = Px { dpi: self.dpi };
        let client = client(self.width, self.height);
        fill(dc, &client, p.tip_bg);
        frame(dc, &client, p.tip_border);

        let inner = TIP_WIDTH - 2 * TIP_PAD_X;
        let v = self.view;

        let r = px.rect(TIP_PAD_X, TIP_SYS_HEADER, inner, TIP_HEADER_H);
        text(dc, &self.fonts.caption, p.secondary, &r, "SYSTEM", DT_LEFT);
        // Detail before the percentage: temperature, memory use, disk rates.
        let temp =
            |t: Option<f32>| t.map(|c| (temp_text(c), Heat::of(round_celsius(c)).text_color(p)));
        let metrics = [
            (glyph::CPU, "CPU", v.cpu, temp(v.cpu_temp)),
            (
                glyph::RAM,
                "RAM",
                v.ram.map(|m| m.percent()),
                v.ram.map(|m| (memory_text(m), p.secondary)),
            ),
            (glyph::GPU, "GPU", v.gpu, temp(v.gpu_temp)),
            (
                glyph::DISK,
                "Disk",
                v.disk.map(|d| d.active),
                v.disk
                    .map(|d| (disk_rates_text(d.read, d.write), p.secondary)),
            ),
        ];
        for (i, (icon, label, value, detail)) in metrics.into_iter().enumerate() {
            let top = TIP_METRIC0 + i as i32 * TIP_METRIC_STEP;
            self.metric(dc, &px, top, icon, label, value, detail);
        }

        self.separator(dc, &px, TIP_SEP1);

        // Network header with the status on the right.
        let status = v.status();
        let (status_color, dot_color) = status_colors(p, status);
        let status_color = if status == Status::Online {
            p.normal
        } else {
            status_color
        };
        let r = px.rect(TIP_PAD_X, TIP_NET_HEADER, inner, TIP_ROW_H);
        text(dc, &self.fonts.caption, p.secondary, &r, "NETWORK", DT_LEFT);
        let label = status_text(status);
        text(dc, &self.fonts.small, status_color, &r, label, DT_RIGHT);
        let w = text_width(dc, &self.fonts.small, label);
        let dot_right = r.right - w - px.v(5);
        let dot_r = RECT {
            left: dot_right - px.v(DOT),
            right: dot_right,
            ..r
        };
        dot(dc, self.fonts, &dot_r, dot_color);

        let net = &v.net;
        let (label_color, value_color) = if net.stale {
            (p.stale_label, p.stale_value)
        } else {
            (p.label, p.value)
        };
        let row = |i: i32| TIP_NET_ROW0 + i * TIP_NET_STEP;
        self.text_row(
            dc,
            &px,
            row(0),
            "Public IP",
            label_color,
            net.public_ip.as_deref(),
            value_color,
        );
        self.location_row(dc, &px, row(1), label_color, value_color);
        self.text_row(
            dc,
            &px,
            row(2),
            "LAN IP",
            p.label,
            net.lan_ip.as_deref(),
            p.value,
        );
        let offline = status == Status::Offline;
        let traffic = v.traffic;
        self.rate_row(
            dc,
            &px,
            row(3),
            glyph::DOWN,
            "Download",
            p.normal,
            traffic.map(|t| t.down),
            offline,
        );
        self.rate_row(
            dc,
            &px,
            row(4),
            glyph::UP,
            "Upload",
            p.country,
            traffic.map(|t| t.up),
            offline,
        );

        self.separator(dc, &px, TIP_SEP2);
        let footer_color = if net.stale {
            p.close_glyph
        } else {
            p.stale_value
        };
        let r = px.rect(TIP_PAD_X, TIP_FOOTER, inner, TIP_FOOTER_H);
        text(
            dc,
            &self.fonts.small,
            footer_color,
            &r,
            &footer_text(net),
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }

    fn separator(&self, dc: HDC, px: &Px, top: i32) {
        let mut r = px.rect(TIP_PAD_X, top, TIP_WIDTH - 2 * TIP_PAD_X, 1);
        r.bottom = r.top + px.v(1).max(1);
        fill(dc, &r, self.palette.track);
    }

    #[allow(clippy::too_many_arguments)]
    fn metric(
        &self,
        dc: HDC,
        px: &Px,
        top: i32,
        icon: &str,
        label: &str,
        value: Option<f32>,
        detail: Option<(String, COLORREF)>,
    ) {
        let p = self.palette;
        let inner = TIP_WIDTH - 2 * TIP_PAD_X;
        let r = px.rect(TIP_PAD_X, top, TIP_ICON, TIP_ROW_H);
        text(dc, &self.fonts.tip_icon, p.label, &r, icon, DT_CENTER);
        let r = px.rect(TIP_PAD_X + TIP_LABEL_X, top, 60, TIP_ROW_H);
        text(dc, &self.fonts.text, p.label, &r, label, DT_LEFT);

        let load = value.map(|v| Load::of(round_percent(v)));
        let value_color = load.map_or(p.stale_value, |l| l.text_color(p));
        let s = percent_text(value);
        let row = px.rect(TIP_PAD_X, top, inner, TIP_ROW_H);
        text(dc, &self.fonts.value, value_color, &row, &s, DT_RIGHT);
        if let Some((detail, color)) = detail {
            let w = text_width(dc, &self.fonts.value, &s) + px.v(TIP_DETAIL_GAP);
            let r = RECT {
                right: row.right - w,
                ..row
            };
            text(dc, &self.fonts.detail, color, &r, &detail, DT_RIGHT);
        }

        let track = px.rect(TIP_PAD_X, top + TIP_ROW_H + TIP_BAR_GAP, inner, TIP_BAR_H);
        let radius = (track.bottom - track.top) / 2;
        match (value, load) {
            (Some(v), Some(load)) => {
                round_rect(dc, &track, radius, p.track);
                let filled = RECT {
                    right: track.left + bar_fill(track.right - track.left, round_percent(v)),
                    ..track
                };
                round_rect(dc, &filled, radius, load.bar_color(p));
            }
            _ => round_rect(dc, &track, radius, p.track_na),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text_row(
        &self,
        dc: HDC,
        px: &Px,
        top: i32,
        label: &str,
        label_color: COLORREF,
        value: Option<&str>,
        value_color: COLORREF,
    ) {
        let r = px.rect(TIP_PAD_X, top, TIP_WIDTH - 2 * TIP_PAD_X, TIP_ROW_H);
        text(dc, &self.fonts.text, label_color, &r, label, DT_LEFT);
        let (s, color) = match value {
            Some(s) => (s, value_color),
            None => ("—", self.palette.stale_value),
        };
        let r = RECT {
            left: r.left + px.v(70),
            ..r
        };
        text(
            dc,
            &self.fonts.value,
            color,
            &r,
            s,
            DT_RIGHT | DT_END_ELLIPSIS,
        );
    }

    fn location_row(
        &self,
        dc: HDC,
        px: &Px,
        top: i32,
        label_color: COLORREF,
        value_color: COLORREF,
    ) {
        let p = self.palette;
        let net = &self.view.net;
        let row = px.rect(TIP_PAD_X, top, TIP_WIDTH - 2 * TIP_PAD_X, TIP_ROW_H);
        text(dc, &self.fonts.text, label_color, &row, "Location", DT_LEFT);
        let value_left = row.left + px.v(70);

        let Some(location) = net.location.as_deref() else {
            text(dc, &self.fonts.text, p.stale_value, &row, "—", DT_RIGHT);
            return;
        };

        // The country tag sits at the right edge; it is hidden while stale.
        let mut right = row.right;
        if let Some(code) = net.country_code.as_deref().filter(|_| !net.stale) {
            let tag_w = text_width(dc, &self.fonts.caption, code) + 2 * px.v(TIP_TAG_PAD);
            let tag_h = px.v(TIP_TAG_H);
            let tag_top = row.top + (row.bottom - row.top - tag_h) / 2;
            let tag = RECT {
                left: right - tag_w,
                right,
                top: tag_top,
                bottom: tag_top + tag_h,
            };
            round_rect(dc, &tag, px.v(3), p.tag_bg);
            text(dc, &self.fonts.caption, p.country, &tag, code, DT_CENTER);
            right = tag.left - px.v(TIP_TAG_GAP);
        }
        let r = RECT {
            left: value_left,
            right,
            ..row
        };
        text(
            dc,
            &self.fonts.text,
            value_color,
            &r,
            location,
            DT_RIGHT | DT_END_ELLIPSIS,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn rate_row(
        &self,
        dc: HDC,
        px: &Px,
        top: i32,
        arrow: &str,
        label: &str,
        arrow_color: COLORREF,
        rate: Option<f64>,
        offline: bool,
    ) {
        let p = self.palette;
        let dim = offline || rate.is_none();
        let r = px.rect(TIP_PAD_X, top, ARROW, TIP_ROW_H);
        let color = if dim { p.stale_label } else { arrow_color };
        text(dc, &self.fonts.arrow, color, &r, arrow, DT_CENTER);
        let r = px.rect(TIP_PAD_X + TIP_LABEL_X - 1, top, 80, TIP_ROW_H);
        text(dc, &self.fonts.text, p.label, &r, label, DT_LEFT);

        let s = rate.map_or_else(|| "—".to_owned(), rate_text);
        let color = if dim { p.stale_value } else { p.value };
        let r = px.rect(TIP_PAD_X, top, TIP_WIDTH - 2 * TIP_PAD_X, TIP_ROW_H);
        text(dc, &self.fonts.value, color, &r, &s, DT_RIGHT);
    }
}

/// Close button: idle shows a bordered square in the panel colors, hover the
/// Windows 11 close red with a white glyph.
pub fn close_button(dc: HDC, size: i32, palette: &Palette, fonts: &Fonts, hovered: bool) {
    let r = client(size, size);
    let glyph_color = if hovered {
        fill(dc, &r, palette.close_hover);
        palette.close_hover_glyph
    } else {
        fill(dc, &r, palette.close_bg);
        frame(dc, &r, palette.close_border);
        palette.close_glyph
    };
    text(dc, &fonts.close, glyph_color, &r, glyph::CLOSE, DT_CENTER);
}
