# screenshot-tool

Lightweight Linux screenshot & screencast daemon with global hotkeys, region selection, and a draggable floating control bar. Zero external runtime dependencies — all capture, encoding, clipboard, and notification logic uses direct API calls.

## Features

- **Fullscreen screenshot** — `Ctrl+Shift+P`
- **Region screenshot** — `Ctrl+Alt+A` (drag to select, with annotation)
- **Screen recording** — `Ctrl+Alt+R` (VP9/WebM, royalty-free)
- **Draggable control bar** — pause, resume, stop recording with a floating bar
- **Auto clipboard** — screenshots copied to clipboard automatically
- **Desktop notifications** — D-Bus notifications with "Open" action
- **X11 & Wayland** — auto-detects display server, works on both
- **Dependency check** — warns at startup if required libraries are missing

## Hotkeys

| Action | Hotkey |
| --- | --- |
| Fullscreen screenshot | `Ctrl+Shift+P` |
| Region screenshot | `Ctrl+Alt+A` |
| Start / Stop recording | `Ctrl+Alt+R` |

## Video Recording

Recordings use **VP9** (libvpx) in a **WebM** container — no proprietary codecs (no H.264, no AAC). Files are saved to `~/Videos/screencasts/`.

Encoder settings: CRF 30, realtime deadline, cpu-used 8 (fast encoding).

## Binaries

| Binary | Purpose |
| --- | --- |
| `screenshot-daemon` | Background daemon (main entry point) |
| `region-selector` | Standalone region selection & annotation tool |
| `record-control` | Floating control bar for recording |
| `deps-dialog` | Dependency warning dialog |

## Build

```bash
cargo build --workspace --release
```

Binaries are output to `target/release/`.

## System Requirements

- Linux desktop (X11 or Wayland)
- Read access to `/dev/input/` for global hotkey detection

### System Library Dependencies

These are system packages (install via your distro's package manager, e.g. `pacman -S libavcodec libavformat libswscale libvpx`).

| Library | Package (Arch) | Purpose |
| --- | --- | --- |
| libavcodec | `ffmpeg` | Video encoding |
| libavformat | `ffmpeg` | Container muxing |
| libswscale | `ffmpeg` | Pixel format conversion |
| libvpx | `libvpx` | VP9 encoder |

All other dependencies are Rust crates compiled statically into the binary — no additional system packages needed.

## Install as systemd Service

```bash
mkdir -p ~/.config/systemd/user
cp screenshot-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now screenshot-daemon
```

## Release

```bash
./scripts/release_folder.sh
./scripts/pack_to_7z.sh release/screenshot-tool-v0.1.0-linux-x86_64
```

## License

MIT
