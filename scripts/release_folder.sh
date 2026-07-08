#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-0.1.7}
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
export SCREENSHOT_DAEMON_SELECTOR_BIN="\$ROOT_DIR/libexec/region-selector"
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
echo "Start:            ./start.sh   (or: screenshot-daemon &)"
echo "Stop:             ./close.sh   (or: click 'Quit' in the tray menu)"
echo "Autostart:        .desktop entry installed (starts on next login)"
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

cat > "$release_dir/start.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

# Start the screenshot daemon in the background (detached).
ROOT_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
DAEMON="$ROOT_DIR/libexec/screenshot-daemon"

if [[ ! -x "$DAEMON" ]]; then
  echo "Error: daemon binary not found: $DAEMON" >&2
  exit 1
fi

export LD_LIBRARY_PATH="$ROOT_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export SCREENSHOT_DAEMON_SELECTOR_BIN="$ROOT_DIR/libexec/region-selector"
export SCREENSHOT_DAEMON_OVERLAY_CAPI="$ROOT_DIR/lib/libregion_overlay_capi.so"

# Refuse to start a second instance if the D-Bus name is already owned.
if gdbus call --session \
    --dest org.freedesktop.DBus \
    --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner \
    org.screenshot_daemon.Service1 2>/dev/null | grep -q true; then
  echo "screenshot-daemon is already running"
  exit 0
fi

LOG_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/screenshot-tool"
mkdir -p "$LOG_DIR"
nohup "$DAEMON" >>"$LOG_DIR/daemon.log" 2>&1 &
echo "screenshot-daemon started (pid $!)"
echo "logs: $LOG_DIR/daemon.log"
EOF
chmod 0755 "$release_dir/start.sh"

cat > "$release_dir/close.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

# Stop the screenshot daemon cleanly via its D-Bus Quit method (no kill/pkill).
if ! gdbus call --session \
    --dest org.freedesktop.DBus \
    --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner \
    org.screenshot_daemon.Service1 2>/dev/null | grep -q true; then
  echo "screenshot-daemon is not running"
  exit 0
fi

gdbus call --session \
  --dest org.screenshot_daemon.Service1 \
  --object-path /org/screenshot_daemon/Service \
  --method org.screenshot_daemon.Service1.Quit

echo "screenshot-daemon stop requested"
EOF
chmod 0755 "$release_dir/close.sh"

cat > "$release_dir/RUNBOOK.md" <<EOF
# screenshot-tool v$version

A **normal desktop application** — no systemd required. Launch it with
\`./start.sh\`, a camera icon appears in the system tray, click the tray icon
for the context menu, and stop it with \`./close.sh\`.

## Quick start (no install)

\`\`\`bash
./start.sh      # launch daemon in the background (detached, logs to ~/.local/share/screenshot-tool/daemon.log)
./close.sh      # stop the daemon cleanly via D-Bus Quit (no kill/pkill)
\`\`\`

A camera icon appears in the system tray. Click it for the context menu
(Fullscreen Screenshot / Region Screenshot / Start / Stop Recording / Quit).

To run in the foreground (e.g. for debugging), use the wrapper directly:
\`\`\`bash
./bin/screenshot-daemon
\`\`\`

## Install (binaries + tray icon + autostart entry)

\`\`\`bash
./install
\`\`\`

This installs:
- Binaries to \`~/.local/opt/screenshot-tool/\`
- Symlinks in \`~/.local/bin/\` (so \`screenshot-daemon\` is on PATH)
- Tray SVG icon to \`~/.local/share/icons/hicolor/scalable/apps/\`
- Autostart \`.desktop\` entry to \`~/.config/autostart/\` (starts on next login)

After installing, start the daemon:
\`\`\`bash
./start.sh             # from this release folder
# — or —
screenshot-daemon &    # from anywhere (uses the installed symlink)
\`\`\`

Or log out and back in — the autostart entry will launch it automatically.

## Stop

\`\`\`bash
./close.sh       # clean D-Bus Quit
\`\`\`

Or click "Quit" in the system tray context menu.

## Uninstall

Stop the daemon first (\`./close.sh\` or tray "Quit"), then:
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
