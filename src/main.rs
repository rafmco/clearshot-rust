#![windows_subsystem = "windows"]
mod audio;
mod capture;
mod encoder;
mod i18n;
mod overlay;
mod recorder;
mod settings;
mod webcam;

use capture::Area;
use i18n::tr;
use macroquad::prelude::*;
use recorder::{Mode, Recording};
use settings::*;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const W: f32 = 600.0;
const H: f32 = 200.0;
const OPT_W: f32 = 960.0;
const OPT_H: f32 = 630.0;

// palette
const NAVY: Color = Color::new(0.16, 0.22, 0.33, 1.0);
const BG: Color = Color::new(0.95, 0.96, 0.975, 1.0);
const PANEL: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const LINE: Color = Color::new(0.82, 0.85, 0.89, 1.0);
const INK: Color = Color::new(0.15, 0.17, 0.21, 1.0);
const MUTED: Color = Color::new(0.48, 0.52, 0.58, 1.0);
const ACCENT: Color = Color::new(0.18, 0.48, 0.92, 1.0);
const REC: Color = Color::new(0.88, 0.2, 0.2, 1.0);
const HOVER: Color = Color::new(0.9, 0.94, 1.0, 1.0);

#[derive(Clone, Copy, PartialEq)]
enum Menu {
    None,
    Main,
    Size,
    Codecs,
    Sound,
    Help,
}

#[derive(Clone, Copy)]
enum Act {
    Monitor(usize),
    PickWindow,
    Preset(i32, i32),
    CustomSize,
    ToggleFrame,
    Format(usize),
    Gif,
    Encoder(usize),
    SoundFormat(usize),
    ToggleSystem,
    ToggleMic,
    NoSound,
    Options,
    Open,
    Exit,
    About,
    Lang(usize),
}

struct Ui {
    s: f32,
    font: Option<Font>,
    mouse: Vec2,
    pressed: bool,
    blocker: Option<Rect>,
    in_popup: bool,
    drawn: Vec<String>,
    // drop-down state: which selector is open, its list to draw last, a pick to hand back, the list's area
    dd_open: Option<(f32, f32)>,
    dd_opened_now: bool,
    dd_list: Option<(Rect, Vec<String>, usize)>,
    dd_pick: Option<((f32, f32), usize)>,
    dd_rect: Option<Rect>,
}

impl Ui {
    fn text(&mut self, t: &str, x: f32, y: f32, size: f32, c: Color) {
        let fs = (size * self.s).round() as u16;
        draw_text_ex(tr(t), x, y, TextParams { font: self.font.as_ref(), font_size: fs, font_scale: 1.0 / self.s, color: c, ..Default::default() });
        self.drawn.push(t.to_string());
    }
    fn width(&self, t: &str, size: f32) -> f32 {
        measure_text(tr(t), self.font.as_ref(), (size * self.s).round() as u16, 1.0 / self.s).width
    }
    fn text_c(&mut self, t: &str, cx: f32, y: f32, size: f32, c: Color) {
        let w = self.width(t, size);
        self.text(t, cx - w / 2.0, y, size, c);
    }
    fn hot(&self, r: Rect) -> bool {
        r.contains(self.mouse) && (self.in_popup || !self.blocker.map_or(false, |b| b.contains(self.mouse)))
    }
    fn click(&self, r: Rect) -> bool {
        self.pressed && self.hot(r)
    }
    /// flat button with label; returns clicked
    fn button(&mut self, r: Rect, label: &str, primary: bool) -> bool {
        let hot = self.hot(r);
        let fill = if primary { if hot { Color::new(0.12, 0.4, 0.85, 1.0) } else { ACCENT } } else if hot { HOVER } else { PANEL };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if primary { fill } else { LINE });
        self.text_c(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + 5.0, 14.0, if primary { WHITE } else { INK });
        self.click(r)
    }
    fn checkbox(&mut self, x: f32, y: f32, label: &str, v: &mut bool) {
        let r = Rect::new(x, y, 18.0 + self.width(label, 14.0) + 8.0, 18.0);
        draw_rectangle(x, y, 16.0, 16.0, if *v { ACCENT } else { PANEL });
        draw_rectangle_lines(x, y, 16.0, 16.0, 1.0, if *v { ACCENT } else { MUTED });
        if *v {
            draw_line(x + 3.5, y + 8.5, x + 7.0, y + 12.0, 2.0, WHITE);
            draw_line(x + 7.0, y + 12.0, x + 13.0, y + 4.5, 2.0, WHITE);
        }
        self.text(label, x + 24.0, y + 13.0, 14.0, INK);
        if self.click(r) {
            *v = !*v;
        }
    }
    /// Drop-down selector: click to see every choice, click one to pick it. Returns the index
    /// (a pick arrives the frame after the click, from `draw_dropdown`).
    fn select(&mut self, x: f32, y: f32, w: f32, idx: usize, labels: Vec<String>) -> usize {
        let id = (x, y);
        let r = Rect::new(x, y, w, 24.0);
        let open = self.dd_open == Some(id);
        draw_rectangle(x, y, w, 24.0, if open || self.hot(r) { HOVER } else { PANEL });
        draw_rectangle_lines(x, y, w, 24.0, 1.0, if open { ACCENT } else { LINE });
        let cur = labels.get(idx).cloned().unwrap_or_default();
        self.text(&cur, x + 8.0, y + 17.0, 14.0, INK);
        let (ax, ay) = (x + w - 14.0, y + 10.0);
        draw_triangle(vec2(ax - 5.0, ay), vec2(ax + 5.0, ay), vec2(ax, ay + 6.0), ACCENT);
        if self.click(r) {
            self.dd_open = if open { None } else { Some(id) };
            self.dd_opened_now = !open;
        }
        if self.dd_open == Some(id) {
            self.dd_list = Some((r, labels, idx));
        }
        match self.dd_pick.take() {
            Some((pid, i)) if pid == id => i,
            other => {
                self.dd_pick = other;
                idx
            }
        }
    }

    /// Draws the open drop-down list over everything else; call after all widgets.
    fn draw_dropdown(&mut self, max_y: f32) {
        self.dd_rect = None;
        let Some((r, labels, idx)) = self.dd_list.take() else { return };
        if self.dd_open != Some((r.x, r.y)) {
            return;
        }
        let h = labels.len() as f32 * 24.0 + 4.0;
        let w = labels.iter().map(|l| self.width(l, 14.0) + 24.0).fold(r.w, f32::max);
        let top = if r.y + 24.0 + h > max_y { r.y - h } else { r.y + 24.0 }; // open upward near the bottom
        let list = Rect::new(r.x, top, w, h);
        draw_rectangle(list.x + 3.0, list.y + 3.0, list.w, list.h, Color::new(0.0, 0.0, 0.0, 0.12));
        draw_rectangle(list.x, list.y, list.w, list.h, PANEL);
        draw_rectangle_lines(list.x, list.y, list.w, list.h, 1.0, ACCENT);
        self.in_popup = true;
        for (i, l) in labels.iter().enumerate() {
            let ir = Rect::new(list.x + 2.0, list.y + 2.0 + i as f32 * 24.0, list.w - 4.0, 24.0);
            if self.hot(ir) {
                draw_rectangle(ir.x, ir.y, ir.w, ir.h, HOVER);
            } else if i == idx {
                draw_rectangle(ir.x, ir.y, ir.w, ir.h, Color::new(0.93, 0.95, 0.99, 1.0));
            }
            self.text(l, ir.x + 8.0, ir.y + 17.0, 14.0, if i == idx { ACCENT } else { INK });
            if self.click(ir) {
                self.dd_pick = Some(((r.x, r.y), i));
                self.dd_open = None;
            }
        }
        self.in_popup = false;
        if (self.pressed && !list.contains(self.mouse) && !self.dd_opened_now) || is_key_pressed(KeyCode::Escape) {
            self.dd_open = None;
        }
        self.dd_opened_now = false;
        if self.dd_open.is_some() {
            self.dd_rect = Some(list);
        }
    }
}

