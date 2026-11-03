//! Screen grabbing with GDI (works with DWM composition, which also covers borderless/windowed games).
use std::ffi::c_void;
use std::time::{Duration, Instant};
use windows::core::BOOL;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn monitors() -> Vec<Area> {
    unsafe extern "system" fn cb(_m: HMONITOR, _dc: HDC, r: *mut RECT, data: LPARAM) -> BOOL {
        let v = &mut *(data.0 as *mut Vec<Area>);
        let r = *r;
        v.push(Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top });
        TRUE
    }
    let mut v: Vec<Area> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut v as *mut _ as isize));
    }
    if v.is_empty() {
        v.push(Area { x: 0, y: 0, w: 1920, h: 1080 });
    }
    v
}

fn window_rect(hwnd: HWND) -> Option<Area> {
    unsafe {
        let mut r = RECT::default();
        if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut c_void, std::mem::size_of::<RECT>() as u32).is_err() {
            GetWindowRect(hwnd, &mut r).ok()?;
        }
        let a = Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top };
        (a.w > 16 && a.h > 16).then_some(a)
    }
}

/// Top-level window under the mouse pointer (smart area detection).
pub fn window_under_cursor() -> Option<Area> {
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok()?;
        let h = GetAncestor(WindowFromPoint(p), GA_ROOT);
        if h.is_invalid() {
            return None;
        }
        window_rect(h)
    }
}

/// Active (foreground) window, used by Game Recording.
pub fn foreground_window() -> Option<Area> {
    unsafe {
        let h = GetForegroundWindow();
        if h.is_invalid() {
            return None;
        }
        window_rect(h)
    }
}

pub fn left_button_down() -> bool {
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x01) as u16 & 0x8000 != 0 }
}

/// Clamp an area to the virtual desktop. Width and height snap to 2 px (H.264 needs even sizes);
/// the recorder pads the width to 16 px for the encoder and crops it back in the file.
pub fn fit(a: Area) -> Area {
    let mons = monitors();
    let l = mons.iter().map(|m| m.x).min().unwrap();
    let t = mons.iter().map(|m| m.y).min().unwrap();
    let r = mons.iter().map(|m| m.x + m.w).max().unwrap();
    let b = mons.iter().map(|m| m.y + m.h).max().unwrap();
    let w = a.w.clamp(64, r - l) & !1;
    let h = a.h.clamp(32, b - t) & !1;
    Area { x: a.x.clamp(l, r - w), y: a.y.clamp(t, b - h), w, h }
}

pub struct Overlay {
    pub text: String,
    pub image: Option<(u32, u32, Vec<u8>)>, // rgba
    pub corner: usize,
}

/// Rings drawn where the mouse is clicked (Options > Click effects).
pub struct ClickFx {
    pub left: u32,
    pub right: u32,
    pub size: i32,
    pub dur: Duration,
}

pub struct Grabber {
    clicks: Vec<(i32, i32, Instant, bool)>,
    buttons: (bool, bool),
    pub w: i32,
    pub h: i32,
    screen: HDC,
    mem: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    font: HFONT,
}

unsafe impl Send for Grabber {}

impl Grabber {
    pub fn new(w: i32, h: i32) -> Grabber {
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(mem), &bi, DIB_RGB_COLORS, &mut bits, None, 0).expect("CreateDIBSection");
            let old = SelectObject(mem, bmp.into());
            let font = CreateFontW(-(h / 24).clamp(14, 48), 0, 0, 0, 700, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY, 0, windows::core::w!("Segoe UI"));
            Grabber { clicks: Vec::new(), buttons: (false, false), w, h, screen, mem, bmp, old, bits: bits as *mut u8, font }
        }
    }

    /// Grab the screen at (x, y) into a BGRA top-down buffer (alpha forced to 255).
    pub fn grab(&mut self, x: i32, y: i32, cursor: bool, ov: &Overlay, cam: Option<&(u32, u32, Vec<u8>)>, cam_w: u32, cam_corner: usize, fx: Option<&ClickFx>) -> Vec<u8> {
        unsafe {
            let _ = BitBlt(self.mem, 0, 0, self.w, self.h, Some(self.screen), x, y, SRCCOPY);
            if let Some(fx) = fx {
                self.draw_clicks(x, y, fx);
            }
            if cursor {
                let mut ci = CURSORINFO { cbSize: std::mem::size_of::<CURSORINFO>() as u32, ..Default::default() };
                if GetCursorInfo(&mut ci).is_ok() && ci.flags.0 & CURSOR_SHOWING.0 != 0 {
                    let mut ii = ICONINFO::default();
                    if GetIconInfo(ci.hCursor.into(), &mut ii).is_ok() {
                        let _ = DrawIconEx(self.mem, ci.ptScreenPos.x - x - ii.xHotspot as i32, ci.ptScreenPos.y - y - ii.yHotspot as i32, ci.hCursor.into(), 0, 0, 0, None, DI_NORMAL);
                        if !ii.hbmMask.is_invalid() {
                            let _ = DeleteObject(ii.hbmMask.into());
                        }
                        if !ii.hbmColor.is_invalid() {
                            let _ = DeleteObject(ii.hbmColor.into());
                        }
                    }
                }
            }
            let n = (self.w * self.h * 4) as usize;
            // webcam goes into the bitmap first so the watermark (text and image) is drawn over it
            if let Some((cw, ch, bgra)) = cam {
                // scale webcam frame to cam_w wide (nearest neighbour)
                let tw = cam_w.min(self.w as u32 / 2).max(32);
                let th = (tw as u64 * *ch as u64 / (*cw).max(1) as u64) as u32;
                let mut scaled = vec![0u8; (tw * th * 4) as usize];
                for yy in 0..th {
                    for xx in 0..tw {
                        let sx = xx * cw / tw;
                        let sy = yy * ch / th;
                        let si = ((sy * cw + sx) * 4) as usize;
                        let di = ((yy * tw + xx) * 4) as usize;
                        scaled[di..di + 4].copy_from_slice(&bgra[si..si + 4]);
                        scaled[di + 3] = 255;
                    }
                }
                let _ = GdiFlush(); // finish BitBlt/cursor drawing before touching the bitmap bits
                blend(std::slice::from_raw_parts_mut(self.bits, n), self.w, self.h, tw, th, &scaled, cam_corner, false, 255);
            }
            if !ov.text.is_empty() {
                let old = SelectObject(self.mem, self.font.into());
                SetBkMode(self.mem, TRANSPARENT);
                let mut t: Vec<u16> = ov.text.encode_utf16().collect();
                let mut m = RECT { left: 16, top: 12, right: self.w - 16, bottom: self.h - 12 };
                let fmt = DT_SINGLELINE
                    | if ov.corner % 2 == 1 { DT_RIGHT } else { DT_LEFT }
                    | if ov.corner >= 2 { DT_BOTTOM } else { DT_TOP };
                SetTextColor(self.mem, COLORREF(0x202020));
                let mut s = RECT { left: m.left + 2, top: m.top + 2, right: m.right + 2, bottom: m.bottom + 2 };
                DrawTextW(self.mem, &mut t, &mut s, fmt);
                SetTextColor(self.mem, COLORREF(0xFFFFFF));
                DrawTextW(self.mem, &mut t, &mut m, fmt);
                SelectObject(self.mem, old);
            }
            let _ = GdiFlush();
            let mut out = std::slice::from_raw_parts(self.bits, n).to_vec();
            for px in out.chunks_exact_mut(4) {
                px[3] = 255;
            }
            if let Some((iw, ih, rgba)) = &ov.image {
                blend(&mut out, self.w, self.h, *iw, *ih, rgba, ov.corner, true, 255);
            }
            out
        }
    }
}

