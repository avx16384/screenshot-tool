#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

CARGO_ARGS=(build --release --bins)
BIN_DIR="$CARGO_TARGET_DIR/release"

echo "[start] building screenshot-daemon and region-selector (release)"
cargo "${CARGO_ARGS[@]}"

DAEMON="$BIN_DIR/screenshot-daemon"
SELECTOR="$BIN_DIR/region-selector"

if [[ ! -x "$DAEMON" ]]; then
  echo "[start] missing executable: $DAEMON" >&2
  exit 1
fi

if [[ ! -x "$SELECTOR" ]]; then
  echo "[start] missing executable: $SELECTOR" >&2
  exit 1
fi

echo "[start] launching screenshot-daemon"
echo "[start] region selector: $SELECTOR"
echo "[start] press Ctrl+Alt+A for region selection"
exec "$DAEMON"