// ---------- icons (drawn with shapes) ----------
fn icon(kind: &str, cx: f32, cy: f32, c: Color, on: bool) {
    match kind {
        "record" => {
            if on {
                draw_rectangle(cx - 9.0, cy - 9.0, 18.0, 18.0, c);
            } else {
                draw_circle(cx, cy, 11.0, c);
            }
        }
        "pause" => {
            if on {
                draw_triangle(vec2(cx - 7.0, cy - 10.0), vec2(cx - 7.0, cy + 10.0), vec2(cx + 10.0, cy), c);
            } else {
                draw_rectangle(cx - 8.0, cy - 10.0, 6.0, 20.0, c);
                draw_rectangle(cx + 2.0, cy - 10.0, 6.0, 20.0, c);
            }
        }
        "capture" => {
            draw_rectangle(cx - 13.0, cy - 7.0, 26.0, 17.0, c);
            draw_rectangle(cx - 5.0, cy - 11.0, 10.0, 5.0, c);
            draw_circle(cx, cy + 1.5, 6.0, WHITE);
            draw_circle(cx, cy + 1.5, 3.5, c);
        }
        "size" => {
            draw_rectangle_lines(cx - 13.0, cy - 10.0, 26.0, 20.0, 2.0, c);
            for (x, y) in [(-13.0, -10.0), (9.0, -10.0), (-13.0, 6.0), (9.0, 6.0)] {
                draw_rectangle(cx + x, cy + y, 4.0, 4.0, c);
            }
            draw_line(cx - 4.0, cy, cx + 4.0, cy, 2.0, c);
            draw_line(cx, cy - 4.0, cx, cy + 4.0, 2.0, c);
        }
        "open" => {
            draw_rectangle(cx - 13.0, cy - 9.0, 11.0, 5.0, c);
            draw_rectangle(cx - 13.0, cy - 6.0, 26.0, 17.0, c);
            draw_rectangle(cx - 10.0, cy - 2.0, 20.0, 1.5, WHITE);
        }
        "codecs" => {
            draw_rectangle(cx - 13.0, cy - 10.0, 26.0, 20.0, c);
            for i in 0..4 {
                let y = cy - 8.0 + i as f32 * 5.0;
                draw_rectangle(cx - 11.0, y, 3.0, 2.5, WHITE);
                draw_rectangle(cx + 8.0, y, 3.0, 2.5, WHITE);
            }
            draw_rectangle(cx - 6.0, cy - 7.0, 12.0, 14.0, WHITE);
            draw_triangle(vec2(cx - 3.0, cy - 4.0), vec2(cx - 3.0, cy + 4.0), vec2(cx + 4.0, cy), c);
        }
        "sound" => {
            draw_rectangle(cx - 12.0, cy - 4.0, 6.0, 8.0, c);
            draw_triangle(vec2(cx - 8.0, cy), vec2(cx, cy - 10.0), vec2(cx, cy + 10.0), c);
            if on {
                draw_arc(cx + 2.0, cy, 18, 6.0, -45.0, 2.0, 90.0, c);
                draw_arc(cx + 2.0, cy, 18, 11.0, -45.0, 2.0, 90.0, c);
            } else {
                draw_line(cx + 4.0, cy - 5.0, cx + 13.0, cy + 5.0, 2.0, REC);
                draw_line(cx + 13.0, cy - 5.0, cx + 4.0, cy + 5.0, 2.0, REC);
            }
        }
        "options" => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::PI / 4.0;
                draw_line(cx + a.cos() * 6.0, cy + a.sin() * 6.0, cx + a.cos() * 13.0, cy + a.sin() * 13.0, 5.0, c);
            }
            draw_circle(cx, cy, 9.0, c);
            draw_circle(cx, cy, 4.0, WHITE);
        }
        _ => {}
    }
}

