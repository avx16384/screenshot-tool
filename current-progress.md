# Current Progress — Tray context-menu fix + v0.1.7 release

Task: The tray icon shows in swaybar but **no context menu appears** on click.

Root cause (confirmed via D-Bus introspection):
- The DBusMenu at `/MenuBar` is served **perfectly** (GetLayout returns all 5
  items with correct `children-display`/`icon-name`/`label` properties).
- The host is **swaybar 1.10.1**.
- On right-click swaybar calls `ContextMenu(x,y)` — ksni explicitly refuses it
  (`"Not supported, please use menu"`).
- On left-click swaybar calls `Activate()` — our tray had no `activate` impl (no-op).
- Result: neither click shows a menu.

Fix: set `MENU_ON_ACTIVATE = true` so `ItemIsMenu=true`. The host then renders
the (already correctly served) DBusMenu on click instead of the refused
ContextMenu/Activate path.

Also add a `Quit` D-Bus method so `close.sh` can stop the daemon cleanly without
`kill`/`pkill`, and ship `start.sh` + `close.sh` in the tar.gz.

## Note on v0.1.6

v0.1.6 **already contains the subprocess fix** — verified by binary string
analysis (`SCREENSHOT_DAEMON_SELECTOR_BIN` + `selector_proc.rs` present;
`SCREENSHOT_DAEMON_SELECTOR_CAPI` + `screenshot_region_selector_run` absent).
The earlier claim that v0.1.6 lacked the fix was wrong: the release was built
from the fix-branch working tree, and the commit timestamp was misleading
because of a later `git commit --amend`. So v0.1.7 is NOT about the subprocess
fix — it is about the tray menu fix, dropping dead `libregion_selector.so`
(~11 MB unused), and shipping start.sh/close.sh.

## Plan

### Phase 1 — Tray + D-Bus code  (DONE)
- [x] `tray.rs`: override `const MENU_ON_ACTIVATE: bool = true;`
- [x] `dbus_service.rs`: add `shutdown: Arc<Notify>` to `ScreenshotService`,
      thread it through `register(tx, shutdown)`, add `async fn quit()` that
      calls `self.shutdown.notify_one()`.
- [x] `main.rs`: pass `shutdown.clone()` into `dbus_service::register`.

### Phase 2 — Release packaging  (DONE)
- [x] `release_folder.sh`:
  - bin wrappers: replace dead `SCREENSHOT_DAEMON_SELECTOR_CAPI` with
    `SCREENSHOT_DAEMON_SELECTOR_BIN` (points at libexec/region-selector).
  - drop dead `libregion_selector.so` from packaging (~11 MB unused).
  - add `start.sh` (background-launch daemon with env + D-Bus name check) and
    `close.sh` (call `org.screenshot_daemon.Service1.Quit` via gdbus).
  - RUNBOOK.md + install echo messages now document start.sh / close.sh.
- [x] bump default version: `release_folder.sh` 0.1.4→0.1.7,
      `package_deb.sh`/`package_rpm.sh` 0.1.2→0.1.7.
- [x] workspace `Cargo.toml`: add `[profile.dev]` + `debug = 0` to
      `[profile.release]`.

### Phase 3 — Build + verify + tag  (PENDING)
- [ ] Rebuild daemon; restart; verify `ItemIsMenu=true` on the SNI via D-Bus.
- [ ] Run `release_folder.sh 0.1.7`, `package_deb.sh 0.1.7`, `package_rpm.sh 0.1.7`.
- [ ] Confirm tar.gz contains install/uninstall/start.sh/close.sh.
- [ ] Commit + annotated tag `v0.1.7`.

## Result
(pending)
