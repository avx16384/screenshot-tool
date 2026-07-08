# screenshot-tool

Linux screenshot and screen recording tool for X11 and Wayland.

A plain desktop application — **no systemd required**. Start it and a camera
icon appears in the system tray; click the tray icon for the context menu, or
use global hotkeys / compositor bindings to trigger captures.

## Features

- **System tray** (StatusNotifierItem over D-Bus) with a context menu:
  Fullscreen Screenshot · Region Screenshot · Start / Stop Recording · Quit
- **Three trigger paths**: tray menu, global hotkeys (evdev), and a session
  D-Bus service for compositor-level bindings (e.g. Sway `bindsym`)
- Fullscreen + region screenshot, screen recording (VP9 + Opus in WebM)
- Works on X11 and Wayland (Sway, etc.)
- Region selection runs as an isolated subprocess — no `EventLoop` recreation
  limit across repeated captures
- Recording border + control bar via a replaceable C API overlay library
- Dependency check dialog on startup if required libraries are missing

## Summary

| Action | Hotkey (evdev) | Output |
| --- | --- | --- |
| Fullscreen screenshot | `PrintScreen` | `~/Pictures/screenshots/` |
| Region screenshot | `Ctrl+Alt+A` | `~/Pictures/screenshots/` |
| Start / stop recording | `Ctrl+Alt+R` | `~/Videos/screencasts/` |

Filenames follow `<prefix>_<YYYYMMDD>_<HHMMSS>_<random6>.<ext>` with a
real-time timestamp generated on each capture.

## Install

### 1. System libraries

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

### 2. Install the package

**tar.gz** (portable, user-level install with autostart entry):

```bash
tar -xzf screenshot-tool-v0.1.7-linux-x86_64.tar.gz
cd screenshot-tool-v0.1.7-linux-x86_64
./install
```

This installs binaries to `~/.local/opt/screenshot-tool/`, symlinks into
`~/.local/bin/`, the tray SVG icon into the hicolor icon theme, and a
freedesktop autostart `.desktop` entry into `~/.config/autostart/` (so the
daemon starts on next login).

**deb** (system-level, to `/opt`):

```bash
sudo dpkg -i screenshot-tool_0.1.7_amd64.deb
```

**rpm** (system-level, to `/opt`):

```bash
sudo rpm -i screenshot-tool-0.1.7-1.x86_64.rpm
```

### Uninstall

For the tar.gz install:

```bash
./uninstall
```

For deb / rpm: `sudo dpkg -r screenshot-tool` / `sudo rpm -e screenshot-tool`.

## Usage

### Start / Stop

The release package ships `start.sh` and `close.sh`:

```bash
./start.sh      # launch the daemon in the background (detached, logs to ~/.local/share/screenshot-tool/daemon.log)
./close.sh      # stop the daemon cleanly via its D-Bus Quit method (no kill/pkill)
```

Or run the binary directly (foreground, useful for debugging):

```bash
screenshot-daemon
```

After `./install`, `screenshot-daemon` is on `PATH` and the autostart entry
launches it on login — no manual start needed.

### System tray

A camera icon appears in the system tray. Click it for the context menu:

| Menu item | Action |
| --- | --- |
| Fullscreen Screenshot | Capture the full screen |
| Region Screenshot | Select a region and capture it |
| Start / Stop Recording | Toggle screen recording |
| Quit | Stop the daemon |

> **swaybar note**: the tray sets `ItemIsMenu=true` so the host renders the
> menu on click. If your tray host does not show the menu, ensure it respects
> the `ItemIsMenu` SNI property.

### Hotkeys (X11 / non-Sway Wayland)

The daemon listens to raw evdev keyboard events for the hotkeys listed in the
[Summary](#summary) table. This works on X11 and Wayland compositors that do
not grab evdev exclusively.

### Compositor hotkeys (Sway)

Sway grabs keyboard input exclusively (`EVIOCGRAB`), which starves the evdev
listener. Bind compositor keys to the daemon's D-Bus service via the
`screenshot-daemon trigger <action>` subcommand instead:

```swayconfig
# ~/.config/sway/config
bindsym Print          exec screenshot-daemon trigger fullscreen
bindsym Ctrl+Mod1+a    exec screenshot-daemon trigger region
bindsym Ctrl+Mod1+r    exec screenshot-daemon trigger record
```

`trigger` is a thin D-Bus client that calls `org.screenshot_daemon.Service1`
(`/org/screenshot_daemon/Service`) — it exits immediately after dispatching.

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

`lib/libregion_overlay_capi.so` provides the recording border and control bar.
It is loaded at runtime via `dlopen` and is replaceable: any implementation
with the same C API can be used.

The current `.so` is shipped as a binary because it uses private crates that
are not part of this open-source repository. Compatible open-source overlay
implementations are welcome.

## Build

```bash
cargo build --release
```

Create release artifacts (the release profile uses `opt-level = "z"`, LTO, and
`debug = 0`):

```bash
./scripts/release_folder.sh 0.1.7   # builds + creates the tar.gz
./scripts/package_deb.sh 0.1.7      # builds the .deb
./scripts/package_rpm.sh 0.1.7      # builds the .rpm
```

Each packaging script invokes `release_folder.sh` automatically if the release
folder does not already exist.
