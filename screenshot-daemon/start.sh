#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

CARGO_ARGS=(build --release --bins)
BIN_DIR="$CARGO_TARGET_DIR/release"

echo "[start] building screenshot-daemon, region-selector and record-control (release)"
cargo "${CARGO_ARGS[@]}"

DAEMON="$BIN_DIR/screenshot-daemon"
SELECTOR="$BIN_DIR/region-selector"
RECORD_CTRL="$BIN_DIR/record-control"

if [[ ! -x "$DAEMON" ]]; then
  echo "[start] missing executable: $DAEMON" >&2
  exit 1
fi

if [[ ! -x "$SELECTOR" ]]; then
  echo "[start] missing executable: $SELECTOR" >&2
  exit 1
fi

if [[ ! -x "$RECORD_CTRL" ]]; then
  echo "[start] missing executable: $RECORD_CTRL" >&2
  exit 1
fi

echo "[start] launching screenshot-daemon"
echo "[start] region selector: $SELECTOR"
echo "[start] record control: $RECORD_CTRL"
echo "[start] press Ctrl+Alt+A for region selection"
echo "[start] press Ctrl+Alt+R to start/stop video recording"
exec "$DAEMON"
