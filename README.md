# screenshot-tool

Linux screenshot and screen recording tool for X11 and Wayland.

## Summary

- Fullscreen screenshot: `Ctrl+Shift+P`
- Region screenshot: `Ctrl+Alt+A`
- Start/stop screen recording: `Ctrl+Alt+R`
- Recording output: VP9 + Opus in WebM
- Screenshot output: `~/Pictures/screenshots/`
- Recording output: `~/Videos/screencasts/`

## Install

Install required system libraries first.

Debian/Ubuntu:

```bash
sudo apt install libgbm1 libavcodec60 libavformat60 libavutil58 libswscale7 libvpx7 libopus0
```

Arch:

```bash
sudo pacman -S ffmpeg mesa libvpx opus
```

Fedora:

```bash
sudo dnf install ffmpeg-libs mesa-libgbm libvpx opus
```

Install the release package:

```bash
tar -xzf screenshot-tool-v0.1.2-linux-x86_64.tar.gz
cd screenshot-tool-v0.1.2-linux-x86_64
./install
```

Uninstall:

```bash
./uninstall
```

## Usage

Start the daemon:

```bash
screenshot-daemon
```

Or run the installed user service:

```bash
systemctl --user start screenshot-daemon.service
```

Hotkeys:

| Action | Hotkey |
| --- | --- |
| Fullscreen screenshot | `Ctrl+Shift+P` |
| Region screenshot | `Ctrl+Alt+A` |
| Start/stop recording | `Ctrl+Alt+R` |

## Config

Config file:

```text
~/.config/screenshot-daemon/config.toml
```

Example:

```toml
save_dir = "/home/user/Pictures/screenshots"
controlbar_draggable = true
```

## Overlay Library

`lib/libregion_overlay_capi.so` provides the recording border and control bar. It is replaceable: any implementation with the same C API can be used.

The current `.so` is shipped as a binary because it uses private crates that are not part of this open-source repository. Compatible open-source overlay implementations are welcome.

## Build

```bash
cargo build --release
```

Create release archive:

```bash
./scripts/release_folder.sh 0.1.2
```
