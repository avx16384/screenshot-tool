# screenshot-daemon

Linux screenshot daemon with interactive region selection and annotation tools.

## Features

- **Global hotkey** (Ctrl+Alt+A) triggers region selection overlay
- **Region selection** — drag to select any screen area
- **Annotation tools** — Rectangle, Ellipse, Circle, Line, Arrow, Text
- **Move tool** — drag to reposition the selected region
- **Draggable toolbar** — positioned near selection, can be moved
- **Save** — crops captured image with annotations, saves as PNG to `~/Pictures/screenshots/`
- **Undo** — Ctrl+Z to remove last annotation
- **D-Bus notification** on save

## Installation

```bash
cargo install screenshot-daemon
```

This installs two binaries:
- `screenshot-daemon` — the background daemon (watches for hotkey)
- `region-selector` — standalone region selector (can be used directly)

## Usage

### Daemon mode

```bash
screenshot-daemon
```

Press **Ctrl+Alt+A** to trigger region selection.

### Standalone region selector

```bash
region-selector --output ~/Pictures/screenshots/shot.png
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Ctrl+Alt+A | Trigger region selection (daemon mode) |
| M | Move tool |
| R | Rectangle tool |
| E | Ellipse tool |
| C | Circle tool |
| L | Line tool |
| A | Arrow tool |
| T | Text tool |
| Enter | Save screenshot |
| Ctrl+Z | Undo last annotation |
| Escape | Cancel / go back |

## Requirements

- Linux desktop with X11 or Wayland
- `input` group membership (for evdev hotkey detection)

## License

MIT
