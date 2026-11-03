//! File writers: Media Foundation (MP4/MOV/M4V/TS/AVI/WMV, MP3/M4A), GIF and WAV.
//! MKV/FLV/VOB/AVI are recorded as MP4 then converted with FFmpeg when it is installed.
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use windows::core::{Interface, GUID, HSTRING};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;

pub enum Msg {
    /// BGRA top-down frame and its time in 100 ns units
    Video(Vec<u8>, i64),
    /// 48 kHz stereo 16-bit
    Audio(Vec<i16>),
    Finish,
}

#[derive(Clone)]
pub struct Job {
    pub path: PathBuf,
    pub w: u32,
    /// width of the frames sent (w rounded up to 16 px); the file is cropped back to `w`
    pub enc_w: u32,
    pub h: u32,
    pub fps: u32,
    pub mbps: u32,
    pub audio: bool,
    pub video: bool,
    pub gpu: bool,
    pub gif: bool,
}

pub fn spawn(job: Job) -> (SyncSender<Msg>, std::thread::JoinHandle<Result<PathBuf, String>>) {
    let (tx, rx) = sync_channel::<Msg>(16);
    let h = std::thread::spawn(move || {
        let ext = job.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if job.gif {
            return write_gif(&job, rx);
        }
        if ext == "wav" {
            return write_wav(&job.path, rx);
        }
        let needs_ffmpeg = matches!(ext.as_str(), "mkv" | "flv" | "vob" | "avi");
        if needs_ffmpeg {
            let tmp = job.path.with_extension("tmp.mp4");
            let mut j = job.clone();
            j.path = tmp.clone();
            unsafe { write_mf(&j, &ext_container("mp4"), rx) }?;
            return convert_with_ffmpeg(&tmp, &job.path, &ext);
        }
        unsafe { write_mf(&job, &ext_container(&ext), rx) }.map(|_| job.path.clone())
    });
    (tx, h)
}

fn ext_container(ext: &str) -> GUID {
    match ext {
        "wmv" => MFTranscodeContainerType_ASF,
        "ts" => MFTranscodeContainerType_MPEG2,
        "mp3" => MFTranscodeContainerType_MP3,
        _ => MFTranscodeContainerType_MPEG4, // mp4, mov, m4v, m4a
    }
}

pub fn ffmpeg_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut dirs = vec![exe_dir];
    if let Ok(p) = std::env::var("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    dirs.into_iter().map(|d| d.join("ffmpeg.exe")).find(|p| p.is_file())
}

fn convert_with_ffmpeg(tmp: &Path, out: &Path, ext: &str) -> Result<PathBuf, String> {
    let Some(ff) = ffmpeg_path() else {
        let keep = out.with_extension("mp4");
        let _ = std::fs::rename(tmp, &keep);
        return Ok(keep); // the Codecs menu already says these need FFmpeg; the MP4 is kept
    };
    let mut cmd = std::process::Command::new(ff);
    cmd.arg("-y").arg("-loglevel").arg("error").arg("-i").arg(tmp);
    if ext == "vob" {
        cmd.args(["-c:v", "mpeg2video", "-q:v", "2", "-c:a", "mp2", "-b:a", "192k", "-f", "vob"]);
    } else if ext == "avi" {
        cmd.args(["-c:v", "copy", "-c:a", "libmp3lame", "-b:a", "192k"]);
    } else {
        cmd.args(["-c", "copy"]);
    }
    use std::os::windows::process::CommandExt;
    let ok = cmd.arg(out).creation_flags(0x0800_0000).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        let _ = std::fs::remove_file(tmp);
        Ok(out.to_path_buf())
    } else {
        let keep = out.with_extension("mp4");
        let _ = std::fs::rename(tmp, &keep);
        Err(format!("FFmpeg conversion failed; kept MP4: {}", keep.display()))
    }
}

unsafe fn mt() -> windows::core::Result<IMFMediaType> {
    MFCreateMediaType()
}

unsafe fn audio_pcm() -> windows::core::Result<IMFMediaType> {
    let t = mt()?;
    t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
    t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
    t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
    t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
    t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
    t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192000)?;
    Ok(t)
}

