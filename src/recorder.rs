//! Ties capture, sound and the file writer together for one recording.
use crate::capture::{self, Area, ClickFx, Grabber, Overlay};
use crate::encoder::{self, Job, Msg};
use crate::settings::{Settings, CLICK_COLORS, DEFAULT_FILE_NAME, SOUND_FORMATS, VIDEO_FORMATS};
use crate::{audio, overlay, webcam};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Screen,
    Game,
    Sound,
}

pub struct Recording {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    started: Instant,
    paused_total: Duration,
    pause_at: Option<Instant>,
    pub path: PathBuf,
    workers: Vec<JoinHandle<()>>,
    tx: SyncSender<Msg>,
    enc: JoinHandle<Result<PathBuf, String>>,
    /// video frames sent to the writer so far (for the actual-FPS readout)
    frames: Arc<AtomicU32>,
}

/// File name (without extension) from the pattern in Options, for "Record", "Capture" or "Sound".
pub fn file_stem(s: &Settings, kind: &str) -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    expand_name(&s.file_name, kind, [t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds], s.file_number)
}

/// Tokens: {type} {YYYY} {MM} {DD} {hh} {mm} {ss} {ms} {n}. Characters Windows forbids become "_".
pub fn expand_name(pattern: &str, kind: &str, t: [u16; 7], n: u32) -> String {
    let p = if pattern.trim().is_empty() { DEFAULT_FILE_NAME } else { pattern };
    let out = p
        .replace("{type}", kind)
        .replace("{YYYY}", &format!("{:04}", t[0]))
        .replace("{MM}", &format!("{:02}", t[1]))
        .replace("{DD}", &format!("{:02}", t[2]))
        .replace("{hh}", &format!("{:02}", t[3]))
        .replace("{mm}", &format!("{:02}", t[4]))
        .replace("{ss}", &format!("{:02}", t[5]))
        .replace("{ms}", &format!("{:03}", t[6]))
        .replace("{n}", &n.to_string());
    let out: String = out.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let out = out.trim().trim_end_matches('.').to_string();
    if out.is_empty() { kind.to_string() } else { out }
}

/// Widens each BGRA row to `to` pixels by repeating its last pixel; the encoder crops it off again.
fn pad_width(px: Vec<u8>, w: i32, to: i32) -> Vec<u8> {
    let (w, to) = (w as usize * 4, to as usize * 4);
    let mut out = Vec::with_capacity(px.len() / w * to);
    for row in px.chunks_exact(w) {
        out.extend_from_slice(row);
        for _ in 0..(to - w) / 4 {
            out.extend_from_slice(&row[w - 4..]);
        }
    }
    out
}

/// "name.ext", or "name (2).ext" ... if it already exists.
fn unique(dir: PathBuf, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{stem} ({i}).{ext}"));
        i += 1;
    }
    p
}

pub fn out_dir(s: &Settings) -> PathBuf {
    let d = PathBuf::from(&s.folder);
    let _ = std::fs::create_dir_all(&d);
    d
}

fn overlay_of(s: &Settings) -> Overlay {
    let image = (!s.watermark_image.is_empty())
        .then(|| image::open(&s.watermark_image).ok())
        .flatten()
        .map(|i| {
            let i = i.to_rgba8();
            (i.width(), i.height(), i.into_raw())
        });
    Overlay { text: s.watermark_text.clone(), image, corner: s.watermark_corner }
}

