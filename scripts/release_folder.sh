#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-0.1.4}
cargo_target_dir=${CARGO_TARGET_DIR:-/tmp/screenshot-tool-target-release}
target_dir="$cargo_target_dir/release"
release_root="$repo_root/release"
release_name="screenshot-tool-v${version}-linux-x86_64"
release_dir="$release_root/$release_name"
archive_path="$release_root/$release_name.tar.gz"

mkdir -p "$release_root"
rm -rf "$release_dir" "$archive_path"

CARGO_TARGET_DIR="$cargo_target_dir" cargo build --manifest-path "$repo_root/Cargo.toml" --release

mkdir -p "$release_dir/bin" "$release_dir/libexec" "$release_dir/lib" "$release_dir/docs" "$release_dir/share/icons/hicolor/scalable/apps" "$release_dir/share/autostart"

for bin in screenshot-daemon region-selector deps-dialog; do
  install -m 0755 "$target_dir/$bin" "$release_dir/libexec/$bin"
done

find_artifact() {
  local name=$1
  if [[ -f "$target_dir/$name" ]]; then
    printf '%s\n' "$target_dir/$name"
  elif [[ -f "$target_dir/deps/$name" ]]; then
    printf '%s\n' "$target_dir/deps/$name"
  else
    echo "missing build artifact: $name" >&2
    return 1
  fi
}

install -m 0755 "$(find_artifact libregion_selector.so)" "$release_dir/libexec/libregion_selector.so"
install -m 0755 "$repo_root/lib/libregion_overlay_capi.so" "$release_dir/lib/libregion_overlay_capi.so"
install -m 0644 "$repo_root/README.md" "$release_dir/docs/README.md"
install -m 0644 "$repo_root/LICENSE" "$release_dir/docs/LICENSE"

# Install the SVG tray icon into the hicolor icon theme
icon_src="$repo_root/screenshot-daemon/assets/screenshot-daemon.svg"
if [[ -f "$icon_src" ]]; then
  install -m 0644 "$icon_src" "$release_dir/share/icons/hicolor/scalable/apps/screenshot-daemon.svg"
fi

for bin in screenshot-daemon region-selector deps-dialog; do
  cat > "$release_dir/bin/$bin" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# Resolve symlinks so ROOT_DIR is correct even when invoked via PATH symlink
SCRIPT_PATH="\$(readlink -f "\${BASH_SOURCE[0]}")"
ROOT_DIR="\$(cd "\$(dirname "\$SCRIPT_PATH")/.." && pwd)"
export LD_LIBRARY_PATH="\$ROOT_DIR/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
export SCREENSHOT_DAEMON_SELECTOR_CAPI="\$ROOT_DIR/libexec/libregion_selector.so"
export SCREENSHOT_DAEMON_OVERLAY_CAPI="\$ROOT_DIR/lib/libregion_overlay_capi.so"
exec "\$ROOT_DIR/libexec/$bin" "\$@"
EOF
  chmod 0755 "$release_dir/bin/$bin"
done

# Autostart .desktop file (freedesktop autostart spec, NOT systemd)
cat > "$release_dir/share/autostart/screenshot-daemon.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Screenshot Daemon
Comment=Screenshot and screen recording tool with system tray icon
Exec=screenshot-daemon
Icon=screenshot-daemon
Terminal=false
Categories=Utility;Graphics;
X-GNOME-Autostart-enabled=true
X-KDE-autostart-after=panel
EOF

cat > "$release_dir/install" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local/opt/screenshot-tool}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
AUTOSTART_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

mkdir -p "$PREFIX" "$BIN_DIR" "$ICON_DIR" "$AUTOSTART_DIR"
cp -a "$ROOT_DIR/." "$PREFIX/"

for bin in screenshot-daemon region-selector deps-dialog; do
  ln -sfn "$PREFIX/bin/$bin" "$BIN_DIR/$bin"
done

# Install SVG tray icon into the hicolor icon theme
if [[ -f "$PREFIX/share/icons/hicolor/scalable/apps/screenshot-daemon.svg" ]]; then
  install -m 0644 "$PREFIX/share/icons/hicolor/scalable/apps/screenshot-daemon.svg" "$ICON_DIR/screenshot-daemon.svg"
  gtk-update-icon-cache -f "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" 2>/dev/null || true
fi

# Install autostart entry (freedesktop autostart spec, NOT systemd)
if [[ -f "$PREFIX/share/autostart/screenshot-daemon.desktop" ]]; then
  install -m 0644 "$PREFIX/share/autostart/screenshot-daemon.desktop" "$AUTOSTART_DIR/screenshot-daemon.desktop"
fi

echo "installed to $PREFIX"
echo "binaries linked in $BIN_DIR"
echo "tray icon installed to $ICON_DIR"
echo "autostart entry installed to $AUTOSTART_DIR"
echo ""
echo "Start manually:   screenshot-daemon"
echo "Enable boot start: (autostart .desktop already installed)"
echo "Stop:             click 'Quit' in the system tray menu"
EOF
chmod 0755 "$release_dir/install"

cat > "$release_dir/uninstall" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local/opt/screenshot-tool}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
AUTOSTART_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

# Note: if the daemon is running, quit it from the tray menu ("Quit") first.
echo "If the daemon is running, quit it from the system tray menu first."

for bin in screenshot-daemon region-selector deps-dialog; do
  rm -f "$BIN_DIR/$bin"
done

rm -f "$ICON_DIR/screenshot-daemon.svg"
rm -f "$AUTOSTART_DIR/screenshot-daemon.desktop"
gtk-update-icon-cache -f "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" 2>/dev/null || true

rm -rf "$PREFIX"

echo "uninstalled screenshot-tool"
EOF
chmod 0755 "$release_dir/uninstall"

cat > "$release_dir/RUNBOOK.md" <<EOF
# screenshot-tool v$version

This is a **normal desktop application** — no systemd required.
Start it and a system tray icon appears; click "Quit" in the tray menu to stop.

## Run without installing

\`\`\`bash
./bin/screenshot-daemon
\`\`\`

A camera icon appears in the system tray. Right-click for the context menu
(Fullscreen Screenshot / Region Screenshot / Start-Stop Recording / Quit).

## Install (binaries + tray icon + autostart entry)

\`\`\`bash
./install
\`\`\`

This installs:
- Binaries to \`~/.local/opt/screenshot-tool/\`
- Symlinks in \`~/.local/bin/\`
- Tray SVG icon to \`~/.local/share/icons/hicolor/scalable/apps/\`
- Autostart \`.desktop\` entry to \`~/.config/autostart/\` (starts on next login)

After installing, start the daemon manually:
\`\`\`bash
screenshot-daemon &
\`\`\`

Or log out and back in — the autostart entry will launch it automatically.

## Uninstall

\`\`\`bash
./uninstall
\`\`\`

The private UI/C API implementation is shipped only as dynamic libraries in \`lib/\`.
No Rust source for private crates is included in this release package.
EOF

(
  cd "$release_root"
  tar -czf "$archive_path" "$release_name"
)

echo "$release_dir"
echo "$archive_path"