unsafe fn audio_out(ext: &str) -> windows::core::Result<IMFMediaType> {
    if ext == "wmv" || ext == "mp3" {
        // WMA and MP3 need one of the encoder's own types
        let sub = if ext == "mp3" { MFAudioFormat_MP3 } else { MFAudioFormat_WMAudioV8 };
        let col = MFTranscodeGetAudioOutputAvailableTypes(&sub, MFT_ENUM_FLAG_ALL.0 as u32, None)?;
        let mut best: Option<IMFMediaType> = None;
        for i in 0..col.GetElementCount()? {
            let t: IMFMediaType = col.GetElement(i)?.cast()?;
            let rate = t.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).unwrap_or(0);
            let ch = t.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap_or(0);
            let bps = t.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND).unwrap_or(0);
            if ch == 2 && (rate == 48000 || rate == 44100) && bps <= 24000 {
                let better = match &best {
                    None => true,
                    Some(b) => bps > b.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND).unwrap_or(0),
                };
                if better {
                    best = Some(t);
                }
            }
        }
        return best.ok_or_else(|| windows::core::Error::new(windows::core::HRESULT(-1), "no matching audio encoder type"));
    }
    let t = mt()?;
    t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
    t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
    t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
    t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
    t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24000)?; // 192 kbit/s
    Ok(t)
}

unsafe fn video_type(sub: &GUID, w: u32, h: u32, fps: u32) -> windows::core::Result<IMFMediaType> {
    let t = mt()?;
    t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    t.SetGUID(&MF_MT_SUBTYPE, sub)?;
    t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
    t.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
    t.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
    t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
    Ok(t)
}

pub fn mf_startup() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(0x0002_0070, MFSTARTUP_FULL);
    }
}

unsafe fn write_mf(job: &Job, container: &GUID, rx: Receiver<Msg>) -> Result<(), String> {
    mf_startup();
    let ext = job.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let r: windows::core::Result<()> = (|| {
        let _ = std::fs::remove_file(&job.path);
        let mut attrs: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attrs, 3)?;
        let attrs = attrs.unwrap();
        attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, container)?;
        attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, job.gpu as u32)?;
        attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(job.path.as_os_str()), None, &attrs)?;
        let mut vstream = None;
        if job.video {
            let sub = if ext == "wmv" { MFVideoFormat_WMV3 } else { MFVideoFormat_H264 };
            let out = video_type(&sub, job.enc_w, job.h, job.fps)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, job.mbps * 1_000_000)?;
            if sub == MFVideoFormat_H264 {
                out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            }
            if job.enc_w != job.w {
                // MFVideoArea: OffsetX/OffsetY (MFOffset, 4 bytes each) = 0, then Area (SIZE cx, cy)
                let mut area = [0u8; 16];
                area[8..12].copy_from_slice(&(job.w as i32).to_le_bytes());
                area[12..16].copy_from_slice(&(job.h as i32).to_le_bytes());
                out.SetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &area)?;
            }
            // an encoder that refuses the crop still records, just with the padded width
            let idx = match writer.AddStream(&out) {
                Ok(i) => i,
                Err(_) => {
                    out.DeleteItem(&MF_MT_MINIMUM_DISPLAY_APERTURE)?;
                    writer.AddStream(&out)?
                }
            };
            let inp = video_type(&MFVideoFormat_RGB32, job.enc_w, job.h, job.fps)?;
            inp.SetUINT32(&MF_MT_DEFAULT_STRIDE, job.enc_w * 4)?; // positive stride = top-down
            writer.SetInputMediaType(idx, &inp, None)?;
            vstream = Some(idx);
        }
        let mut astream = None;
        if job.audio {
            let idx = writer.AddStream(&audio_out(&ext)?)?;
            writer.SetInputMediaType(idx, &audio_pcm()?, None)?;
            astream = Some(idx);
        }
        writer.BeginWriting()?;
        let frame_dur = 10_000_000i64 / job.fps.max(1) as i64;
        let mut audio_frames: i64 = 0;
        let mut last_video = -1i64;
        for m in rx {
            match m {
                Msg::Video(px, t) => {
                    let Some(idx) = vstream else { continue };
                    if t <= last_video {
                        continue;
                    }
                    last_video = t;
                    writer.WriteSample(idx, &sample(&px, t, frame_dur)?)?;
                }
                Msg::Audio(pcm) => {
                    let Some(idx) = astream else { continue };
                    let bytes = std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 2);
                    let n = pcm.len() as i64 / 2;
                    let t = audio_frames * 10_000_000 / 48000;
                    let d = n * 10_000_000 / 48000;
                    audio_frames += n;
                    writer.WriteSample(idx, &sample(bytes, t, d)?)?;
                }
                Msg::Finish => break,
            }
        }
        writer.Finalize()?;
        Ok(())
    })();
    r.map_err(|e| format!("Could not write {}: {}", job.path.display(), e.message()))
}

