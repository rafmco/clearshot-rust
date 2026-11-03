use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const VIDEO_FORMATS: [&str; 9] = ["mp4", "mkv", "mov", "wmv", "flv", "avi", "ts", "vob", "m4v"];
pub const SOUND_FORMATS: [&str; 3] = ["mp3", "m4a", "wav"];
pub const IMAGE_FORMATS: [&str; 4] = ["png", "jpg", "gif", "bmp"];
pub const CODECS: [&str; 3] = ["Built-in H.264", "GPU H.264 (NVENC/QSV/AMF)", "Animated GIF"];
pub const FPS_CHOICES: [u32; 6] = [15, 20, 24, 30, 50, 60];
pub const QUALITY_CHOICES: [u32; 5] = [2, 4, 8, 12, 20]; // Mbit/s
pub const FRAME_OPACITY: [u32; 4] = [100, 75, 50, 25];
pub const CORNERS: [&str; 4] = ["Top left", "Top right", "Bottom left", "Bottom right"];
pub const HOTKEY_NAMES: [&str; 3] = ["Record / Stop", "Pause / Resume", "Capture"];
pub const DEFAULT_FILE_NAME: &str = "{type} {YYYY}-{MM}-{DD} {hh}.{mm}.{ss}";
/// click effect colours as Windows COLORREF (0x00BBGGRR)
pub const CLICK_COLORS: [(&str, u32); 5] = [("Red", 0x0000FF), ("Blue", 0xFF6020), ("Green", 0x30C040), ("Yellow", 0x00D7FF), ("White", 0xFFFFFF)];
pub const CLICK_SIZES: [u32; 4] = [40, 60, 80, 100];
pub const CLICK_MS: [u32; 4] = [300, 500, 800, 1000];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub folder: String,
    pub video_format: usize,
    pub sound_format: usize,
    pub image_format: usize,
    pub codec: usize,
    pub fps: u32,
    pub mbps: u32,
    pub gif_fps: u32,
    pub record_system: bool,
    pub record_mic: bool,
    pub show_cursor: bool,
    /// virtual-key codes for record, pause, capture
    pub hotkeys: [u32; 3],
    pub watermark_text: String,
    pub watermark_image: String,
    pub watermark_corner: usize,
    pub webcam: bool,
    pub webcam_corner: usize,
    pub webcam_width: u32,
    pub area: [i32; 4],
    /// recording frame opacity in percent
    pub frame_opacity: u32,
    /// index into i18n::LANGS
    pub language: usize,
    /// output file name pattern, see recorder::expand_name
    pub file_name: String,
    /// value of {n} in the next file name
    pub file_number: u32,
    pub click_fx: bool,
    /// indexes into CLICK_COLORS
    pub click_left: usize,
    pub click_right: usize,
    pub click_size: u32,
    pub click_ms: u32,
    pub show_fps: bool,
    /// microphone endpoint id; empty = Windows default
    pub mic_device: String,
    /// webcam name; empty = the first one found
    pub webcam_device: String,
}

impl Default for Settings {
    fn default() -> Self {
        let docs = std::env::var("USERPROFILE").map(|p| format!("{p}\\Documents\\ClearShot")).unwrap_or("ClearShot".into());
        Settings {
            folder: docs,
            video_format: 0,
            sound_format: 0,
            image_format: 0,
            codec: 0,
            fps: 30,
            mbps: 8,
            gif_fps: 10,
            record_system: true,
            record_mic: false,
            show_cursor: true,
            hotkeys: [0x71, 0x72, 0x7A], // F2, F3, F11
            watermark_text: String::new(),
            watermark_image: String::new(),
            watermark_corner: 3,
            webcam: false,
            webcam_corner: 3,
            webcam_width: 320,
            area: [200, 150, 1280, 720],
            frame_opacity: 100,
            language: crate::i18n::system_default(),
            file_name: DEFAULT_FILE_NAME.into(),
            file_number: 1,
            click_fx: false,
            click_left: 0,
            click_right: 1,
            click_size: 60,
            click_ms: 500,
            show_fps: true,
            mic_device: String::new(),
            webcam_device: String::new(),
        }
    }
}

fn path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or(".".into());
    PathBuf::from(base).join("ClearShot").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, serde_json::to_string_pretty(self).unwrap());
    }
}

pub fn key_name(vk: u32) -> String {
    match vk {
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x13 => "Pause".into(),
        0x2C => "PrtSc".into(),
        0x91 => "ScrLk".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        _ => format!("Key {vk}"),
    }
}
