# screenshot-tool

Lightweight Linux screenshot and screencast daemon with global hotkeys, region selection, and a draggable floating control bar.

The public release ships project executables plus project C API dynamic libraries. It does not bundle FFmpeg, libvpx, opus, GBM, libc, or other system libraries.

## Features

- Fullscreen screenshot: `Ctrl+Shift+P`
- Region screenshot: `Ctrl+Alt+A`
- Screen recording: `Ctrl+Alt+R`
- VP9 video and Opus audio in WebM
- Runtime-loaded C API overlay and selector libraries
- Fullscreen red recording border overlay
- Draggable recording control bar, controlled by TOML config
- X11 and Wayland support

## Release Layout

| Path | Purpose |
| --- | --- |
| `bin/` | Wrapper entry points that set dynamic library paths |
| `libexec/` | Project executable binaries |
| `lib/` | Project dynamic libraries: `libscreenshot_daemon.so`, `libregion_overlay_capi.so` |
| `install` | User install script |
| `uninstall` | User uninstall script |

Private Rust crate source is not included. The selector and overlay implementation is shipped as `.so` files and loaded at runtime.

## Install

```bash
tar -xzf screenshot-tool-v0.1.2-linux-x86_64.tar.gz
cd screenshot-tool-v0.1.2-linux-x86_64
./install
```

Uninstall:

```bash
./uninstall
```

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

## System Libraries

Install these with the system package manager. They are intentionally not bundled in the release archive.

Debian/Ubuntu 24.04:

```bash
sudo apt install libgbm1 libavcodec60 libavformat60 libavutil58 libswscale7 libvpx7 libopus0
```

Arch Linux:

```bash
sudo pacman -S ffmpeg mesa libvpx opus
```

Fedora:

```bash
sudo dnf install ffmpeg-libs mesa-libgbm libvpx opus
```

Required runtime libraries:

| Library | Purpose |
| --- | --- |
| libavcodec | VP9 and Opus encoding through FFmpeg |
| libavformat | WebM muxing |
| libavutil | FFmpeg utility runtime |
| libswscale | pixel format conversion |
| libvpx | VP9 codec |
| libopus | Opus codec |
| libgbm | graphics buffer management |

## Build Release

The opensource repository keeps private implementation dependencies as local shadow links during development. Do not copy those crate sources into this repository.

Build the binary release package:

```bash
./scripts/release_folder.sh 0.1.2
```

The generated archive contains only project binaries, project `.so` files, docs, service files, and install scripts.

## License

MIT
