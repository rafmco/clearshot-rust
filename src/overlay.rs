//! The movable, resizable recording frame: a top-most layered window whose inside is see-through
//! and click-through. It is excluded from screen capture so it never shows up in recordings.
use crate::capture::{fit, Area};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub static AREA: Mutex<Area> = Mutex::new(Area { x: 200, y: 150, w: 1280, h: 720 });
pub static VISIBLE: AtomicBool = AtomicBool::new(true);
pub static RECORDING: AtomicBool = AtomicBool::new(false);
/// Frame opacity 0..255 (Options > Frame opacity).
pub static OPACITY: AtomicU8 = AtomicU8::new(255);
/// Timer ticks (50 ms) left of the white capture flash.
pub static FLASH: AtomicU32 = AtomicU32::new(0);
static APPLIED_ALPHA: AtomicU8 = AtomicU8::new(255);
/// Webcam overlay (width px, corner) to outline on the frame, or None when the webcam is off.
pub static CAM: Mutex<Option<(u32, usize)>> = Mutex::new(None);
/// Actual recording frame rate x10 shown on the size label; 0 hides it.
pub static FPS: AtomicU32 = AtomicU32::new(0);

const B: i32 = 4; // border thickness, outside the recorded area
const HANDLE: i32 = 12;
const KEY: COLORREF = COLORREF(0x00FF00FF); // magenta = transparent

pub fn area() -> Area {
    *AREA.lock().unwrap()
}
pub fn set_area(a: Area) {
    *AREA.lock().unwrap() = fit(a);
}

/// Arrow-key handling shared by the frame and the main window.
/// Arrow = move 1 px, Ctrl+Arrow = move 10 px, Shift+Arrow = resize 2 px (x10 with Ctrl).
pub fn nudge(dx: i32, dy: i32, ctrl: bool, shift: bool) {
    let step = if ctrl { 10 } else { 1 };
    let mut a = area();
    if shift {
        if RECORDING.load(Ordering::Relaxed) {
            return;
        }
        a.w += dx * step * 2;
        a.h += dy * step * 2;
    } else {
        a.x += dx * step;
        a.y += dy * step;
    }
    set_area(a);
}

pub fn spawn() {
    std::thread::spawn(|| unsafe { run() });
}

unsafe fn outer(a: Area) -> RECT {
    RECT { left: a.x - B, top: a.y - B, right: a.x + a.w + B, bottom: a.y + a.h + B }
}

unsafe fn run() {
    let inst = GetModuleHandleW(None).unwrap();
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc_),
        hInstance: inst.into(),
        lpszClassName: w!("ClearShotFrame"),
        hCursor: LoadCursorW(None, IDC_SIZEALL).unwrap(),
        ..Default::default()
    };
    RegisterClassW(&wc);
    let r = outer(area());
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        w!("ClearShotFrame"),
        w!("Recording area"),
        WS_POPUP,
        r.left,
        r.top,
        r.right - r.left,
        r.bottom - r.top,
        None,
        None,
        Some(inst.into()),
        None,
    )
    .unwrap();
    let _ = SetLayeredWindowAttributes(hwnd, KEY, 255, LWA_COLORKEY);
    let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11)); // WDA_EXCLUDEFROMCAPTURE
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    SetTimer(Some(hwnd), 1, 50, None);
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

