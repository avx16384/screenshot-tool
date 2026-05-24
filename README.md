# screenshot-tool

This is the open-source workspace for the Linux screenshot daemon and
standalone region selector.

The source is synchronized from the monorepo path:

```text
rust/apps/screenshot-daemon
```

This workspace exposes the project in a smaller open-source-oriented shape.

`Cargo.toml` at this folder is a real workspace manifest. The
`screenshot-daemon/` package folder is a real directory so GitHub displays the
source files normally.

## Synced Folders

| Folder | Purpose |
| --- | --- |
| `screenshot-daemon/` | synced package folder for the full package; builds both binaries |

The mapping is recorded at `../mapping.md`. Run
`./scripts/sync_screenshot_daemon.sh` to refresh this folder from the monorepo
source. Packaged runtime output lives in `release/`.

## Binaries

The package builds two binaries:

- `screenshot-daemon` - background daemon that listens for the screenshot hotkey.
- `region-selector` - standalone region selector and annotation tool.

## Build

Build from the monorepo Rust workspace:

```bash
cd ../../rust
cargo build -p screenshot-daemon --release
```

Build from this independent open-source workspace:

```bash
cargo build --workspace --release

cargo build -p screenshot-daemon --release
cargo build --bin region-selector --release
```

The release binaries are produced under:

```text
rust/target/release/screenshot-daemon
rust/target/release/region-selector
```

## Release

Create a small optimized Linux x86_64 release folder:

```bash
./scripts/release_folder.sh
```

The release folder is written to:

```text
release/screenshot-tool-v0.1.0-linux-x86_64
```

Pack that folder into a shareable `.7z`:

```bash
./scripts/pack_to_7z.sh release/screenshot-tool-v0.1.0-linux-x86_64
```

The archive is written next to the folder:

```text
release/screenshot-tool-v0.1.0-linux-x86_64.7z
```

You can override the release folder version name:

```bash
./scripts/release_folder.sh 0.1.1
./scripts/pack_to_7z.sh release/screenshot-tool-v0.1.1-linux-x86_64
```

## Run

Daemon mode:

```bash
../../rust/target/release/screenshot-daemon
```

Standalone region selector:

```bash
../../rust/target/release/region-selector --output ~/Pictures/screenshots/shot.png
```

## Requirements

- Linux desktop with X11 or Wayland
- access to input events for global hotkey detection
- screenshot output directory such as `~/Pictures/screenshots`
