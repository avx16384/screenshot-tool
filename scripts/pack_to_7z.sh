#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $(basename "$0") <release-folder>" >&2
  exit 2
fi

release_dir=${1%/}

if [[ ! -d "$release_dir" ]]; then
  echo "Error: release folder not found: $release_dir" >&2
  exit 1
fi

parent_dir=$(cd "$(dirname "$release_dir")" && pwd)
folder_name=$(basename "$release_dir")
archive_path="$parent_dir/$folder_name.7z"

rm -f "$archive_path"

(
  cd "$parent_dir"
  7z a "$archive_path" "$folder_name" >/dev/null
)

echo "$archive_path"
