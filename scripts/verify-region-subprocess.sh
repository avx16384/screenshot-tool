#!/usr/bin/env bash
# verify-region-subprocess.sh — regression guard for the winit
# one-EventLoop-per-process fix.
#
# Background
# ----------
# winit's `EventLoop` can only be created ONCE per process on Linux.
# `eframe::run_native` builds an `EventLoop` internally. The daemon used to
# `dlopen` `libregion_selector.so` and call the C entry point in-process,
# so the daemon process lived across captures: the second region capture
# hit winit's hard limit and failed with
#   `winit EventLoopError: EventLoop can't be recreated`
#   → `selector capi returned status 1`.
#
# The fix: spawn the `region-selector` binary as a SUBPROCESS for each
# capture. Each capture gets its own process → its own `EventLoop` → the
# recreation error cannot occur.
#
# This script guards against regressions on two levels:
#   1. STATIC  — scan the Rust source to confirm the daemon still routes the
#                region selector through `selector_proc::run` (subprocess),
#                NOT the old in-process `capi_runtime::run_region_selector`.
#   2. RUNTIME — trigger region capture TWICE in a row via the D-Bus service
#                and assert no `EventLoop can't be recreated` / `EventLoopError`
#                appears in the daemon log. Before the fix this was the exact
#                failure mode on the 2nd capture.
#
# Usage
# -----
#   ./scripts/verify-region-subprocess.sh           # static + runtime (default)
#   ./scripts/verify-region-subprocess.sh --static  # static source check only
#
# Requirements for the runtime check:
#   - screenshot-daemon running with the D-Bus service
#     `org.screenshot_daemon.Service1` registered
#   - `screenshot-daemon` on PATH (for the `trigger` subcommand)
#   - The `region-selector` binary installed alongside the daemon
#
# Exit codes
# ----------
#   0  pass
#   1  static check failed (source regression)
#   2  runtime check failed (EventLoop recreation error seen, or daemon/binary missing)
#   3  user aborted / bad args
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DAEMON_SRC="$REPO_ROOT/screenshot-daemon/src"
LOG_FILE="${SCREENSHOT_DAEMON_LOG:-$HOME/.local/share/screenshot-daemon.log}"

mode="all"
case "${1:-}" in
  --static)  mode="static"  ;;
  --runtime) mode="runtime" ;;
  -h|--help)
    sed -n '2,40p' "$0"; exit 0 ;;
  "")        mode="all"      ;;
  *) echo "unknown arg: $1" >&2; exit 3 ;;
esac

# ─────────────────────────────────────────────────────────────────────────────
# Layer 1 — STATIC source check
# ─────────────────────────────────────────────────────────────────────────────
# Confirm capture.rs calls selector_proc::run (subprocess), and that the old
# in-process dlopen entry point (capi_runtime::run_region_selector) is NOT
# referenced from capture.rs. Also confirm selector_proc.rs exists.
static_check() {
  local rc=0

  if [[ ! -f "$DAEMON_SRC/selector_proc.rs" ]]; then
    echo "STATIC FAIL: $DAEMON_SRC/selector_proc.rs is missing — the subprocess spawner must exist"
    rc=1
  fi

  if ! grep -qE 'selector_proc::run' "$DAEMON_SRC/capture.rs"; then
    echo "STATIC FAIL: capture.rs does not call selector_proc::run — region selector is no longer a subprocess"
    rc=1
  fi

  if grep -nE 'capi_runtime::run_region_selector' "$DAEMON_SRC/capture.rs" >/dev/null 2>&1; then
    echo "STATIC FAIL: capture.rs references capi_runtime::run_region_selector — in-process dlopen must not be used for the region selector (winit one-EventLoop-per-process limit)"
    rc=1
  fi

  if [[ $rc -eq 0 ]]; then
    echo "STATIC PASS: capture.rs routes through selector_proc::run (subprocess); no in-process run_region_selector call"
  fi
  return $rc
}

