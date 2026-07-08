# Current Progress — Region selector subprocess fix

Task: Fix the winit `EventLoop can't be recreated` error that makes the
region hotkey fail on the second invocation. Switch the daemon from
in-process dlopen (`libregion_selector.so`) to spawning the
`region-selector` binary as a subprocess.

## Plan

### Phase 1 — Plan + docs (before any code)
- [x] Write `.trae/documents/screenshot-daemon-region-selector-subprocess.md`
- [x] Write `opensource/screenshot-tool/current-progress.md`

### Phase 2 — Implement subprocess path
- [x] Create `screenshot-daemon/src/selector_proc.rs` (async subprocess spawner)
- [x] Rewire `capture.rs` `capture_region` + `select_record_region` to use `selector_proc::run`
- [x] Register `mod selector_proc;` in `main.rs`
- [x] Remove dead selector dlopen code from `capi_runtime.rs`
- [x] Bump deploy script VERSION to 0.1.6

### Phase 3 — Build + verify
- [x] Rebuild + redeploy v0.1.6 via `/tmp/deploy-screenshot-tray.sh`
- [x] Verify region hotkey works twice in a row (no EventLoop error)
- [x] Verify record hotkey still returns region
- [x] Commit

## Result
- Region capture triggered twice via D-Bus (`screenshot-daemon trigger region`).
  Subprocess #1 (pid 105096) and #2 (pid 105352) both spawned cleanly at
  `/home/phnics/.local/opt/screenshot-tool/libexec/region-selector`. Log shows
  `spawning region-selector` twice with NO `EventLoop can't be recreated`.
  Daemon stayed alive + D-Bus name held throughout. FIX CONFIRMED.

## Notes
- Root cause: winit allows only ONE `EventLoop` per process. Daemon dlopens
  the selector in-process → process lives across captures → second
  `eframe::run_native` hits the hard limit → status 1.
- The recording overlay uses `tinyui` (raw Wayland), NOT winit — unaffected,
  stays on dlopen.
- `region-selector` binary already speaks the stdout protocol
  (`cancelled` / `saved:<path>` / `fullscreen` / `region:x,y,w,h`).
