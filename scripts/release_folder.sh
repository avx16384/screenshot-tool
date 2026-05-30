#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_root/../../rust/apps/screenshot-daemon"
version=${1:-0.1.2}
cargo_target_dir=${CARGO_TARGET_DIR:-/tmp/screenshot-tool-target-release}
target_dir="$cargo_target_dir/release"
release_root="$repo_root/release"
release_name="screenshot-tool-v${version}-linux-x86_64"
release_dir="$release_root/$release_name"
archive_path="$release_root/$release_name.tar.gz"

mkdir -p "$release_root"
rm -rf "$release_dir" "$archive_path"

CARGO_TARGET_DIR="$cargo_target_dir" cargo build --manifest-path "$source_dir/Cargo.toml" --release

mkdir -p "$release_dir/bin" "$release_dir/libexec" "$release_dir/lib" "$release_dir/docs" "$release_dir/systemd"

for bin in screenshot-daemon region-selector record-region-overlay deps-dialog; do
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

install -m 0755 "$(find_artifact libscreenshot_daemon.so)" "$release_dir/lib/libscreenshot_daemon.so"
install -m 0755 "$(find_artifact libregion_overlay_capi.so)" "$release_dir/lib/libregion_overlay_capi.so"
install -m 0644 "$repo_root/README.md" "$release_dir/docs/README.md"
install -m 0644 "$repo_root/LICENSE" "$release_dir/docs/LICENSE"

for bin in screenshot-daemon region-selector record-region-overlay deps-dialog; do
  cat > "$release_dir/bin/$bin" <<EOF
#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")/.." && pwd)"
export LD_LIBRARY_PATH="\$ROOT_DIR/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
export SCREENSHOT_DAEMON_SELECTOR_CAPI="\$ROOT_DIR/lib/libscreenshot_daemon.so"
export SCREENSHOT_DAEMON_OVERLAY_CAPI="\$ROOT_DIR/lib/libregion_overlay_capi.so"
exec "\$ROOT_DIR/libexec/$bin" "\$@"
EOF
  chmod 0755 "$release_dir/bin/$bin"
done

cat > "$release_dir/systemd/screenshot-daemon.user.service" <<'EOF'
[Unit]
Description=Screenshot daemon hotkey service
After=graphical-session.target
Wants=graphical-session.target

[Service]
Type=simple
Environment=RUST_LOG=info
ExecStart=%h/.local/opt/screenshot-tool/bin/screenshot-daemon
Restart=on-failure
RestartSec=2s

[Install]
WantedBy=default.target
EOF

cat > "$release_dir/install" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local/opt/screenshot-tool}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

mkdir -p "$PREFIX" "$BIN_DIR" "$UNIT_DIR"
cp -a "$ROOT_DIR/." "$PREFIX/"

for bin in screenshot-daemon region-selector record-region-overlay deps-dialog; do
  ln -sfn "$PREFIX/bin/$bin" "$BIN_DIR/$bin"
done

install -m 0644 "$PREFIX/systemd/screenshot-daemon.user.service" "$UNIT_DIR/screenshot-daemon.service"
systemctl --user daemon-reload
systemctl --user enable --now screenshot-daemon.service

echo "installed to $PREFIX"
echo "binaries linked in $BIN_DIR"
echo "service: screenshot-daemon.service"
EOF
chmod 0755 "$release_dir/install"

cat > "$release_dir/uninstall" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local/opt/screenshot-tool}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

systemctl --user disable --now screenshot-daemon.service 2>/dev/null || true
rm -f "$UNIT_DIR/screenshot-daemon.service"
systemctl --user daemon-reload 2>/dev/null || true

for bin in screenshot-daemon region-selector record-region-overlay deps-dialog; do
  rm -f "$BIN_DIR/$bin"
done

rm -rf "$PREFIX"

echo "uninstalled screenshot-tool"
EOF
chmod 0755 "$release_dir/uninstall"

cat > "$release_dir/RUNBOOK.md" <<EOF
# screenshot-tool v$version

Run without installing:

\`\`\`bash
./bin/screenshot-daemon
\`\`\`

Install as a user systemd service:

\`\`\`bash
./install
\`\`\`

Uninstall:

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
