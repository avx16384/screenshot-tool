#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-0.1.2}
release_root="$repo_root/release"
release_name="screenshot-tool-v${version}-linux-x86_64"
release_dir="$release_root/$release_name"
pkg_root="$release_root/deb/screenshot-tool_${version}_amd64"
deb_path="$release_root/screenshot-tool_${version}_amd64.deb"

if [[ ! -d "$release_dir" ]]; then
  "$repo_root/scripts/release_folder.sh" "$version" >/dev/null
fi

rm -rf "$pkg_root" "$deb_path"
mkdir -p "$pkg_root/DEBIAN"
mkdir -p "$pkg_root/opt/screenshot-tool"
mkdir -p "$pkg_root/usr/bin"
mkdir -p "$pkg_root/usr/share/doc/screenshot-tool"

cp -a "$release_dir/." "$pkg_root/opt/screenshot-tool/"

for bin in screenshot-daemon region-selector deps-dialog; do
  ln -s "/opt/screenshot-tool/bin/$bin" "$pkg_root/usr/bin/$bin"
done

install -m 0644 "$release_dir/docs/README.md" "$pkg_root/usr/share/doc/screenshot-tool/README.md"
install -m 0644 "$release_dir/docs/LICENSE" "$pkg_root/usr/share/doc/screenshot-tool/copyright"

installed_size=$(du -sk "$pkg_root" | awk '{print $1}')
cat > "$pkg_root/DEBIAN/control" <<EOF
Package: screenshot-tool
Version: $version
Section: utils
Priority: optional
Architecture: amd64
Installed-Size: $installed_size
Maintainer: avx16384
Depends: libgbm1, libavcodec60, libavformat60, libavutil58, libswscale7, libvpx7, libopus0
Description: Linux screenshot and screen recording tool
 Screenshot and screen recording daemon for X11 and Wayland.
EOF

cat > "$pkg_root/DEBIAN/postinst" <<'EOF'
#!/usr/bin/env bash
set -e
echo "screenshot-tool installed. Run: screenshot-daemon"
EOF
chmod 0755 "$pkg_root/DEBIAN/postinst"

dpkg-deb --build --root-owner-group "$pkg_root" "$deb_path"
echo "$deb_path"
