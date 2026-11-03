# ClearShot

A free, open-source screen, game and sound recorder for Windows. No ads, no watermark unless you add one, single executable.

[![Release](https://img.shields.io/github/v/release/rafmco/clearshot-rust)](https://github.com/rafmco/clearshot-rust/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

> 🇧🇷 [Leia em português](#português)

## Download

Get the latest `clearshot-windows-x64.zip` from the [Releases page](https://github.com/rafmco/clearshot-rust/releases/latest), unzip it and run `clearshot.exe`. No installation needed.

**Requirements:** Windows 10 or 11 (64-bit). [FFmpeg](https://ffmpeg.org) is optional and only needed for MKV, FLV, VOB and AVI output.

## Features

- **Three modes:** Screen Recording (area inside a resizable frame), Game Recording (the window in front) and Sound Recording (audio only)
- **Video formats:** MP4, MOV, M4V, TS (H.264 + AAC), WMV and animated GIF built in; MKV, FLV, VOB and AVI through FFmpeg
- **Hardware encoding:** GPU H.264 via NVIDIA NVENC, Intel Quick Sync or AMD AMF
- **Audio:** PC sound, microphone, both or none; MP3, M4A or WAV for sound-only recordings
- **Screenshots:** PNG, JPG, GIF or BMP
- **Pause/resume** with the paused time cut from the file
- **Webcam** picture-in-picture and **text/image watermark** in any corner
- **Smart area:** click a window, pick a monitor or choose a size preset
- **Global hotkeys** that work while a game is in front, re-bindable in Options
- **Auto-save:** closing during a recording finishes and keeps the file
- **Languages:** English, Português (Brasil), Español

## Default hotkeys

| Action | Key |
|---|---|
| Record / Stop | F2 |
| Pause / Resume | F3 |
| Screenshot | F11 |
| Move frame | Arrow keys (Ctrl = 10 px) |
| Resize frame | Shift + Arrow keys |

Recordings are saved to `Documents\ClearShot`. Full behaviour details are in [MECHANICS.md](MECHANICS.md).

## Building from source

1. Install [Rust](https://rustup.rs) (it will ask for the *Visual Studio Build Tools* with "Desktop development with C++").
2. Clone and build:

```
git clone https://github.com/rafmco/clearshot-rust.git
cd clearshot-rust
cargo clean
cargo build --release
```

The executable is created at `target\release\clearshot.exe`. Use `cargo run` during development.

## Contributing

Issues and pull requests are welcome. For larger changes, please open an issue first to discuss the idea.

## Clean-room notice

ClearShot is an independent implementation written from a description of the desired behaviour. No code, binaries, assets or resources from any other program were used. It is not affiliated with or endorsed by any other software vendor.

## License

[MIT](LICENSE)

---

## Português

Gravador de tela, jogos e áudio para Windows, gratuito e de código aberto. Sem anúncios e em um único executável.

**Download:** baixe o `clearshot-windows-x64.zip` na [página de Releases](https://github.com/rafmco/clearshot-rust/releases/latest), descompacte e execute o `clearshot.exe`. Não precisa instalar.

**Requisitos:** Windows 10 ou 11 (64 bits). O [FFmpeg](https://ffmpeg.org) é opcional e só é necessário para MKV, FLV, VOB e AVI.

**Atalhos padrão:** F2 grava/para, F3 pausa/continua, F11 captura a tela. Todos podem ser alterados em Opções.

As gravações ficam em `Documentos\ClearShot`. Para compilar, veja [Building from source](#building-from-source). A licença é [MIT](LICENSE).