impl Grabber {
    /// Polled once per frame: a click shorter than one frame can be missed (rare at 15+ fps).
    unsafe fn draw_clicks(&mut self, x: i32, y: i32, fx: &ClickFx) {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let down = |vk: i32| GetAsyncKeyState(vk) as u16 & 0x8000 != 0;
        let (l, r) = (down(0x01), down(0x02));
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let now = Instant::now();
        if l && !self.buttons.0 {
            self.clicks.push((p.x, p.y, now, false));
        }
        if r && !self.buttons.1 {
            self.clicks.push((p.x, p.y, now, true));
        }
        self.buttons = (l, r);
        self.clicks.retain(|c| now - c.2 < fx.dur);
        let old_brush = SelectObject(self.mem, GetStockObject(NULL_BRUSH));
        for &(cx, cy, t, right) in &self.clicks {
            let k = (now - t).as_secs_f32() / fx.dur.as_secs_f32(); // 0 -> 1 over the animation
            let rad = ((0.3 + 0.7 * k) * fx.size as f32 / 2.0) as i32;
            let pen = CreatePen(PS_SOLID, 4, COLORREF(if right { fx.right } else { fx.left }));
            let old_pen = SelectObject(self.mem, pen.into());
            let (cx, cy) = (cx - x, cy - y);
            let _ = Ellipse(self.mem, cx - rad, cy - rad, cx + rad, cy + rad);
            SelectObject(self.mem, old_pen);
            let _ = DeleteObject(pen.into());
        }
        SelectObject(self.mem, old_brush);
    }
}

/// Alpha-blend `src` into the BGRA frame at a corner with a 16px margin.
fn blend(dst: &mut [u8], w: i32, h: i32, sw: u32, sh: u32, src: &[u8], corner: usize, src_is_rgba: bool, opacity: u32) {
    let m = 16i32;
    let ox = if corner % 2 == 1 { w - sw as i32 - m } else { m };
    let oy = if corner >= 2 { h - sh as i32 - m } else { m };
    for y in 0..sh as i32 {
        let dy = oy + y;
        if dy < 0 || dy >= h {
            continue;
        }
        for x in 0..sw as i32 {
            let dx = ox + x;
            if dx < 0 || dx >= w {
                continue;
            }
            let si = ((y as u32 * sw + x as u32) * 4) as usize;
            let di = ((dy * w + dx) * 4) as usize;
            let (b, g, r) = if src_is_rgba { (src[si + 2], src[si + 1], src[si]) } else { (src[si], src[si + 1], src[si + 2]) };
            let a = src[si + 3] as u32 * opacity / 255;
            for (k, c) in [b, g, r].into_iter().enumerate() {
                dst[di + k] = ((c as u32 * a + dst[di + k] as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem, self.old);
            let _ = DeleteObject(self.bmp.into());
            let _ = DeleteObject(self.font.into());
            let _ = DeleteDC(self.mem);
            ReleaseDC(None, self.screen);
        }
    }
}

/// BGRA -> RGBA in place.
pub fn bgra_to_rgba(mut v: Vec<u8>) -> Vec<u8> {
    for px in v.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    v
}
