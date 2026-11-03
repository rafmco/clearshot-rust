# ClearShot – how it behaves (numbers sheet)

Clean room: nothing from the original program was opened. Values come from the person's request
("request") or are my estimates ("estimated"). The app reads them from the code exactly as written here.

## Modes (request)
| Mode | What it records | How it starts |
|---|---|---|
| Screen Recording | the area inside the green frame | Record button or Record hotkey |
| Game Recording | the window in front (the game) | Record arms it, then the hotkey starts/stops inside the game |
| Sound Recording | audio only | Record button or hotkey |

## Controls
| Action | Default | Source |
|---|---|---|
| Record / Stop hotkey | F2 | estimated |
| Pause / Resume hotkey | F3 | estimated |
| Capture hotkey | F11 | estimated |
| Hotkeys | global (work while a game is in front), re-bindable in Options | request |
| Move frame | Arrow = 1 px | request (step estimated) |
| Move frame fast | Ctrl+Arrow = 10 px | estimated |
| Resize frame | Shift+Arrow = 2 px (x10 with Ctrl) | person's feedback |
| Drag frame | edges/corners resize, centre cross moves | request |
| Smart area | "Select a window (click it)", full screen per monitor, size presets | request |

## Recording frame
| Value | Number | Source |
|---|---|---|
| Default area | 1280 x 720 at (200, 150) | estimated |
| Border thickness | 4 px (outside the recorded area, never recorded) | estimated |
| Grab handle size | 12 px | estimated |
| Minimum size | 64 x 64 | estimated |
| Width step | multiple of 2 px; video is encoded at the next multiple of 16 and cropped back in the file | measured: Windows' H.264 path skewed 646/648-px-wide video, 656 was correct |
| Height step | multiple of 2 px | measured: 490 px tall decoded correctly |
| Colour | green idle, red while recording | estimated |
| Opacity | 100 / 75 / 50 / 25 % (Options) | person's feedback |
| Capture feedback | app window flashes white for 0.4 s, frame inside flashes white for 0.15 s | estimated |

## Encoding
| Value | Number | Source |
|---|---|---|
| Frame rate | 30 fps (choices 15/20/24/30/50/60) | estimated |
| Video bitrate | 8 Mbit/s (choices 2/4/8/12/20) | estimated |
| GIF frame rate | 10 fps (choices 5/10/15/20) | estimated |
| Audio | 48 kHz stereo, AAC/MP3 192 kbit/s | estimated |
| Video formats | MP4, MKV, MOV, WMV, FLV, AVI, TS, VOB, M4V + animated GIF (GIF is in Options > File format and the Codecs menu) | request |
| Built in | MP4, MOV, M4V, TS (H.264+AAC), WMV (WMV9+WMA), GIF | tested by self-test |
| Via FFmpeg | MKV, FLV, VOB, AVI (saved as MP4 when FFmpeg isn't installed) | decision, see notes |
| Encoders | Built-in H.264, GPU H.264 (NVENC/QuickSync/AMF through Windows) | request |
| Sound formats | MP3, M4A, WAV | estimated (request says audio-only) |
| Image formats | PNG, JPG, GIF, BMP | request |
| Sound sources | PC sound, microphone, both, or none | request |
| PC sound level | speaker volume undone (gain = −master dB, max 40 dB), soft limiter above 0.8 | measured: 0.5 tone recorded at 0.06 with speakers at 20% (−24 dB) |

## Behaviour
| Value | Setting | Source |
|---|---|---|
| Ads | none, anywhere | request |
| Closing while recording | file is finished and kept (auto-save), nothing opens | request |
| Pause | paused time is cut out of the file | request |
| Output folder | Documents\ClearShot, "Open" shows it in Explorer | request (folder name estimated) |
| File names | pattern in Options, default "{type} {YYYY}-{MM}-{DD} {hh}.{mm}.{ss}"; tokens {type} {YYYY} {MM} {DD} {hh} {mm} {ss} {ms} {n} | person's feedback |
| Click effects | expanding ring, left/right colour, 40–100 px, 0.3–1.0 s | person's feedback |
| Actual FPS | measured each second, on the status line and the frame label while recording (Options) | person's feedback |
| Custom size | Size > Custom size... (min 64 x 64) | person's feedback |
| Watermark | text and/or image, chosen corner | request |
| Webcam | picture-in-picture, corner + width 320 px; its box is outlined on the recording frame | request (size estimated) |

## Languages
| Value | Setting | Source |
|---|---|---|
| Languages | English, Português (Brasil), Español (Menu or Options) | person's feedback |
| Default | follows the Windows display language | estimated |

## Left out
- VFW (old Video for Windows) codecs: decided to skip, rare today.