unsafe fn sample(bytes: &[u8], t: i64, d: i64) -> windows::core::Result<IMFSample> {
    let buf = MFCreateMemoryBuffer(bytes.len() as u32)?;
    let mut p = std::ptr::null_mut();
    buf.Lock(&mut p, None, None)?;
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
    buf.Unlock()?;
    buf.SetCurrentLength(bytes.len() as u32)?;
    let s = MFCreateSample()?;
    s.AddBuffer(&buf)?;
    s.SetSampleTime(t)?;
    s.SetSampleDuration(d)?;
    Ok(s)
}

fn write_wav(path: &Path, rx: Receiver<Msg>) -> Result<PathBuf, String> {
    let r: std::io::Result<()> = (|| {
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
        let hdr = |len: u32| -> Vec<u8> {
            let mut h = Vec::with_capacity(44);
            h.extend(b"RIFF");
            h.extend((36 + len).to_le_bytes());
            h.extend(b"WAVEfmt ");
            h.extend(16u32.to_le_bytes());
            h.extend(1u16.to_le_bytes());
            h.extend(2u16.to_le_bytes());
            h.extend(48000u32.to_le_bytes());
            h.extend(192000u32.to_le_bytes());
            h.extend(4u16.to_le_bytes());
            h.extend(16u16.to_le_bytes());
            h.extend(b"data");
            h.extend(len.to_le_bytes());
            h
        };
        f.write_all(&hdr(0))?;
        for m in rx {
            match m {
                Msg::Audio(pcm) => {
                    for s in pcm {
                        f.write_all(&s.to_le_bytes())?;
                    }
                }
                Msg::Finish => break,
                _ => {}
            }
        }
        let mut f = f.into_inner().map_err(|e| e.into_error())?;
        let len = (f.stream_position()? - 44) as u32;
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&hdr(len))?;
        Ok(())
    })();
    r.map(|_| path.to_path_buf()).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

fn write_gif(job: &Job, rx: Receiver<Msg>) -> Result<PathBuf, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};
    let r: image::ImageResult<()> = (|| {
        let f = std::io::BufWriter::new(std::fs::File::create(&job.path)?);
        let mut enc = GifEncoder::new_with_speed(f, 20);
        enc.set_repeat(Repeat::Infinite)?;
        let mut pending: Option<(Vec<u8>, i64)> = None;
        let flush = |enc: &mut GifEncoder<_>, px: Vec<u8>, ms: i64| -> image::ImageResult<()> {
            let img = RgbaImage::from_raw(job.w, job.h, crate::capture::bgra_to_rgba(px)).unwrap();
            enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(ms.max(20) as u32, 1)))
        };
        for m in rx {
            match m {
                Msg::Video(px, t) => {
                    if let Some((p, t0)) = pending.take() {
                        flush(&mut enc, p, (t - t0) / 10_000)?;
                    }
                    pending = Some((px, t));
                }
                Msg::Finish => break,
                _ => {}
            }
        }
        if let Some((p, _)) = pending {
            flush(&mut enc, p, 1000 / job.fps.max(1) as i64)?;
        }
        Ok(())
    })();
    r.map(|_| job.path.clone()).map_err(|e| format!("Could not write GIF: {e}"))
}
