//! Thin RAII wrappers and drawing helpers over GDI.
//!
//! Colors go through the stock `DC_BRUSH` / `DC_PEN` objects, so painting never
//! creates per-color brushes or pens.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, GetStockObject, RoundRect,
    SelectObject, SetBkMode, SetDCBrushColor, SetDCPenColor, SetTextColor, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DC_BRUSH, DC_PEN, DEFAULT_CHARSET, DEFAULT_PITCH, DRAW_TEXT_FORMAT,
    DT_CALCRECT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HBITMAP, HBRUSH, HDC, HFONT,
    HGDIOBJ, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SRCCOPY, TRANSPARENT,
};

/// `BeginPaint` / `EndPaint` pair.
pub struct PaintGuard {
    hwnd: HWND,
    ps: PAINTSTRUCT,
    pub hdc: HDC,
}

impl PaintGuard {
    pub fn begin(hwnd: HWND) -> Self {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: called from WM_PAINT for a window owned by this thread.
        let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
        Self { hwnd, ps, hdc }
    }
}

impl Drop for PaintGuard {
    fn drop(&mut self) {
        // SAFETY: matches the BeginPaint in `begin`.
        unsafe {
            let _ = EndPaint(self.hwnd, &self.ps);
        }
    }
}

/// Off-screen bitmap for flicker-free painting; `blit` copies it to the target.
pub struct BackBuffer {
    pub dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    width: i32,
    height: i32,
}

impl BackBuffer {
    pub fn new(target: HDC, width: i32, height: i32) -> Option<Self> {
        // SAFETY: plain GDI calls; every created object is released in Drop.
        unsafe {
            let dc = CreateCompatibleDC(target);
            if dc.is_invalid() {
                return None;
            }
            let bitmap = CreateCompatibleBitmap(target, width, height);
            if bitmap.is_invalid() {
                let _ = DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bitmap);
            SetBkMode(dc, TRANSPARENT);
            SelectObject(dc, GetStockObject(DC_BRUSH));
            SelectObject(dc, GetStockObject(DC_PEN));
            Some(Self {
                dc,
                bitmap,
                old,
                width,
                height,
            })
        }
    }

    pub fn blit(&self, target: HDC) {
        // SAFETY: both DCs are valid; sizes match the bitmap.
        unsafe {
            let _ = BitBlt(
                target,
                0,
                0,
                self.width,
                self.height,
                self.dc,
                0,
                0,
                SRCCOPY,
            );
        }
    }
}

impl Drop for BackBuffer {
    fn drop(&mut self) {
        // SAFETY: restores the original bitmap before deleting ours, then the DC.
        // Stock objects selected into the DC need no cleanup.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}

/// Owned GDI font.
pub struct Font(HFONT);

impl Font {
    /// `px` is the em height in device pixels (already DPI-scaled).
    pub fn new(face: PCWSTR, px: i32, weight: u32) -> Self {
        // SAFETY: plain FFI call; `face` is a static wide string.
        let font = unsafe {
            CreateFontW(
                -px,
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                face,
            )
        };
        Self(font)
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // SAFETY: fonts are only selected for the duration of a `text` call.
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

pub fn fill(dc: HDC, r: &RECT, color: COLORREF) {
    // SAFETY: `dc` has DC_BRUSH available as a stock object.
    unsafe {
        SetDCBrushColor(dc, color);
        FillRect(dc, r, HBRUSH(GetStockObject(DC_BRUSH).0));
    }
}

pub fn frame(dc: HDC, r: &RECT, color: COLORREF) {
    // SAFETY: as in `fill`.
    unsafe {
        SetDCBrushColor(dc, color);
        FrameRect(dc, r, HBRUSH(GetStockObject(DC_BRUSH).0));
    }
}

/// Solid rounded rectangle. `dc` must have DC_BRUSH and DC_PEN selected.
pub fn round_rect(dc: HDC, r: &RECT, radius: i32, color: COLORREF) {
    if r.right <= r.left || r.bottom <= r.top {
        return;
    }
    // SAFETY: `dc` is valid with the DC pen and brush selected (see BackBuffer).
    unsafe {
        SetDCBrushColor(dc, color);
        SetDCPenColor(dc, color);
        let _ = RoundRect(dc, r.left, r.top, r.right, r.bottom, radius * 2, radius * 2);
    }
}

/// Draws single-line text; `flags` adds alignment / ellipsis to the defaults.
pub fn text(dc: HDC, font: &Font, color: COLORREF, r: &RECT, s: &str, flags: DRAW_TEXT_FORMAT) {
    // DrawTextW faults on an empty buffer (the slice pointer is dangling and the
    // count is zero); there is nothing to draw anyway.
    if s.is_empty() {
        return;
    }
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    let mut rect = *r;
    // SAFETY: the font is valid and restored before returning.
    unsafe {
        let old = SelectObject(dc, font.0);
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            &mut wide,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | flags,
        );
        SelectObject(dc, old);
    }
}

/// Width of `s` in `font`, in device pixels.
pub fn text_width(dc: HDC, font: &Font, s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    let mut rect = RECT::default();
    // SAFETY: as in `text`; DT_CALCRECT only measures.
    unsafe {
        let old = SelectObject(dc, font.0);
        DrawTextW(
            dc,
            &mut wide,
            &mut rect,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
    rect.right - rect.left
}
