#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-0.1.2}
release_root="$repo_root/release"
release_name="screenshot-tool-v${version}-linux-x86_64"
release_dir="$release_root/$release_name"
rpm_root="$release_root/rpm"
buildroot="$rpm_root/BUILDROOT/screenshot-tool-${version}-1.x86_64"
spec_path="$rpm_root/SPECS/screenshot-tool.spec"

if [[ ! -d "$release_dir" ]]; then
  "$repo_root/scripts/release_folder.sh" "$version" >/dev/null
fi

rm -rf "$rpm_root"
mkdir -p "$rpm_root/SPECS" "$rpm_root/RPMS" "$rpm_root/BUILD" "$rpm_root/SOURCES" "$rpm_root/SRPMS" "$rpm_root/rpmdb"

cat > "$spec_path" <<EOF
Name: screenshot-tool
Version: $version
Release: 1%{?dist}
Summary: Linux screenshot and screen recording tool
License: MIT
BuildArch: x86_64
Requires: ffmpeg-libs
Requires: mesa-libgbm
Requires: libvpx
Requires: opus

%description
Screenshot and screen recording daemon for X11 and Wayland.

%install
mkdir -p %{buildroot}/opt/screenshot-tool
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/doc/screenshot-tool
cp -a $release_dir/. %{buildroot}/opt/screenshot-tool/
ln -s /opt/screenshot-tool/bin/screenshot-daemon %{buildroot}/usr/bin/screenshot-daemon
ln -s /opt/screenshot-tool/bin/region-selector %{buildroot}/usr/bin/region-selector
ln -s /opt/screenshot-tool/bin/deps-dialog %{buildroot}/usr/bin/deps-dialog
install -m 0644 $release_dir/docs/README.md %{buildroot}/usr/share/doc/screenshot-tool/README.md
install -m 0644 $release_dir/docs/LICENSE %{buildroot}/usr/share/doc/screenshot-tool/LICENSE

%files
/opt/screenshot-tool
/usr/bin/screenshot-daemon
/usr/bin/region-selector
/usr/bin/deps-dialog
/usr/share/doc/screenshot-tool
EOF

local_rpmbuild="$repo_root/tools/rpm-root/usr/bin/rpmbuild"
local_rpmlib="$repo_root/tools/rpm-root/usr/lib/x86_64-linux-gnu"
local_rpmconfig="$repo_root/tools/rpm-root/usr/lib/rpm"

if [[ -x "$local_rpmbuild" ]]; then
  LD_LIBRARY_PATH="$local_rpmlib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    RPM_CONFIGDIR="$local_rpmconfig" \
    "$local_rpmbuild" --define "_topdir $rpm_root" --define "_dbpath $rpm_root/rpmdb" --buildroot "$buildroot" -bb "$spec_path"
  find "$rpm_root/RPMS" -type f -name '*.rpm' -print
elif command -v rpmbuild >/dev/null 2>&1; then
  rpmbuild --define "_topdir $rpm_root" --define "_dbpath $rpm_root/rpmdb" --buildroot "$buildroot" -bb "$spec_path"
  find "$rpm_root/RPMS" -type f -name '*.rpm' -print
else
  echo "rpmbuild not found. Install rpm-build or unpack it under tools/rpm-root, then run:" >&2
  echo "  rpmbuild --define '_topdir $rpm_root' --buildroot '$buildroot' -bb '$spec_path'" >&2
  echo "$spec_path"
fi