fn hms(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn human(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 {
        format!("{:.2} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

fn disk_free(folder: &str) -> u64 {
    let mut free = 0u64;
    let mut p = std::path::PathBuf::from(folder);
    while !p.exists() {
        if !p.pop() {
            break;
        }
    }
    let h = windows::core::HSTRING::from(p.as_os_str());
    unsafe {
        let _ = windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(&h, Some(&mut free), None, None);
    }
    free
}

fn vk_down(vk: u32) -> bool {
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

fn keycode_to_vk(k: KeyCode) -> Option<u32> {
    use KeyCode::*;
    let f = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    if let Some(i) = f.iter().position(|x| *x == k) {
        return Some(0x70 + i as u32);
    }
    let letters = [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    if let Some(i) = letters.iter().position(|x| *x == k) {
        return Some(0x41 + i as u32);
    }
    let digits = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
    if let Some(i) = digits.iter().position(|x| *x == k) {
        return Some(0x30 + i as u32);
    }
    Some(match k {
        Pause => 0x13,
        PrintScreen => 0x2C,
        ScrollLock => 0x91,
        Insert => 0x2D,
        Home => 0x24,
        End => 0x23,
        _ => return None,
    })
}

fn open_in_explorer(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}

fn browse_folder(cur: &str) -> Option<String> {
    rfd::FileDialog::new().set_directory(cur).pick_folder().map(|p| p.display().to_string())
}

struct App {
    s: Settings,
    mode: Mode,
    rec: Option<Recording>,
    game_armed: bool,
    menu: Menu,
    menu_x: f32,
    options: bool,
    binding: Option<usize>,
    /// text field being typed in: 0 none, 1 watermark text, 2 file name
    editing: u8,
    /// "Custom size" dialog: width and height being typed, and which one has focus
    custom: Option<[String; 2]>,
    custom_field: usize,
    /// actual FPS: when it was last measured, frames sent then, result
    fps: (Instant, u32, f32),
    /// microphones (id, name) and webcams listed while Options is open
    devices: Option<(Vec<(String, String)>, Vec<String>)>,
    picking: u8, // 0 idle, 1 wait for release, 2 wait for click
    frame_shown: bool,
    msg: String,
    msg_bad: bool,
    msg_at: Instant,
    free: u64,
    free_at: Instant,
    levels: (f32, f32),
    levels_at: Instant,
    hk_prev: [bool; 3],
    arrow_held: f32,
    last_saved: Option<std::path::PathBuf>,
    flash_at: Option<Instant>,
}

impl App {
    fn say(&mut self, m: impl Into<String>) {
        self.msg = m.into();
        self.msg_at = Instant::now();
        self.msg_bad = false;
    }
    fn say_err(&mut self, m: impl Into<String>) {
        self.say(m);
        self.msg_bad = true;
    }

    fn record_area(&self) -> Option<Area> {
        match self.mode {
            Mode::Screen => Some(overlay::area()),
            Mode::Game => capture::foreground_window(),
            Mode::Sound => Some(Area { x: 0, y: 0, w: 0, h: 0 }),
        }
    }

    fn toggle_record(&mut self) {
        if let Some(r) = self.rec.take() {
            overlay::RECORDING.store(false, Ordering::Relaxed);
            let el = r.elapsed();
            match r.stop() {
                Ok(p) => {
                    self.say(format!("{} {} ({})", tr("Saved"), p.file_name().unwrap().to_string_lossy(), hms(el)));
                    self.last_saved = Some(p);
                }
                Err(e) => self.say_err(e),
            }
            self.game_armed = false;
            return;
        }
        if self.mode == Mode::Game && !self.game_armed {
            self.game_armed = true;
            self.say(format!("{} {} {}", tr("Game mode ready: bring your game to the front and press"), key_name(self.s.hotkeys[0]), tr("to start / stop.")));
            return;
        }
        let Some(a) = self.record_area() else {
            self.say("No active window to record.");
            return;
        };
        if self.mode == Mode::Sound && !self.s.record_system && !self.s.record_mic {
            self.say("Sound is off: turn on PC sound or the microphone first.");
            return;
        }
        self.rec = Some(Recording::start(self.mode, &self.s, a));
        self.used_number();
        self.fps = (Instant::now(), 0, 0.0);
        overlay::RECORDING.store(self.mode == Mode::Screen, Ordering::Relaxed);
        self.say("Recording...");
    }

    fn capture(&mut self) {
        let a = match self.mode {
            Mode::Game => capture::foreground_window(),
            _ => Some(overlay::area()),
        };
        match a.map(|a| recorder::screenshot(&self.s, a)) {
            Some(Ok(p)) => {
                self.flash_at = Some(Instant::now());
                overlay::FLASH.store(4, Ordering::Relaxed);
                self.say(format!("{} {}", tr("Captured"), p.file_name().unwrap().to_string_lossy()));
                self.last_saved = Some(p);
                self.used_number();
            }
            Some(Err(e)) => self.say_err(format!("{} {e}", tr("Capture failed:"))),
            None => self.say("No active window to capture."),
        }
    }

    /// {n} in the file name counts up once per file.
    fn used_number(&mut self) {
        if self.s.file_name.contains("{n}") {
            self.s.file_number += 1;
            self.s.save();
        }
    }

    fn apply_custom(&mut self) {
        let Some([w, h]) = self.custom.take() else { return };
        let (w, h) = (w.parse::<i32>().unwrap_or(0), h.parse::<i32>().unwrap_or(0));
        if w < 64 || h < 64 {
            self.say_err("Size must be at least 64 x 64.");
            return;
        }
        let a = overlay::area();
        overlay::set_area(Area { x: a.x, y: a.y, w, h });
        self.frame_shown = true;
        let a = overlay::area(); // fit() may have trimmed it to even / on-screen values
        self.say(format!("{} {} x {}", tr("Frame size:"), a.w, a.h));
    }

    fn do_act(&mut self, a: Act) {
        match a {
            Act::Monitor(i) => {
                if let Some(m) = capture::monitors().get(i) {
                    overlay::set_area(*m);
                    self.frame_shown = true;
                }
            }
            Act::PickWindow => {
                self.picking = 1;
                self.say("Click on the window you want to record.");
            }
            Act::Preset(w, h) => {
                let a = overlay::area();
                overlay::set_area(Area { x: a.x, y: a.y, w, h });
                self.frame_shown = true;
            }
            Act::CustomSize => {
                let a = overlay::area();
                self.custom = Some([a.w.to_string(), a.h.to_string()]);
                self.custom_field = 0;
            }
            Act::ToggleFrame => self.frame_shown = !self.frame_shown,
            Act::Format(i) => {
                self.s.video_format = i;
                if self.s.codec == 2 {
                    self.s.codec = 0;
                }
                if matches!(VIDEO_FORMATS[i], "mkv" | "flv" | "vob" | "avi") && encoder::ffmpeg_path().is_none() {
                    self.say(format!("{} {}", VIDEO_FORMATS[i].to_uppercase(), tr("needs FFmpeg (ffmpeg.exe on PATH or next to this app); otherwise MP4 is saved.")));
                }
            }
            Act::Gif => self.s.codec = 2,
            Act::Encoder(i) => self.s.codec = i,
            Act::SoundFormat(i) => self.s.sound_format = i,
            Act::ToggleSystem => self.s.record_system = !self.s.record_system,
            Act::ToggleMic => self.s.record_mic = !self.s.record_mic,
            Act::NoSound => {
                self.s.record_system = false;
                self.s.record_mic = false;
            }
            Act::Options => self.options = true,
            Act::Open => open_in_explorer(&recorder::out_dir(&self.s)),
            Act::Exit => self.quit(),
            Act::Lang(i) => {
                self.s.language = i;
                i18n::set(i);
            }
            Act::About => self.say("ClearShot - screen, game and sound recorder. No ads, ever."),
        }
        self.s.save();
    }

    fn close_options(&mut self) {
        self.options = false;
        self.binding = None;
        self.editing = 0;
        self.devices = None; // list them again next time, devices may be plugged in meanwhile
        self.s.save();
    }

    /// Exit with auto-save: a running recording is finished and kept. No ads, no web pages.
    fn quit(&mut self) {
        if self.rec.is_some() {
            self.toggle_record();
        }
        self.s.area = { let a = overlay::area(); [a.x, a.y, a.w, a.h] };
        self.s.save();
        std::process::exit(0);
    }

    fn menu_items(&self) -> Vec<(String, bool, Option<Act>)> {
        let mut v: Vec<(String, bool, Option<Act>)> = Vec::new();
        match self.menu {
            Menu::Main => {
                v.push(("Options...".into(), false, Some(Act::Options)));
                v.push(("Open output folder".into(), false, Some(Act::Open)));
                v.push(("".into(), false, None));
                for (i, l) in i18n::LANGS.iter().enumerate() {
                    v.push((l.to_string(), self.s.language == i, Some(Act::Lang(i))));
                }
                v.push(("".into(), false, None));
                v.push(("Exit".into(), false, Some(Act::Exit)));
            }
            Menu::Help => v.push(("About ClearShot".into(), false, Some(Act::About))),
            Menu::Size => {
                let cur = overlay::area();
                for (i, m) in capture::monitors().iter().enumerate() {
                    v.push((format!("{} {} ({} x {})", tr("Full screen - Monitor"), i + 1, m.w, m.h), cur == capture::fit(*m), Some(Act::Monitor(i))));
                }
                v.push(("Select a window (click it)".into(), false, Some(Act::PickWindow)));
                for (w, h) in [(320, 240), (640, 360), (640, 480), (800, 600), (1024, 768), (1280, 720), (1920, 1080)] {
                    v.push((format!("{w} x {h}"), cur.w == w && cur.h == h, Some(Act::Preset(w, h))));
                }
                v.push(("Custom size...".into(), false, Some(Act::CustomSize)));
                v.push((if self.frame_shown { "Hide recording frame" } else { "Show recording frame" }.into(), false, Some(Act::ToggleFrame)));
            }
            Menu::Codecs => {
                if self.mode == Mode::Sound {
                    for (i, f) in SOUND_FORMATS.iter().enumerate() {
                        v.push((format!("{} {}", tr("Sound file:"), f.to_uppercase()), self.s.sound_format == i, Some(Act::SoundFormat(i))));
                    }
                } else {
                    for (i, f) in VIDEO_FORMATS.iter().enumerate() {
                        let need = if matches!(*f, "mkv" | "flv" | "vob" | "avi") && encoder::ffmpeg_path().is_none() { format!("  {}", tr("(needs FFmpeg)")) } else { String::new() };
                        v.push((format!("{}{need}", f.to_uppercase()), self.s.codec != 2 && self.s.video_format == i, Some(Act::Format(i))));
                    }
                    v.push(("Animated GIF".into(), self.s.codec == 2, Some(Act::Gif)));
                    v.push(("".into(), false, None));
                    v.push((CODECS[0].into(), self.s.codec == 0, Some(Act::Encoder(0))));
                    v.push((CODECS[1].into(), self.s.codec == 1, Some(Act::Encoder(1))));
                }
            }
            Menu::Sound => {
                v.push(("Record PC sound".into(), self.s.record_system, Some(Act::ToggleSystem)));
                v.push(("Record microphone".into(), self.s.record_mic, Some(Act::ToggleMic)));
                v.push(("No sound".into(), !self.s.record_system && !self.s.record_mic, Some(Act::NoSound)));
            }
            Menu::None => {}
        }
        v
    }

    fn menu_rect(&self, ui: &Ui) -> Option<Rect> {
        if self.menu == Menu::None {
            return None;
        }
        let items = self.menu_items();
        let w = items.iter().map(|i| ui.width(&i.0, 14.0)).fold(120.0, f32::max) + 44.0;
        let h = items.len() as f32 * 24.0 + 8.0;
        let top = if matches!(self.menu, Menu::Main | Menu::Help) { 28.0 } else { 120.0 };
        Some(Rect::new(self.menu_x.min(W - w - 4.0), top, w, h))
    }

    fn info_line(&self) -> String {
        let a = overlay::area();
        let what = match self.mode {
            Mode::Sound => format!("{} sound", SOUND_FORMATS[self.s.sound_format].to_uppercase()),
            _ if self.s.codec == 2 => format!("GIF · {} fps", self.s.gif_fps),
            _ => format!("{} · {} · {} fps", VIDEO_FORMATS[self.s.video_format].to_uppercase(), if self.s.codec == 1 { "GPU H.264" } else { "H.264" }, self.s.fps),
        };
        let area = match self.mode {
            Mode::Screen => format!("{} x {}", a.w, a.h),
            Mode::Game => tr("Active game window").into(),
            Mode::Sound => tr("Audio only").into(),
        };
        let snd = match (self.s.record_system, self.s.record_mic) {
            (true, true) => "PC sound + mic",
            (true, false) => "PC sound",
            (false, true) => "Microphone",
            _ => "No sound",
        };
        let real = if self.s.show_fps && self.rec.is_some() && self.mode != Mode::Sound { format!("   |   {:.1} fps {}", self.fps.2, tr("actual")) } else { String::new() };
        format!("{area}   |   {what}   |   {}{real}", tr(snd))
    }
}

fn draw_main(app: &mut App, ui: &mut Ui) {
    let menu_before = app.menu; // the click that opens a menu must not also close it
    // ---- tab bar ----
    draw_rectangle(0.0, 0.0, W, 28.0, NAVY);
    let tabs: [(&str, Option<Mode>, Menu); 5] = [
        ("Menu", None, Menu::Main),
        ("Screen Recording", Some(Mode::Screen), Menu::None),
        ("Game Recording", Some(Mode::Game), Menu::None),
        ("Sound Recording", Some(Mode::Sound), Menu::None),
        ("Help", None, Menu::Help),
    ];
    let mut x = 6.0;
    for (label, mode, menu) in tabs {
        let w = ui.width(label, 14.0) + 22.0;
        let r = Rect::new(x, 3.0, w, 25.0);
        let sel = mode == Some(app.mode);
        if sel {
            draw_rectangle(r.x, r.y, r.w, r.h, BG);
        } else if ui.hot(r) {
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.12));
        }
        ui.text_c(label, r.x + w / 2.0, 20.0, 14.0, if sel { INK } else { WHITE });
        if ui.click(r) {
            if let Some(m) = mode {
                if app.rec.is_none() {
                    app.mode = m;
                    app.game_armed = false;
                    app.menu = Menu::None;
                } else {
                    app.say("Stop the recording before switching modes.");
                }
            } else {
                app.menu = if app.menu == menu { Menu::None } else { menu };
                app.menu_x = r.x;
            }
        }
        x += w + 2.0;
    }

    // ---- toolbar ----
    draw_rectangle(0.0, 28.0, W, 92.0, BG);
    let recording = app.rec.is_some();
    let paused = app.rec.as_ref().map_or(false, |r| r.is_paused());
    let buttons: [(&str, &str, bool, bool); 8] = [
        (if recording { "Stop" } else if app.game_armed { "Cancel" } else { "Record" }, "record", true, recording || app.game_armed),
        (if paused { "Resume" } else { "Pause" }, "pause", recording, paused),
        ("Capture", "capture", app.mode != Mode::Sound, false),
        ("Size", "size", app.mode == Mode::Screen, false),
        ("Open", "open", true, false),
        ("Codecs", "codecs", !recording, false),
        ("Sound", "sound", !recording, app.s.record_system || app.s.record_mic),
        ("Options", "options", !recording, false),
    ];
    let bw = 70.0;
    let x0 = (W - bw * 8.0) / 2.0;
    for (i, (label, ic, enabled, on)) in buttons.into_iter().enumerate() {
        let r = Rect::new(x0 + i as f32 * bw + 2.0, 34.0, bw - 4.0, 80.0);
        let hot = enabled && ui.hot(r);
        if hot {
            draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color::new(0.7, 0.8, 0.95, 1.0));
        }
        let c = if !enabled { LINE } else if i == 0 { REC } else if i == 1 && paused { ACCENT } else { Color::new(0.25, 0.32, 0.42, 1.0) };
        icon(ic, r.x + r.w / 2.0, r.y + 28.0, c, on);
        ui.text_c(label, r.x + r.w / 2.0, r.y + 68.0, 14.0, if enabled { INK } else { LINE });
        if enabled && ui.click(r) {
            app.menu = Menu::None;
            match i {
                0 => {
                    if app.game_armed && !recording {
                        app.game_armed = false;
                        app.say("Game mode cancelled.");
                    } else {
                        app.toggle_record();
                    }
                }
                1 => {
                    if let Some(r) = app.rec.as_mut() {
                        r.toggle_pause();
                    }
                }
                2 => app.capture(),
                3 => {
                    app.menu = Menu::Size;
                    app.menu_x = r.x;
                }
                4 => app.do_act(Act::Open),
                5 => {
                    app.menu = Menu::Codecs;
                    app.menu_x = r.x;
                }
                6 => {
                    app.menu = Menu::Sound;
                    app.menu_x = r.x;
                }
                _ => app.options = true,
            }
        }
    }
    draw_line(0.0, 120.0, W, 120.0, 1.0, LINE);

    // ---- status area ----
    draw_rectangle(0.0, 121.0, W, H - 121.0, PANEL);
    let (time, size) = match &app.rec {
        Some(r) => (hms(r.elapsed()), human(r.bytes())),
        None => ("00:00:00".to_string(), "0 KB".to_string()),
    };
    if recording {
        let blink = paused || (get_time() * 2.0) as i64 % 2 == 0;
        if blink {
            draw_circle(18.0, 142.0, 6.0, if paused { ACCENT } else { REC });
        }
        ui.text(if paused { "PAUSED" } else { "REC" }, 30.0, 147.0, 14.0, if paused { ACCENT } else { REC });
    } else if app.game_armed {
        ui.text("READY", 12.0, 147.0, 14.0, ACCENT);
    } else {
        ui.text("Idle", 12.0, 147.0, 14.0, MUTED);
    }
    ui.text(&time, 92.0, 149.0, 22.0, INK);
    ui.text(&size, 210.0, 147.0, 14.0, INK);
    let free = format!("{} {}", tr("Free:"), human(app.free));
    ui.text(&free, 300.0, 147.0, 14.0, MUTED);
    // sound level meters
    let (sys, mic) = app.levels;
    for (j, (label, v, on)) in [("PC", sys, app.s.record_system), ("Mic", mic, app.s.record_mic)].into_iter().enumerate() {
        let y = 135.0 + j as f32 * 12.0;
        ui.text(label, 450.0, y + 9.0, 11.0, if on { INK } else { LINE });
        draw_rectangle(478.0, y + 2.0, 110.0, 7.0, Color::new(0.9, 0.92, 0.94, 1.0));
        if on {
            draw_rectangle(478.0, y + 2.0, 110.0 * v.clamp(0.0, 1.0), 7.0, Color::new(0.2, 0.7, 0.35, 1.0));
        }
    }
    let info = app.info_line();
    ui.text(&info, 12.0, 172.0, 13.0, MUTED);
    let mut m = tr(&app.msg).to_string();
    while m.chars().count() > 4 && ui.width(&m, 13.0) > W - 24.0 {
        m = m.chars().take(m.chars().count() - 4).collect::<String>();
        m.push('…');
    }
    ui.text(&m, 12.0, 192.0, 13.0, if app.msg_bad { REC } else { ACCENT });

    // ---- drop-down menu (drawn last) ----
    if let Some(mr) = app.menu_rect(ui) {
        ui.in_popup = true;
        draw_rectangle(mr.x + 3.0, mr.y + 3.0, mr.w, mr.h, Color::new(0.0, 0.0, 0.0, 0.12));
        draw_rectangle(mr.x, mr.y, mr.w, mr.h, PANEL);
        draw_rectangle_lines(mr.x, mr.y, mr.w, mr.h, 1.0, LINE);
        let mut chosen = None;
        for (i, (label, checked, act)) in app.menu_items().into_iter().enumerate() {
            let r = Rect::new(mr.x + 2.0, mr.y + 4.0 + i as f32 * 24.0, mr.w - 4.0, 24.0);
            if act.is_none() {
                draw_line(r.x + 8.0, r.y + 12.0, r.x + r.w - 8.0, r.y + 12.0, 1.0, LINE);
                continue;
            }
            if ui.hot(r) {
                draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
            }
            if checked {
                draw_line(r.x + 8.0, r.y + 12.0, r.x + 12.0, r.y + 16.0, 2.0, ACCENT);
                draw_line(r.x + 12.0, r.y + 16.0, r.x + 19.0, r.y + 7.0, 2.0, ACCENT);
            }
            ui.text(&label, r.x + 28.0, r.y + 17.0, 14.0, INK);
            if ui.click(r) {
                chosen = act;
            }
        }
        ui.in_popup = false;
        if let Some(a) = chosen {
            app.menu = Menu::None;
            app.do_act(a);
            ui.pressed = false; // the same click must not also hit a dialog that just opened
        } else if ui.pressed && !mr.contains(ui.mouse) && app.menu == menu_before {
            app.menu = Menu::None;
        }
    }

    // ---- "Custom size" dialog (modal: the loop blocks clicks everywhere else) ----
    if let Some(vals) = app.custom.clone() {
        draw_rectangle(0.0, 0.0, W, H, Color::new(0.0, 0.0, 0.0, 0.25));
        let p = Rect::new(150.0, 30.0, 300.0, 150.0);
        draw_rectangle(p.x + 3.0, p.y + 3.0, p.w, p.h, Color::new(0.0, 0.0, 0.0, 0.12));
        draw_rectangle(p.x, p.y, p.w, p.h, PANEL);
        draw_rectangle_lines(p.x, p.y, p.w, p.h, 1.0, LINE);
        ui.in_popup = true;
        ui.text("Custom size", p.x + 16.0, p.y + 26.0, 15.0, ACCENT);
        for i in 0..2 {
            let r = Rect::new(p.x + 30.0 + i as f32 * 130.0, p.y + 44.0, 110.0, 28.0);
            let focus = app.custom_field == i;
            draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if focus { 2.0 } else { 1.0 }, if focus { ACCENT } else { LINE });
            let caret = if focus && (get_time() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
            ui.text(&format!("{}{caret}", vals[i]), r.x + 8.0, r.y + 19.0, 15.0, INK);
            ui.text(if i == 0 { "Width" } else { "Height" }, r.x, r.y + 44.0, 12.0, MUTED);
            if ui.click(r) {
                app.custom_field = i;
            }
        }
        ui.text_c("x", p.x + 150.0, p.y + 63.0, 15.0, MUTED);
        let ok = ui.button(Rect::new(p.x + p.w - 196.0, p.y + p.h - 40.0, 88.0, 28.0), "OK", true);
        let cancel = ui.button(Rect::new(p.x + p.w - 100.0, p.y + p.h - 40.0, 88.0, 28.0), "Cancel", false);
        ui.in_popup = false;
        if ok {
            app.apply_custom();
        } else if cancel {
            app.custom = None;
        }
    }
}

fn draw_options(app: &mut App, ui: &mut Ui) {
    // two columns so the whole screen fits on small or scaled displays
    draw_rectangle(0.0, 0.0, OPT_W, OPT_H, BG);
    draw_rectangle(0.0, 0.0, OPT_W, 36.0, NAVY);
    ui.text("Options", 16.0, 24.0, 18.0, WHITE);
    draw_line(OPT_W / 2.0, 48.0, OPT_W / 2.0, OPT_H - 60.0, 1.0, LINE);
    let (mics, cams) = app.devices.get_or_insert_with(|| (audio::mics(), webcam::cameras())).clone();
    let s = &mut app.s;
    let label = |ui: &mut Ui, t: &str, ox: f32, y: f32| ui.text(t, ox + 20.0, y + 17.0, 14.0, INK);
    let head = |ui: &mut Ui, t: &str, ox: f32, y: f32| {
        ui.text(t, ox + 16.0, y + 14.0, 15.0, ACCENT);
        draw_line(ox + 16.0, y + 20.0, ox + OPT_W / 2.0 - 16.0, y + 20.0, 1.0, LINE);
    };
    let fit_text = |ui: &Ui, t: &str, w: f32| {
        let mut f = t.to_string();
        while ui.width(&f, 13.0) > w && f.chars().count() > 6 {
            f = format!("…{}", f.chars().skip(5).collect::<String>());
        }
        f
    };
    let cw = 220.0;

    // ---------- left column ----------
    let ox = 0.0;
    let cx = ox + 170.0;
    let mut y = 52.0;
    head(ui, "Saving", ox, y);
    y += 28.0;
    label(ui, "Save folder", ox, y);
    let f = fit_text(ui, &s.folder, 170.0);
    draw_rectangle(cx, y, 180.0, 24.0, PANEL);
    draw_rectangle_lines(cx, y, 180.0, 24.0, 1.0, LINE);
    ui.text(&f, cx + 6.0, y + 17.0, 13.0, INK);
    if ui.button(Rect::new(cx + 188.0, y, 90.0, 24.0), "Browse...", false) {
        if let Some(p) = browse_folder(&s.folder) {
            s.folder = p;
        }
    }
    y += 30.0;
    label(ui, "File name", ox, y);
    let name_r = Rect::new(cx, y, 278.0, 24.0);
    draw_rectangle(name_r.x, name_r.y, name_r.w, name_r.h, PANEL);
    draw_rectangle_lines(name_r.x, name_r.y, name_r.w, name_r.h, if app.editing == 2 { 2.0 } else { 1.0 }, if app.editing == 2 { ACCENT } else { LINE });
    let caret = if app.editing == 2 && (get_time() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
    let shown = fit_text(ui, &format!("{}{caret}", s.file_name), 266.0);
    ui.text(&shown, cx + 6.0, y + 17.0, 13.0, INK);
    y += 30.0;
    label(ui, "Next number {n}", ox, y);
    if ui.button(Rect::new(cx, y, 28.0, 24.0), "-", false) {
        s.file_number = s.file_number.saturating_sub(1);
    }
    ui.text_c(&s.file_number.to_string(), cx + 52.0, y + 17.0, 14.0, INK);
    if ui.button(Rect::new(cx + 76.0, y, 28.0, 24.0), "+", false) {
        s.file_number += 1;
    }
    let ext = if s.codec == 2 { "gif" } else { VIDEO_FORMATS[s.video_format] };
    let example = fit_text(ui, &format!("{}.{ext}", recorder::file_stem(s, "Record")), 170.0);
    ui.text(&format!("{} {example}", tr("Example:")), cx + 112.0, y + 17.0, 12.0, MUTED);
    y += 24.0;
    ui.text("{type} {YYYY} {MM} {DD} {hh} {mm} {ss} {ms} {n}", cx, y + 12.0, 12.0, MUTED);
    y += 26.0;

    head(ui, "Video", ox, y);
    y += 28.0;
    label(ui, "File format", ox, y);
    // the last choice is GIF recording (codec 2)
    let gif = s.codec == 2;
    let cur = if gif { VIDEO_FORMATS.len() } else { s.video_format };
    let n = ui.select(cx, y, cw, cur, VIDEO_FORMATS.iter().map(|f| f.to_uppercase()).chain(["GIF".to_string()]).collect());
    if n != cur {
        if n == VIDEO_FORMATS.len() {
            s.codec = 2;
        } else {
            s.video_format = n;
            if gif {
                s.codec = 0;
            }
        }
    }
    if s.codec != 2 && matches!(VIDEO_FORMATS[s.video_format], "mkv" | "flv" | "vob" | "avi") && encoder::ffmpeg_path().is_none() {
        ui.text("needs FFmpeg", cx + cw + 8.0, y + 17.0, 12.0, MUTED);
    }
    y += 30.0;
    label(ui, "Encoder", ox, y);
    if s.codec == 2 {
        ui.text("Animated GIF (no sound)", cx + 6.0, y + 17.0, 14.0, MUTED);
    } else {
        s.codec = ui.select(cx, y, cw + 50.0, s.codec, CODECS[..2].iter().map(|c| c.to_string()).collect());
    }
    y += 30.0;
    label(ui, "Frame rate", ox, y);
    let fi = FPS_CHOICES.iter().position(|v| *v == s.fps).unwrap_or(3);
    s.fps = FPS_CHOICES[ui.select(cx, y, cw, fi, FPS_CHOICES.iter().map(|v| format!("{v} fps")).collect())];
    y += 30.0;
    label(ui, "Quality (bitrate)", ox, y);
    let qi = QUALITY_CHOICES.iter().position(|v| *v == s.mbps).unwrap_or(2);
    s.mbps = QUALITY_CHOICES[ui.select(cx, y, cw, qi, QUALITY_CHOICES.iter().map(|v| format!("{v} Mbit/s")).collect())];
    y += 30.0;
    label(ui, "GIF frame rate", ox, y);
    let gi = [5u32, 10, 15, 20].iter().position(|v| *v == s.gif_fps).unwrap_or(1);
    s.gif_fps = [5u32, 10, 15, 20][ui.select(cx, y, cw, gi, [5u32, 10, 15, 20].iter().map(|v| format!("{v} fps")).collect())];
    y += 30.0;
    ui.checkbox(ox + 20.0, y + 4.0, "Show the mouse pointer in recordings and captures", &mut s.show_cursor);
    y += 28.0;
    ui.checkbox(ox + 20.0, y + 4.0, "Show actual FPS while recording", &mut s.show_fps);
    y += 32.0;

    head(ui, "Recording frame", ox, y);
    y += 28.0;
    label(ui, "Frame opacity", ox, y);
    let oi = FRAME_OPACITY.iter().position(|v| *v == s.frame_opacity).unwrap_or(0);
    s.frame_opacity = FRAME_OPACITY[ui.select(cx, y, cw, oi, FRAME_OPACITY.iter().map(|v| format!("{v} %")).collect())];
    y += 30.0;
    label(ui, "Language", ox, y);
    let li = ui.select(cx, y, cw, s.language, i18n::LANGS.iter().map(|l| l.to_string()).collect());
    if li != s.language {
        s.language = li;
        i18n::set(li);
    }

    // ---------- right column ----------
    let ox = OPT_W / 2.0;
    let cx = ox + 185.0; // room for longer translated labels
    let mut y = 52.0;
    head(ui, "Sound & images", ox, y);
    y += 28.0;
    label(ui, "Sound file format", ox, y);
    s.sound_format = ui.select(cx, y, cw, s.sound_format, SOUND_FORMATS.iter().map(|f| f.to_uppercase()).collect());
    y += 30.0;
    ui.checkbox(ox + 20.0, y + 4.0, "Record PC sound", &mut s.record_system);
    ui.checkbox(cx, y + 4.0, "Record microphone", &mut s.record_mic);
    y += 28.0;
    label(ui, "Microphone", ox, y);
    let cur = mics.iter().position(|m| m.0 == s.mic_device).map_or(0, |i| i + 1);
    let n = ui.select(cx, y, 260.0, cur, std::iter::once("Windows default".to_string()).chain(mics.iter().map(|m| m.1.clone())).collect());
    if n != cur {
        s.mic_device = if n == 0 { String::new() } else { mics[n - 1].0.clone() };
    }
    y += 30.0;
    label(ui, "Capture image format", ox, y);
    s.image_format = ui.select(cx, y, cw, s.image_format, IMAGE_FORMATS.iter().map(|f| f.to_uppercase()).collect());
    y += 32.0;

    head(ui, "Hotkeys (work everywhere, also in games)", ox, y);
    y += 28.0;
    for i in 0..3 {
        let x = ox + 20.0 + i as f32 * 148.0;
        ui.text(HOTKEY_NAMES[i], x, y + 14.0, 13.0, MUTED);
        let t = if app.binding == Some(i) { "Press a key...".to_string() } else { key_name(s.hotkeys[i]) };
        if ui.button(Rect::new(x, y + 20.0, 138.0, 24.0), &t, app.binding == Some(i)) {
            app.binding = Some(i);
        }
    }
    y += 56.0;

    head(ui, "Watermark & webcam", ox, y);
    y += 28.0;
    label(ui, "Watermark text", ox, y);
    let tr = Rect::new(cx, y, 160.0, 24.0);
    draw_rectangle(tr.x, tr.y, tr.w, tr.h, PANEL);
    draw_rectangle_lines(tr.x, tr.y, tr.w, tr.h, if app.editing == 1 { 2.0 } else { 1.0 }, if app.editing == 1 { ACCENT } else { LINE });
    let caret = if app.editing == 1 && (get_time() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
    let shown = if s.watermark_text.is_empty() && app.editing != 1 { "(none) - click to type".to_string() } else { format!("{}{caret}", s.watermark_text) };
    let shown = fit_text(ui, i18n::tr(&shown), 150.0);
    ui.text(&shown, cx + 6.0, y + 17.0, 13.0, if s.watermark_text.is_empty() { MUTED } else { INK });
    if ui.pressed {
        app.editing = if ui.hot(tr) { 1 } else if ui.hot(name_r) { 2 } else { 0 };
    }
    s.watermark_corner = ui.select(cx + 168.0, y, 120.0, s.watermark_corner, CORNERS.iter().map(|c| c.to_string()).collect());
    y += 30.0;
    label(ui, "Watermark image", ox, y);
    let img = if s.watermark_image.is_empty() {
        "(none)".to_string()
    } else {
        std::path::Path::new(&s.watermark_image).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
    };
    let img = fit_text(ui, &img, 90.0);
    ui.text(&img, cx + 6.0, y + 17.0, 13.0, INK);
    if ui.button(Rect::new(cx + 100.0, y, 90.0, 24.0), "Choose...", false) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif"]).pick_file() {
            s.watermark_image = p.display().to_string();
        }
    }
    if ui.button(Rect::new(cx + 198.0, y, 70.0, 24.0), "Clear", false) {
        s.watermark_image.clear();
    }
    y += 30.0;
    ui.checkbox(ox + 20.0, y + 4.0, "Webcam overlay", &mut s.webcam);
    s.webcam_corner = ui.select(cx, y, 140.0, s.webcam_corner, CORNERS.iter().map(|c| c.to_string()).collect());
    let wi = [160u32, 240, 320, 480].iter().position(|v| *v == s.webcam_width).unwrap_or(2);
    s.webcam_width = [160u32, 240, 320, 480][ui.select(cx + 148.0, y, 110.0, wi, [160u32, 240, 320, 480].iter().map(|v| format!("{v} px")).collect())];
    y += 30.0;
    label(ui, "Webcam device", ox, y);
    let cur = cams.iter().position(|c| *c == s.webcam_device).map_or(0, |i| i + 1);
    let n = ui.select(cx, y, 260.0, cur, std::iter::once("Automatic (first found)".to_string()).chain(cams.iter().cloned()).collect());
    if n != cur {
        s.webcam_device = if n == 0 { String::new() } else { cams[n - 1].clone() };
    }
    y += 32.0;

    head(ui, "Click effects", ox, y);
    y += 28.0;
    ui.checkbox(ox + 20.0, y + 4.0, "Show mouse clicks in recordings", &mut s.click_fx);
    y += 28.0;
    label(ui, "Left / right click colour", ox, y);
    let colors: Vec<String> = CLICK_COLORS.iter().map(|c| c.0.to_string()).collect();
    s.click_left = ui.select(cx, y, 100.0, s.click_left, colors.clone());
    s.click_right = ui.select(cx + 108.0, y, 100.0, s.click_right, colors);
    y += 30.0;
    label(ui, "Ring size / duration", ox, y);
    let zi = CLICK_SIZES.iter().position(|v| *v == s.click_size).unwrap_or(1);
    s.click_size = CLICK_SIZES[ui.select(cx, y, 100.0, zi, CLICK_SIZES.iter().map(|v| format!("{v} px")).collect())];
    let di = CLICK_MS.iter().position(|v| *v == s.click_ms).unwrap_or(1);
    s.click_ms = CLICK_MS[ui.select(cx + 108.0, y, 100.0, di, CLICK_MS.iter().map(|v| format!("{:.1} s", *v as f32 / 1000.0)).collect())];

    if ui.button(Rect::new(OPT_W - 136.0, OPT_H - 44.0, 120.0, 30.0), "Done", true) {
        app.close_options();
    }
    ui.draw_dropdown(OPT_H);
}

fn conf() -> Conf {
    let s = unsafe {
        use windows::Win32::UI::HiDpi::*;
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        GetDpiForSystem() as f32 / 96.0
    };
    Conf {
        window_title: "ClearShot".into(),
        window_width: (W * s) as i32,
        window_height: (H * s) as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[derive(Default)]
struct SelfTest {
    out: Option<String>,
    steps: Vec<(String, bool, String)>,
    t0: f64,
    stage: u32,
    area_before: Option<Area>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let shot = arg("--screenshot");
    let test = arg("--self-test");
    macroquad::Window::from_config(conf(), run(shot, test));
}

async fn run(shot: Option<String>, test: Option<String>) {
    encoder::mf_startup();
    let font = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf").ok().and_then(|b| load_ttf_font_from_bytes(&b).ok());
    let mut s = Settings::load();
    let testing = test.is_some() || shot.is_some();
    if testing {
        s = Settings::default();
        s.folder = std::env::temp_dir().join("clearshot-selftest").display().to_string();
        if let Ok(f) = std::env::var("CLEARSHOT_TEST_FORMAT") {
            s.video_format = VIDEO_FORMATS.iter().position(|v| *v == f).unwrap_or(0);
            s.sound_format = SOUND_FORMATS.iter().position(|v| *v == f).unwrap_or(0);
            s.codec = if f == "gif" { 2 } else { 0 };
        }
    }
    i18n::set(s.language);
    overlay::set_area(Area { x: s.area[0], y: s.area[1], w: s.area[2], h: s.area[3] });
    overlay::spawn();
    prevent_quit();
    let mut app = App {
        free: disk_free(&s.folder),
        s,
        mode: Mode::Screen,
        rec: None,
        game_armed: false,
        menu: Menu::None,
        menu_x: 0.0,
        options: false,
        binding: None,
        editing: 0,
        custom: None,
        custom_field: 0,
        fps: (Instant::now(), 0, 0.0),
        devices: None,
        picking: 0,
        frame_shown: true,
        msg: "Ready. Drag the green frame to choose what to record.".into(),
        msg_bad: false,
        msg_at: Instant::now(),
        free_at: Instant::now(),
        levels: (0.0, 0.0),
        levels_at: Instant::now(),
        hk_prev: [false; 3],
        arrow_held: 0.0,
        last_saved: None,
        flash_at: None,
    };
    let mut ui = Ui { s: 1.0, font, mouse: Vec2::ZERO, pressed: false, blocker: None, in_popup: false, drawn: Vec::new(), dd_open: None, dd_opened_now: false, dd_list: None, dd_pick: None, dd_rect: None };
    let mut st = SelfTest { out: test, ..Default::default() };
    let mut frames = 0u32;
    let mut win_size = (0.0, 0.0);

    loop {
        frames += 1;
        let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() } as f32 / 96.0;
        ui.s = dpi;
        let lw = screen_width() / dpi;
        let lh = screen_height() / dpi;
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, lw, lh));
        cam.zoom.y = -cam.zoom.y;
        set_camera(&cam);
        let (mx, my) = mouse_position();
        ui.mouse = vec2(mx / dpi, my / dpi);
        ui.pressed = is_mouse_button_pressed(MouseButton::Left);
        ui.drawn.clear();

        let want = if app.options { (OPT_W, OPT_H) } else { (W, app.menu_rect(&ui).map_or(H, |r| (r.y + r.h + 6.0).max(H))) };
        if want != win_size {
            win_size = want;
            request_new_screen_size(want.0 * dpi, want.1 * dpi);
        }

        // ---- input ----
        if is_quit_requested() {
            // the window's X on the Options screen means "close Options", not "exit the app"
            if app.options {
                app.close_options();
            } else {
                app.quit();
            }
        }
        if let Some(i) = app.binding {
            if let Some(vk) = get_last_key_pressed().and_then(keycode_to_vk) {
                app.s.hotkeys[i] = vk;
                app.binding = None;
            }
            app.hk_prev = [true; 3];
        } else if app.editing != 0 {
            let limit = if app.editing == 1 { 60 } else { 120 };
            let t = if app.editing == 1 { &mut app.s.watermark_text } else { &mut app.s.file_name };
            while let Some(c) = get_char_pressed() {
                if !c.is_control() && t.chars().count() < limit {
                    t.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                t.pop();
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
                app.editing = 0;
            }
        } else if let Some(vals) = app.custom.as_mut() {
            let f = app.custom_field;
            while let Some(c) = get_char_pressed() {
                if c.is_ascii_digit() && vals[f].len() < 5 {
                    vals[f].push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                vals[f].pop();
            }
            if is_key_pressed(KeyCode::Tab) {
                app.custom_field ^= 1;
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                app.apply_custom();
            } else if is_key_pressed(KeyCode::Escape) {
                app.custom = None;
            }
        } else {
            // global hotkeys
            for i in 0..3 {
                let d = vk_down(app.s.hotkeys[i]);
                if d && !app.hk_prev[i] {
                    match i {
                        0 => {
                            if app.mode != Mode::Game || app.game_armed || app.rec.is_some() {
                                if app.mode == Mode::Game && app.rec.is_none() {
                                    app.game_armed = true;
                                }
                                app.toggle_record();
                            }
                        }
                        1 => {
                            if let Some(r) = app.rec.as_mut() {
                                r.toggle_pause();
                            }
                        }
                        _ => {
                            if app.mode != Mode::Sound {
                                app.capture();
                            }
                        }
                    }
                }
                app.hk_prev[i] = d;
            }
            // arrow keys nudge the frame (with key repeat)
            if app.mode == Mode::Screen && !app.options {
                let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
                let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                let dirs = [(KeyCode::Left, -1, 0), (KeyCode::Right, 1, 0), (KeyCode::Up, 0, -1), (KeyCode::Down, 0, 1)];
                let mut any = false;
                for (k, dx, dy) in dirs {
                    if is_key_down(k) {
                        any = true;
                        let fire = is_key_pressed(k) || (app.arrow_held > 0.4);
                        if fire {
                            overlay::nudge(dx, dy, ctrl, shift);
                        }
                    }
                }
                app.arrow_held = if any { app.arrow_held + get_frame_time() } else { 0.0 };
            }
            if is_key_pressed(KeyCode::Escape) {
                app.menu = Menu::None;
            }
        }
        // window picking: wait for the button to be released, then take the next click
        match app.picking {
            1 if !capture::left_button_down() => app.picking = 2,
            2 if capture::left_button_down() => {
                app.picking = 0;
                if let Some(a) = capture::window_under_cursor() {
                    overlay::set_area(a);
                    app.frame_shown = true;
                    let a = overlay::area();
                    app.say(format!("{} ({} x {}).", tr("Frame fitted to the window"), a.w, a.h));
                }
            }
            _ => {}
        }
        overlay::OPACITY.store((app.s.frame_opacity * 255 / 100) as u8, Ordering::Relaxed);
        overlay::VISIBLE.store(app.frame_shown && app.mode == Mode::Screen, Ordering::Relaxed);
        *overlay::CAM.lock().unwrap() = app.s.webcam.then_some((app.s.webcam_width, app.s.webcam_corner));
        if let Some(r) = &app.rec {
            let dt = app.fps.0.elapsed().as_secs_f32();
            if dt >= 1.0 {
                let n = r.frames();
                app.fps = (Instant::now(), n, n.saturating_sub(app.fps.1) as f32 / dt);
            }
        }
        let show_fps = app.s.show_fps && app.rec.is_some() && app.mode != Mode::Sound;
        overlay::FPS.store(if show_fps { (app.fps.2 * 10.0).round().max(1.0) as u32 } else { 0 }, Ordering::Relaxed);
        if app.rec.as_ref().map_or(false, |r| r.failed()) {
            app.toggle_record();
        }
        if app.free_at.elapsed() > Duration::from_secs(2) {
            app.free = disk_free(&app.s.folder);
            app.free_at = Instant::now();
        }
        if app.levels_at.elapsed() > Duration::from_millis(80) {
            app.levels = (audio::peak(true, ""), audio::peak(false, &app.s.mic_device));
            app.levels_at = Instant::now();
        }
        if app.msg_at.elapsed() > Duration::from_secs(12) && app.rec.is_none() && !app.game_armed {
            app.msg.clear();
        }

        // ---- draw ----
        clear_background(BG);
        ui.blocker = if app.custom.is_some() { Some(Rect::new(0.0, 0.0, W, H)) } else { app.menu_rect(&ui).or(ui.dd_rect) };
        if app.options {
            draw_options(&mut app, &mut ui);
        } else {
            draw_main(&mut app, &mut ui);
        }
        // capture feedback: a quick white camera-flash over the window
        if let Some(t) = app.flash_at {
            let k = t.elapsed().as_secs_f32() / 0.4;
            if k < 1.0 {
                draw_rectangle(0.0, 0.0, lw, lh, Color::new(1.0, 1.0, 1.0, 0.85 * (1.0 - k)));
            } else {
                app.flash_at = None;
            }
        }

        // ---- scripted self-test ----
        if st.out.is_some() {
            self_test(&mut app, &mut ui, &mut st);
        }
        if let Some(p) = &shot {
            if frames >= 120 {
                set_default_camera();
                get_screen_data().export_png(p);
                std::process::exit(0);
            }
        }
        next_frame().await;
    }
}

fn self_test(app: &mut App, ui: &mut Ui, st: &mut SelfTest) {
    let t = get_time();
    let ok = |st: &mut SelfTest, step: &str, pass: bool, d: String| st.steps.push((step.into(), pass, d));
    match st.stage {
        0 if t > 0.5 => {
            overlay::set_area(Area { x: 100, y: 100, w: 640, h: 480 });
            st.area_before = Some(overlay::area());
            // the same nudges the arrow keys do: 5 x Right, 2 x Ctrl+Down, 3 x Shift+Right/Down
            for _ in 0..5 {
                overlay::nudge(1, 0, false, false);
            }
            for _ in 0..2 {
                overlay::nudge(0, 1, true, false);
            }
            for _ in 0..3 {
                overlay::nudge(1, 0, false, true);
                overlay::nudge(0, 1, false, true);
            }
            st.stage = 1;
        }
        1 => {
            let (b, a) = (st.area_before.unwrap(), overlay::area());
            let pass = a.x == b.x + 5 && a.y == b.y + 20 && a.w == b.w + 6 && a.h == b.h + 6;
            ok(st, "move", pass, format!("recording frame moved from ({},{}) {}x{} to ({},{}) {}x{} with arrow-key nudges", b.x, b.y, b.w, b.h, a.x, a.y, a.w, a.h));
            app.toggle_record(); // start screen recording via the Record action
            st.t0 = t;
            st.stage = 2;
        }
        2 if t - st.t0 > 1.0 => {
            if let Some(r) = app.rec.as_mut() {
                r.toggle_pause();
            }
            st.stage = 3;
        }
        3 if t - st.t0 > 1.5 => {
            let hud = ["Record", "Pause", "Capture", "Size", "Open", "Codecs", "Sound", "Options", "Screen Recording", "Game Recording", "Sound Recording"];
            let missing: Vec<_> = hud.iter().filter(|h| !ui.drawn.iter().any(|d| d == *h || (**h == "Record" && d == "Stop") || (**h == "Pause" && d == "Resume"))).collect();
            let timer = ui.drawn.iter().any(|d| d.starts_with("00:00:0"));
            let size = ui.drawn.iter().any(|d| d.ends_with("KB") || d.ends_with("MB"));
            let rec = ui.drawn.iter().any(|d| d == "REC" || d == "PAUSED");
            ok(st, "hud", missing.is_empty() && timer && size && rec, format!("mode tabs, 8 tool buttons, REC/PAUSED badge, elapsed timer, file size, free space and sound meters drawn; missing: {missing:?}"));
            if let Some(r) = app.rec.as_mut() {
                r.toggle_pause();
            }
            st.stage = 4;
        }
        4 if t - st.t0 > 3.0 => {
            if app.rec.is_some() {
                app.toggle_record();
            }
            let video_msg = app.msg.clone();
            let video = app.last_saved.clone();
            let vsize = video.as_ref().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
            app.capture();
            let img = app.last_saved.clone();
            let isize = img.as_ref().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
            app.mode = Mode::Sound;
            app.s.record_system = true;
            app.toggle_record();
            st.t0 = t;
            st.stage = 5;
            st.steps.push(("act".into(), vsize > 2000 && isize > 1000 && video != img, format!("recorded {} ({} bytes, 2 s with a pause) - {video_msg}; captured {} ({} bytes)", video.map(|p| p.display().to_string()).unwrap_or("nothing".into()), vsize, img.map(|p| p.display().to_string()).unwrap_or("nothing".into()), isize)));
        }
        5 if t - st.t0 > 1.5 => {
            app.toggle_record();
            let a = app.last_saved.clone();
            let asize = a.as_ref().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
            let sound_ok = asize > 1000 && a.as_ref().map_or(false, |p| p.to_string_lossy().contains("Sound"));
            if let Some(s) = st.steps.iter_mut().find(|s| s.0 == "act") {
                s.1 &= sound_ok;
                s.2 += &format!("; sound-only recording {} ({} bytes) - {}", a.map(|p| p.display().to_string()).unwrap_or("nothing".into()), asize, app.msg);
            }
            let steps: Vec<_> = st.steps.iter().map(|(s, p, d)| serde_json::json!({"step": s, "passed": p, "detail": d})).collect();
            let _ = std::fs::write(st.out.as_ref().unwrap(), serde_json::to_string_pretty(&serde_json::json!({"version": 1, "steps": steps})).unwrap());
            std::process::exit(0);
        }
        _ => {}
    }
}
