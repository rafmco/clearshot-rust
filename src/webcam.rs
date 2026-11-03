//! Webcam (facecam) frames via the Media Foundation source reader; the latest frame is shared.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Media::MediaFoundation::*;

/// Size of the last webcam picture, for the preview box on the recording frame.
static SIZE: AtomicU64 = AtomicU64::new(0);

/// shortcut: 4:3 until the camera has run once (most start at 640x480); open it at startup if that box is off.
pub fn size() -> (u32, u32) {
    match SIZE.load(Ordering::Relaxed) {
        0 => (4, 3),
        v => ((v >> 32) as u32, v as u32),
    }
}

pub type Frame = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>; // BGRA

/// Video capture devices Windows knows about.
unsafe fn devices() -> windows::core::Result<Vec<IMFActivate>> {
    let mut attrs: Option<IMFAttributes> = None;
    MFCreateAttributes(&mut attrs, 1)?;
    let attrs = attrs.unwrap();
    attrs.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
    let mut devs: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut n = 0u32;
    MFEnumDeviceSources(&attrs, &mut devs, &mut n)?;
    if devs.is_null() {
        return Ok(Vec::new());
    }
    let v = std::slice::from_raw_parts_mut(devs, n as usize).iter_mut().filter_map(|d| d.take()).collect();
    windows::Win32::System::Com::CoTaskMemFree(Some(devs as _));
    Ok(v)
}

unsafe fn name(a: &IMFActivate) -> String {
    let mut p = windows::core::PWSTR::null();
    let mut len = 0u32;
    if a.GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut p, &mut len).is_err() {
        return String::new();
    }
    let s = p.to_string().unwrap_or_default();
    windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
    s
}

/// Webcam names, for Options.
pub fn cameras() -> Vec<String> {
    unsafe { devices().map(|v| v.iter().map(|a| name(a)).collect()).unwrap_or_default() }
}

/// Starts the webcam called `want` (the first one when it is empty or not plugged in).
pub fn start(stop: Arc<AtomicBool>, want: String) -> Frame {
    let frame: Frame = Arc::new(Mutex::new(None));
    let f = frame.clone();
    std::thread::spawn(move || unsafe {
        if let Err(e) = run(&f, &stop, &want) {
            eprintln!("webcam: {e}");
        }
    });
    frame
}

unsafe fn run(out: &Frame, stop: &AtomicBool, want: &str) -> windows::core::Result<()> {
    crate::encoder::mf_startup();
    let list = devices()?;
    let Some(dev) = list.iter().find(|a| !want.is_empty() && name(a) == want).or(list.first()) else {
        return Err(windows::core::Error::new(windows::core::HRESULT(-1), "no webcam found"));
    };
    let src: IMFMediaSource = dev.ActivateObject()?;
    let mut ra: Option<IMFAttributes> = None;
    MFCreateAttributes(&mut ra, 1)?;
    let ra = ra.unwrap();
    ra.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
    let reader = MFCreateSourceReaderFromMediaSource(&src, &ra)?;
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    let t = MFCreateMediaType()?;
    t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
    reader.SetCurrentMediaType(stream, None, &t)?;
    let cur = reader.GetCurrentMediaType(stream)?;
    let size = cur.GetUINT64(&MF_MT_FRAME_SIZE)?;
    let (w, h) = ((size >> 32) as u32, size as u32);
    SIZE.store(size, Ordering::Relaxed);
    // negative stride means bottom-up rows
    let bottom_up = cur.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| (s as i32) < 0).unwrap_or(false);
    while !stop.load(Ordering::Relaxed) {
        let mut flags = 0u32;
        let mut sample: Option<IMFSample> = None;
        reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
        let Some(s) = sample else { continue };
        let buf = s.ConvertToContiguousBuffer()?;
        let mut p = std::ptr::null_mut();
        let mut len = 0u32;
        buf.Lock(&mut p, None, Some(&mut len))?;
        let need = (w * h * 4) as usize;
        if len as usize >= need {
            let src = std::slice::from_raw_parts(p, need);
            let row = (w * 4) as usize;
            let mut v = src.to_vec();
            if bottom_up {
                for y in 0..h as usize {
                    v[y * row..(y + 1) * row].copy_from_slice(&src[(h as usize - 1 - y) * row..(h as usize - y) * row]);
                }
            }
            *out.lock().unwrap() = Some((w, h, v));
        }
        buf.Unlock()?;
    }
    let _ = src.Shutdown();
    Ok(())
}
