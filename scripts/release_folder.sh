#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-$(grep -m1 '^version = ' "$repo_root/Cargo.toml" | sed -E 's/version = "([^"]+)"/\1/')}
cargo_target_dir=${CARGO_TARGET_DIR:-/tmp/screenshot-tool-target-release}
target_dir="$cargo_target_dir/release"
release_root="$repo_root/release"
release_name="screenshot-tool-v${version}-linux-x86_64"
release_dir="$release_root/$release_name"

mkdir -p "$release_root"
rm -rf "$release_dir"

CARGO_TARGET_DIR="$cargo_target_dir" cargo build --manifest-path "$repo_root/Cargo.toml" --workspace --release

mkdir -p "$release_dir"
cp "$target_dir/screenshot-daemon" "$release_dir/"
cp "$target_dir/region-selector" "$release_dir/"
cp "$repo_root/README.md" "$release_dir/"
cp "$repo_root/LICENSE" "$release_dir/"

cat > "$release_dir/RUN.txt" <<'EOF'
screenshot-tool

This package contains two Linux binaries:
- screenshot-daemon
- region-selector

Supported display servers:
- Wayland
- X11

Run the standalone region selector:
./region-selector --output ~/Pictures/screenshots/shot.png

Run the hotkey daemon:
./screenshot-daemon

The daemon needs permission to read keyboard input events for global hotkeys.
On many systems, that means running it as a user in the input group.
EOF

echo "$release_dir"