impl Recording {
    pub fn start(mode: Mode, s: &Settings, area: Area) -> Recording {
        let stop = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let gif = mode != Mode::Sound && s.codec == 2;
        let ext = match mode {
            Mode::Sound => SOUND_FORMATS[s.sound_format],
            _ if gif => "gif",
            _ => VIDEO_FORMATS[s.video_format],
        };
        let path = unique(out_dir(s), &file_stem(s, if mode == Mode::Sound { "Sound" } else { "Record" }), ext);
        let area = capture::fit(area);
        let fps = if gif { s.gif_fps } else { s.fps };
        let has_audio = !gif && (s.record_system || s.record_mic);
        // Windows' H.264 path skews widths that are not a multiple of 16 (seen at 646/648 px), so frames are padded
        let enc_w = if gif { area.w } else { (area.w + 15) & !15 };
        let frames = Arc::new(AtomicU32::new(0));
        let job = Job { path: path.clone(), w: area.w as u32, enc_w: enc_w as u32, h: area.h as u32, fps, mbps: s.mbps, audio: has_audio, video: mode != Mode::Sound, gpu: s.codec == 1, gif };
        let (tx, enc) = encoder::spawn(job);
        let mut workers = Vec::new();
        if has_audio {
            let atx = tx.clone();
            if let Some(h) = audio::start_mixer(s.record_system, s.record_mic, &s.mic_device, stop.clone(), paused.clone(), move |pcm| {
                let _ = atx.send(Msg::Audio(pcm));
            }) {
                workers.push(h);
            }
        }
        if mode != Mode::Sound {
            let (vtx, st, pa, fr) = (tx.clone(), stop.clone(), paused.clone(), frames.clone());
            let fx = s.click_fx.then(|| ClickFx {
                left: CLICK_COLORS[s.click_left.min(CLICK_COLORS.len() - 1)].1,
                right: CLICK_COLORS[s.click_right.min(CLICK_COLORS.len() - 1)].1,
                size: s.click_size as i32,
                dur: Duration::from_millis(s.click_ms.max(100) as u64),
            });
            let ov = overlay_of(s);
            let cam = s.webcam.then(|| webcam::start(stop.clone(), s.webcam_device.clone()));
            let (cursor, cam_w, cam_corner) = (s.show_cursor, s.webcam_width, s.webcam_corner);
            workers.push(std::thread::spawn(move || {
                let mut g = Grabber::new(area.w, area.h);
                let frame = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
                let mut active = Duration::ZERO;
                let mut last = Instant::now();
                let mut next = Duration::ZERO;
                while !st.load(Ordering::Relaxed) {
                    let now = Instant::now();
                    if !pa.load(Ordering::Relaxed) {
                        active += now - last;
                    }
                    last = now;
                    if pa.load(Ordering::Relaxed) || active < next {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    next += frame;
                    if next < active {
                        next = active; // fell behind: skip ahead instead of bursting
                    }
                    // the frame can be moved while recording; game mode follows the game window
                    let pos = if mode == Mode::Game { capture::foreground_window().map(|a| (a.x, a.y)).unwrap_or((area.x, area.y)) } else { let a = overlay::area(); (a.x, a.y) };
                    let camf = cam.as_ref().and_then(|c| c.lock().unwrap().clone());
                    let px = g.grab(pos.0, pos.1, cursor, &ov, camf.as_ref(), cam_w, cam_corner, fx.as_ref());
                    let px = if enc_w == area.w { px } else { pad_width(px, area.w, enc_w) };
                    if vtx.send(Msg::Video(px, (active.as_nanos() / 100) as i64)).is_err() {
                        break;
                    }
                    fr.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        Recording { stop, paused, started: Instant::now(), paused_total: Duration::ZERO, pause_at: None, path, workers, tx, enc, frames }
    }

    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.pause_at.is_some()
    }

    pub fn toggle_pause(&mut self) {
        if let Some(p) = self.pause_at.take() {
            self.paused_total += p.elapsed();
            self.paused.store(false, Ordering::Relaxed);
        } else {
            self.pause_at = Some(Instant::now());
            self.paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn elapsed(&self) -> Duration {
        let end = self.pause_at.unwrap_or_else(Instant::now);
        (end - self.started).saturating_sub(self.paused_total)
    }

    pub fn bytes(&self) -> u64 {
        let tmp = self.path.with_extension("tmp.mp4");
        std::fs::metadata(&self.path).or_else(|_| std::fs::metadata(tmp)).map(|m| m.len()).unwrap_or(0)
    }

    /// The writer stopped by itself (it could not open the file or encoder).
    pub fn failed(&self) -> bool {
        self.enc.is_finished()
    }

    /// Stops and finishes the file (also used for the auto-save on exit).
    pub fn stop(self) -> Result<PathBuf, String> {
        self.stop.store(true, Ordering::Relaxed);
        for w in self.workers {
            let _ = w.join();
        }
        let _ = self.tx.send(Msg::Finish);
        drop(self.tx);
        self.enc.join().unwrap_or_else(|_| Err("writer crashed".into()))
    }
}

/// One-click screenshot of the area, saved in the chosen image format.
pub fn screenshot(s: &Settings, area: Area) -> Result<PathBuf, String> {
    let a = capture::fit(area);
    let mut g = Grabber::new(a.w, a.h);
    let px = g.grab(a.x, a.y, s.show_cursor, &overlay_of(s), None, 0, 0, None);
    let img = image::RgbaImage::from_raw(a.w as u32, a.h as u32, capture::bgra_to_rgba(px)).unwrap();
    let ext = crate::settings::IMAGE_FORMATS[s.image_format];
    let path = unique(out_dir(s), &file_stem(s, "Capture"), ext);
    let r = match ext {
        "jpg" | "bmp" => image::DynamicImage::ImageRgba8(img).to_rgb8().save(&path),
        _ => img.save(&path),
    };
    r.map(|_| path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_pattern() {
        let t = [2026, 11, 3, 9, 5, 7, 42];
        assert_eq!(expand_name(DEFAULT_FILE_NAME, "Record", t, 1), "Record 2026-11-03 09.05.07");
        assert_eq!(expand_name("{type}_{YYYY}_{MM}_{DD}_{hh}_{mm}_{ss}_{ms}_{n}", "Capture", t, 12), "Capture_2026_11_03_09_05_07_042_12");
        assert_eq!(expand_name("a:b?", "Sound", t, 1), "a_b_");
        assert_eq!(expand_name("  ", "Sound", t, 1), "Sound 2026-11-03 09.05.07");
    }

    #[test]
    fn padding() {
        let px: Vec<u8> = (0..2 * 2 * 4).map(|i| i as u8).collect(); // 2x2 BGRA
        let out = pad_width(px, 2, 4);
        assert_eq!(out.len(), 4 * 2 * 4);
        assert_eq!(&out[8..16], &[4, 5, 6, 7, 4, 5, 6, 7]); // row 0 repeats its last pixel
        assert_eq!(&out[16..24], &[8, 9, 10, 11, 12, 13, 14, 15]);
    }
}
