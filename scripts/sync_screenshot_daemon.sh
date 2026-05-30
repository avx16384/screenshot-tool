#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_root/../../rust/apps/screenshot-daemon"
target_dir="$repo_root/screenshot-daemon"
link_target="../../rust/apps/screenshot-daemon"

if [[ ! -d "$source_dir" ]]; then
  echo "Error: source folder not found: $source_dir" >&2
  exit 1
fi

if [[ -L "$target_dir" || -e "$target_dir" ]]; then
  rm -rf "$target_dir"
fi

ln -s "$link_target" "$target_dir"

echo "$target_dir"