# ─────────────────────────────────────────────────────────────────────────────
# Layer 2 — RUNTIME integration check
# ─────────────────────────────────────────────────────────────────────────────
# Trigger region capture twice via the D-Bus service and assert the daemon
# spawns a fresh region-selector subprocess each time, with NO EventLoop
# recreation error in the log.
#
# Detection is LOG-BASED, not PID-based: the overlay may complete (save or
# cancel) in well under a second, so `pgrep` is unreliable. Instead we watch
# the daemon log for `spawning region-selector` lines from `selector_proc`.
runtime_check() {
  # 2a. prerequisites
  if ! command -v screenshot-daemon >/dev/null 2>&1; then
    echo "RUNTIME SKIP: screenshot-daemon not on PATH (cannot run trigger subcommand)"
    return 2
  fi
  if ! pgrep -f 'libexec/screenshot-daemon' >/dev/null 2>&1; then
    echo "RUNTIME SKIP: screenshot-daemon is not running — start it first ('screenshot-daemon &' or via autostart)"
    return 2
  fi
  if ! timeout 5 gdbus call --session --dest org.freedesktop.DBus \
        --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner \
        'org.screenshot_daemon.Service1' 2>/dev/null | grep -q true; then
    echo "RUNTIME SKIP: D-Bus name org.screenshot_daemon.Service1 not registered — daemon must be running with the D-Bus service"
    return 2
  fi
  if [[ ! -f "$LOG_FILE" ]]; then
    echo "RUNTIME SKIP: log file $LOG_FILE not found (set SCREENSHOT_DAEMON_LOG to override)"
    return 2
  fi

  # Wait until the log grows with a line matching $1, scanning only lines added
  # after $2 (baseline line count). Times out after $3 seconds. Returns 0 if
  # the pattern matched, 1 on timeout.
  wait_for_log() {
    local pattern="$1" baseline="$2" secs="$3"
    for _ in $(seq 1 "$secs"); do
      sleep 1
      if tail -n +$((baseline + 1)) "$LOG_FILE" 2>/dev/null | grep -qE "$pattern"; then
        return 0
      fi
    done
    return 1
  }

  # Cancel any live region-selector overlay so the daemon's main loop is freed
  # for the next trigger. SIGTERM lets eframe close cleanly; if the process is
  # already gone this is a no-op.
  cancel_overlay() {
    pkill -TERM -x region-selector 2>/dev/null || true
    sleep 2
  }

  local log_before spawn_count eventloop_hits
  log_before=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
  echo "RUNTIME: log baseline = $log_before lines"

  # 2b. trigger #1
  echo "RUNTIME: trigger #1 (region)"
  if ! screenshot-daemon trigger region; then
    echo "RUNTIME FAIL: 'screenshot-daemon trigger region' returned non-zero"
    return 2
  fi
  if ! wait_for_log "spawning region-selector" "$log_before" 10; then
    echo "RUNTIME FAIL: no 'spawning region-selector' log line after trigger #1"
    tail -n 20 "$LOG_FILE" 2>/dev/null || true
    return 2
  fi
  echo "RUNTIME: region-selector subprocess #1 spawned (log-confirmed)"
  cancel_overlay

  # 2c. trigger #2 — THE KEY TEST. Before the fix this failed in-process
  #     with `EventLoop can't be recreated` on the second capture.
  local log_after_1
  log_after_1=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
  echo "RUNTIME: trigger #2 (region) — key test"
  if ! screenshot-daemon trigger region; then
    echo "RUNTIME FAIL: 'screenshot-daemon trigger region' (2nd) returned non-zero"
    return 2
  fi
  if ! wait_for_log "spawning region-selector" "$log_after_1" 10; then
    echo "RUNTIME FAIL: no 'spawning region-selector' log line after trigger #2"
    echo "--- log since #1 ---"
    tail -n +$((log_after_1 + 1)) "$LOG_FILE" 2>/dev/null || true
    return 2
  fi
  echo "RUNTIME: region-selector subprocess #2 spawned (FIX HOLDS — 2nd process spawned)"
  cancel_overlay

  # 2d. Final assertion over everything logged since the start: the daemon
  #     must have spawned the selector at least twice and must NOT have logged
  #     the winit EventLoop recreation error at any point.
  #
  # NOTE: `grep -c` prints the count (even "0") to stdout AND exits 1 when the
  # count is 0, so we must NOT append `|| echo 0` — that would double the "0"
  # and break the integer comparison. `set -o pipefail` doesn't affect
  # command substitution's captured stdout, only its exit code.
  spawn_count=$(tail -n +$((log_before + 1)) "$LOG_FILE" 2>/dev/null | grep -cE "spawning region-selector" || true)
  eventloop_hits=$(tail -n +$((log_before + 1)) "$LOG_FILE" 2>/dev/null | grep -cE "EventLoop can't be recreated|EventLoopError" || true)
  spawn_count=${spawn_count:-0}
  eventloop_hits=${eventloop_hits:-0}

  if [[ "$eventloop_hits" -ne 0 ]]; then
    echo "RUNTIME FAIL: EventLoop recreation error appears $eventloop_hits time(s) in the log"
    tail -n +$((log_before + 1)) "$LOG_FILE" 2>/dev/null | grep -E "EventLoop|selector" || true
    return 2
  fi
  if [[ "$spawn_count" -lt 2 ]]; then
    echo "RUNTIME FAIL: expected ≥2 'spawning region-selector' log lines, got $spawn_count"
    return 2
  fi

  echo "RUNTIME PASS: region-selector spawned $spawn_count times, 0 EventLoop errors"
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Run
# ─────────────────────────────────────────────────────────────────────────────
final_rc=0

if [[ "$mode" == "all" || "$mode" == "static" ]]; then
  echo "=== STATIC CHECK ==="
  static_check || final_rc=1
fi

if [[ "$mode" == "all" || "$mode" == "runtime" ]]; then
  echo ""
  echo "=== RUNTIME CHECK ==="
  runtime_check
  rc=$?
  # A runtime SKIP (rc=2) due to missing daemon should not fail the whole script
  # if the static check already passed — but a real EventLoop regression (also
  # rc=2 with an error message) should. We treat rc=2 as failure either way so
  # CI catches missing prerequisites; developers can use --static to skip it.
  if [[ $rc -ne 0 ]]; then
    final_rc=${final_rc:-2}
    [[ $final_rc -eq 0 ]] && final_rc=2
  fi
fi

echo ""
if [[ $final_rc -eq 0 ]]; then
  echo "RESULT: PASS — region selector subprocess invariant holds"
else
  echo "RESULT: FAIL (rc=$final_rc)"
fi
exit $final_rc
