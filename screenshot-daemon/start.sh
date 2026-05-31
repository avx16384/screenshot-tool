#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

CARGO_ARGS=(build --release -p screenshot-daemon -p region-overlay-capi)
BIN_DIR="$CARGO_TARGET_DIR/release"

echo "[start] building screenshot-daemon and C API dylibs (release)"
cargo "${CARGO_ARGS[@]}"

DAEMON="$BIN_DIR/screenshot-daemon"
SELECTOR_CAPI="$BIN_DIR/libscreenshot_daemon.so"
OVERLAY_CAPI="$BIN_DIR/libregion_overlay_capi.so"

if [[ ! -x "$DAEMON" ]]; then
  echo "[start] missing executable: $DAEMON" >&2
  exit 1
fi

if [[ ! -f "$SELECTOR_CAPI" ]]; then
  echo "[start] missing selector C API dylib: $SELECTOR_CAPI" >&2
  exit 1
fi

if [[ ! -f "$OVERLAY_CAPI" ]]; then
  echo "[start] missing overlay C API dylib: $OVERLAY_CAPI" >&2
  exit 1
fi

echo "[start] launching screenshot-daemon"
echo "[start] selector C API: $SELECTOR_CAPI"
echo "[start] overlay C API: $OVERLAY_CAPI"
echo "[start] press Ctrl+Alt+A for region selection"
echo "[start] press Ctrl+Alt+R to start/stop video recording"
exec "$DAEMON"
