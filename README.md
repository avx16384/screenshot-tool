# screenshot-tool

This is the open-source workspace for the Linux screenshot daemon and
standalone region selector.

The real source remains in the monorepo at:

```text
rust/apps/screenshot-daemon
```

This shadow workspace exposes the project in a smaller open-source-oriented shape
without duplicating source files.

`Cargo.toml` at this folder is a real workspace manifest. The Rust source files
inside the member crates are shadow links to the monorepo source.

## Shadow Folders

| Folder | Purpose |
| --- | --- |
| `daemon/` | shadow folder for the `screenshot-daemon` binary |
| `region-selector/` | shadow folder for the standalone `region-selector` binary |
| `release/` | packaged binaries, systemd units, runbook, and release docs |

The mapping is recorded at `../mapping.md`.

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

- Linux with X11
- access to input events for global hotkey detection
- screenshot output directory such as `~/Pictures/screenshots`