static LAST: Mutex<(Area, bool, bool, Option<(u32, usize)>, u32)> = Mutex::new((Area { x: 0, y: 0, w: 0, h: 0 }, false, false, None, 0));

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            let a = area();
            let vis = VISIBLE.load(Ordering::Relaxed);
            let rec = RECORDING.load(Ordering::Relaxed);
            let mut last = LAST.lock().unwrap();
            if last.0 != a {
                let r = outer(a);
                let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            if last.1 != vis {
                let _ = ShowWindow(hwnd, if vis { SW_SHOWNOACTIVATE } else { SW_HIDE });
            }
            let cam = *CAM.lock().unwrap();
            let fps = FPS.load(Ordering::Relaxed);
            if last.2 != rec || last.3 != cam || last.4 != fps {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            *last = (a, vis, rec, cam, fps);
            let alpha = OPACITY.load(Ordering::Relaxed);
            if APPLIED_ALPHA.swap(alpha, Ordering::Relaxed) != alpha {
                let _ = SetLayeredWindowAttributes(hwnd, KEY, alpha, LWA_COLORKEY | LWA_ALPHA);
            }
            if FLASH.load(Ordering::Relaxed) > 0 {
                FLASH.fetch_sub(1, Ordering::Relaxed);
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_WINDOWPOSCHANGED => {
            let mut r = RECT::default();
            let _ = GetWindowRect(hwnd, &mut r);
            let a = Area { x: r.left + B, y: r.top + B, w: r.right - r.left - 2 * B, h: r.bottom - r.top - 2 * B };
            if a != area() {
                set_area(a);
                LAST.lock().unwrap().0 = area();
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_SIZING => {
            let r = &mut *(lp.0 as *mut RECT);
            if RECORDING.load(Ordering::Relaxed) {
                *r = outer(area()); // size is fixed while recording
            } else {
                // snap the inner size like fit() does, moving the edge being dragged
                let w = ((r.right - r.left - 2 * B).max(64) & !1) + 2 * B;
                let h = ((r.bottom - r.top - 2 * B).max(64) & !1) + 2 * B;
                let e = wp.0 as u32;
                if matches!(e, WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) { r.left = r.right - w } else { r.right = r.left + w }
                if matches!(e, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) { r.top = r.bottom - h } else { r.bottom = r.top + h }
            }
            LRESULT(1)
        }
        WM_GETMINMAXINFO => {
            let m = &mut *(lp.0 as *mut MINMAXINFO);
            m.ptMinTrackSize = POINT { x: 64, y: 64 };
            m.ptMaxTrackSize = POINT { x: 16384, y: 16384 };
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let mut r = RECT::default();
            let _ = GetWindowRect(hwnd, &mut r);
            let x = (lp.0 & 0xFFFF) as i16 as i32 - r.left;
            let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32 - r.top;
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            let (cx, cy) = (w / 2, h / 2);
            if (x - cx).abs() <= HANDLE * 2 && (y - cy).abs() <= HANDLE * 2 {
                return LRESULT(HTCAPTION as isize);
            }
            if RECORDING.load(Ordering::Relaxed) {
                return LRESULT(HTCAPTION as isize);
            }
            let l = x < HANDLE;
            let rt = x >= w - HANDLE;
            let t = y < HANDLE;
            let b = y >= h - HANDLE;
            let hit = match (l, rt, t, b) {
                (true, _, true, _) => HTTOPLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                (true, ..) => HTLEFT,
                (_, true, ..) => HTRIGHT,
                (_, _, true, _) => HTTOP,
                (.., true) => HTBOTTOM,
                _ => HTCAPTION,
            };
            LRESULT(hit as isize)
        }
        WM_KEYDOWN => {
            let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
            let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
            match VIRTUAL_KEY(wp.0 as u16) {
                VK_LEFT => nudge(-1, 0, ctrl, shift),
                VK_RIGHT => nudge(1, 0, ctrl, shift),
                VK_UP => nudge(0, -1, ctrl, shift),
                VK_DOWN => nudge(0, 1, ctrl, shift),
                _ => {}
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            let key = CreateSolidBrush(KEY);
            FillRect(dc, &r, key);
            if FLASH.load(Ordering::Relaxed) > 1 {
                // capture feedback: the inside flashes white (the frame is never in captures)
                let white = CreateSolidBrush(COLORREF(0xFFFFFF));
                FillRect(dc, &r, white);
                let _ = DeleteObject(white.into());
            }
            let col = if RECORDING.load(Ordering::Relaxed) { COLORREF(0x2020E0) } else { COLORREF(0x30C040) };
            let br = CreateSolidBrush(col);
            let (w, h) = (r.right, r.bottom);
            for rr in [
                RECT { left: 0, top: 0, right: w, bottom: B },
                RECT { left: 0, top: h - B, right: w, bottom: h },
                RECT { left: 0, top: 0, right: B, bottom: h },
                RECT { left: w - B, top: 0, right: w, bottom: h },
                // corner + edge-middle grab handles
                RECT { left: 0, top: 0, right: HANDLE, bottom: HANDLE },
                RECT { left: w - HANDLE, top: 0, right: w, bottom: HANDLE },
                RECT { left: 0, top: h - HANDLE, right: HANDLE, bottom: h },
                RECT { left: w - HANDLE, top: h - HANDLE, right: w, bottom: h },
                RECT { left: w / 2 - HANDLE / 2, top: 0, right: w / 2 + HANDLE / 2, bottom: HANDLE },
                RECT { left: w / 2 - HANDLE / 2, top: h - HANDLE, right: w / 2 + HANDLE / 2, bottom: h },
                RECT { left: 0, top: h / 2 - HANDLE / 2, right: HANDLE, bottom: h / 2 + HANDLE / 2 },
                RECT { left: w - HANDLE, top: h / 2 - HANDLE / 2, right: w, bottom: h / 2 + HANDLE / 2 },
                // centre move handle (cross)
                RECT { left: w / 2 - HANDLE * 2, top: h / 2 - 3, right: w / 2 + HANDLE * 2, bottom: h / 2 + 3 },
                RECT { left: w / 2 - 3, top: h / 2 - HANDLE * 2, right: w / 2 + 3, bottom: h / 2 + HANDLE * 2 },
            ] {
                FillRect(dc, &rr, br);
            }
            // size label
            let a = area();
            let fps = FPS.load(Ordering::Relaxed);
            let label = if fps > 0 { format!(" {} x {}  ·  {:.1} fps ", a.w, a.h, fps as f32 / 10.0) } else { format!(" {} x {} ", a.w, a.h) };
            let mut t: Vec<u16> = label.encode_utf16().collect();
            SetBkColor(dc, col);
            SetTextColor(dc, COLORREF(0xFFFFFF));
            SetBkMode(dc, OPAQUE);
            let mut lr = RECT { left: HANDLE + 4, top: B, right: w, bottom: B + 20 };
            DrawTextW(dc, &mut t, &mut lr, DT_SINGLELINE | DT_LEFT | DT_TOP);
            // where the webcam picture will go: same size and 16 px margin as capture::Grabber::grab
            if let Some((cam_w, corner)) = *CAM.lock().unwrap() {
                let (sw, sh) = crate::webcam::size();
                let tw = (cam_w as i32).min(a.w / 2).max(32);
                let th = tw * sh as i32 / sw.max(1) as i32;
                let x = B + if corner % 2 == 1 { a.w - tw - 16 } else { 16 };
                let y = B + if corner >= 2 { a.h - th - 16 } else { 16 };
                for rr in [
                    RECT { left: x, top: y, right: x + tw, bottom: y + 2 },
                    RECT { left: x, top: y + th - 2, right: x + tw, bottom: y + th },
                    RECT { left: x, top: y, right: x + 2, bottom: y + th },
                    RECT { left: x + tw - 2, top: y, right: x + tw, bottom: y + th },
                ] {
                    FillRect(dc, &rr, br);
                }
                let mut t: Vec<u16> = format!(" Webcam {} x {} ", tw, th).encode_utf16().collect();
                let mut lr = RECT { left: x + 2, top: y + 2, right: x + tw - 2, bottom: y + 22 };
                DrawTextW(dc, &mut t, &mut lr, DT_SINGLELINE | DT_LEFT | DT_TOP);
            }
            let _ = DeleteObject(key.into());
            let _ = DeleteObject(br.into());
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
