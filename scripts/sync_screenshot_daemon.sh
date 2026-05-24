#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_root/../../rust/apps/screenshot-daemon"
target_dir="$repo_root/screenshot-daemon"

if [[ ! -d "$source_dir" ]]; then
  echo "Error: source folder not found: $source_dir" >&2
  exit 1
fi

if [[ -L "$target_dir" || -e "$target_dir" ]]; then
  rm -rf "$target_dir"
fi

mkdir -p "$target_dir"

while IFS= read -r -d '' dir; do
  rel=${dir#"$source_dir"/}
  [[ "$rel" == "$source_dir" ]] && rel=.
  mkdir -p "$target_dir/$rel"
done < <(find "$source_dir" -path "$source_dir/.codex" -prune -o -type d -print0)

while IFS= read -r -d '' file; do
  rel=${file#"$source_dir"/}
  cp -p "$file" "$target_dir/$rel"
done < <(find "$source_dir" -path "$source_dir/.codex" -prune -o -type f -print0)

echo "$target_dir"
