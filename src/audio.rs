//! WASAPI capture of PC sound (loopback) and the microphone, mixed to 48 kHz stereo 16-bit.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

pub const RATE: u32 = 48000;

type Buf = Arc<Mutex<VecDeque<f32>>>;

/// Starts one capture thread per source. Returns the buffers (interleaved stereo f32).
fn start_source(loopback: bool, mic_id: String, stop: Arc<AtomicBool>) -> Buf {
    let buf: Buf = Arc::new(Mutex::new(VecDeque::new()));
    let b = buf.clone();
    std::thread::spawn(move || unsafe {
        if let Err(e) = capture_loop(loopback, &mic_id, &b, &stop) {
            eprintln!("audio source ({}) failed: {e}", if loopback { "system" } else { "mic" });
        }
    });
    buf
}

/// Active microphones as (endpoint id, name), for Options.
pub fn mics() -> Vec<(String, String)> {
    unsafe {
        let r: windows::core::Result<Vec<(String, String)>> = (|| {
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let list = en.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
            let mut v = Vec::new();
            for i in 0..list.GetCount()? {
                let dev = list.Item(i)?;
                let p = dev.GetId()?;
                let id = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as _));
                let name = dev.OpenPropertyStore(STGM_READ)?.GetValue(&windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName)?.to_string();
                v.push((id, name));
            }
            Ok(v)
        })();
        r.unwrap_or_default()
    }
}

/// PC sound, or the chosen microphone; the Windows default when `mic_id` is empty or unplugged.
unsafe fn device(en: &IMMDeviceEnumerator, loopback: bool, mic_id: &str) -> windows::core::Result<IMMDevice> {
    if !loopback && !mic_id.is_empty() {
        if let Ok(d) = en.GetDevice(&windows::core::HSTRING::from(mic_id)) {
            if d.GetState().map_or(false, |s| s == DEVICE_STATE_ACTIVE) {
                return Ok(d);
            }
        }
    }
    en.GetDefaultAudioEndpoint(if loopback { eRender } else { eCapture }, eConsole)
}

unsafe fn capture_loop(loopback: bool, mic_id: &str, buf: &Buf, stop: &AtomicBool) -> windows::core::Result<()> {
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let dev = device(&en, loopback, mic_id)?;
    let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
    let fmt = WAVEFORMATEX {
        wFormatTag: 3, // IEEE float
        nChannels: 2,
        nSamplesPerSec: RATE,
        wBitsPerSample: 32,
        nBlockAlign: 8,
        nAvgBytesPerSec: RATE * 8,
        cbSize: 0,
    };
    let mut flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    if loopback {
        flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
    }
    client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &fmt, None)?;
    let cap: IAudioCaptureClient = client.GetService()?;
    // loopback sound arrives already turned down by the speaker volume; undo that so recordings
    // are as loud as the source (measured: a 0.5 tone recorded at 0.067 before this)
    let vol: Option<Endpoints::IAudioEndpointVolume> = if loopback { dev.Activate(CLSCTX_ALL, None).ok() } else { None };
    client.Start()?;
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        loop {
            let n = cap.GetNextPacketSize()?;
            if n == 0 {
                break;
            }
            let mut data = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut fl = 0u32;
            cap.GetBuffer(&mut data, &mut frames, &mut fl, None, None)?;
            let mut q = buf.lock().unwrap();
            if fl & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                q.extend(std::iter::repeat(0.0).take(frames as usize * 2));
            } else {
                let db = vol.as_ref().and_then(|v| v.GetMasterVolumeLevel().ok()).unwrap_or(0.0);
                let gain = 10f32.powf(-db / 20.0).clamp(1.0, 100.0);
                q.extend(std::slice::from_raw_parts(data as *const f32, frames as usize * 2).iter().map(|v| v * gain));
            }
            // never let a source run more than 1 s ahead
            let max = RATE as usize * 2;
            if q.len() > max {
                let d = q.len() - max;
                q.drain(..d);
            }
            drop(q);
            cap.ReleaseBuffer(frames)?;
        }
    }
    let _ = client.Stop();
    Ok(())
}

/// Mixes the active sources in real time. `sink` gets 16-bit stereo chunks paced by the wall clock,
/// padded with silence when a source has nothing (loopback is silent when nothing plays), so the
/// sound stays in sync with the video. Nothing is written while `paused` is set.
pub fn start_mixer(system: bool, mic: bool, mic_id: &str, stop: Arc<AtomicBool>, paused: Arc<AtomicBool>, mut sink: impl FnMut(Vec<i16>) + Send + 'static) -> Option<std::thread::JoinHandle<()>> {
    if !system && !mic {
        return None;
    }
    let mut srcs = Vec::new();
    if system {
        srcs.push(start_source(true, String::new(), stop.clone()));
    }
    if mic {
        srcs.push(start_source(false, mic_id.to_string(), stop.clone()));
    }
    Some(std::thread::spawn(move || {
        let mut active = std::time::Duration::ZERO;
        let mut last = Instant::now();
        let mut written: u64 = 0; // frames
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let now = Instant::now();
            let p = paused.load(Ordering::Relaxed);
            if !p {
                active += now - last;
            }
            last = now;
            let want = (active.as_secs_f64() * RATE as f64) as u64;
            let frames = want.saturating_sub(written) as usize;
            let mut mix = vec![0f32; frames * 2];
            for s in &srcs {
                let mut q = s.lock().unwrap();
                if p {
                    q.clear(); // drop what's heard while paused
                    continue;
                }
                let take = q.len().min(mix.len());
                for (m, v) in mix.iter_mut().zip(q.drain(..take)) {
                    *m += v;
                }
            }
            if frames == 0 {
                continue;
            }
            written += frames as u64;
            // soft limiter above 0.8 instead of hard clipping (sound effects can push PC sound past full scale)
            let soft = |v: f32| if v.abs() <= 0.8 { v } else { v.signum() * (0.8 + 0.2 * ((v.abs() - 0.8) / 0.2).tanh()) };
            sink(mix.iter().map(|v| (soft(*v) * 32767.0) as i16).collect());
        }
    }))
}

/// Peak level of PC sound or the chosen microphone, for the level meter (0..1).
pub fn peak(loopback: bool, mic_id: &str) -> f32 {
    unsafe {
        let r: windows::core::Result<f32> = (|| {
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let dev = device(&en, loopback, mic_id)?;
            let m: Endpoints::IAudioMeterInformation = dev.Activate(CLSCTX_ALL, None)?;
            m.GetPeakValue()
        })();
        r.unwrap_or(0.0)
    }
}
