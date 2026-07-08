//! Spawn the `region-selector` binary as a subprocess.
//!
//! ## Why a subprocess (not in-process dlopen)
//!
//! `winit`'s `EventLoop` can only be created **once per process** on Linux.
//! `eframe::run_native` builds an `EventLoop` internally. The daemon used to
//! `dlopen` `libregion_selector.so` and call the C entry point in-process,
//! so the daemon process lived across captures: the first region capture
//! created and dropped an `EventLoop`, and the second capture hit winit's
//! hard one-`EventLoop`-per-process limit and panicked across the
//! `extern "C"` boundary (`EventLoopError: EventLoop can't be recreated`).
//!
//! Spawning the `region-selector` binary as a fresh child process for each
//! capture gives each invocation its own process and therefore its own
//! `EventLoop` — the recreation error cannot occur.
//!
//! The recording overlay is unaffected: it uses the private `tinyui` crate
//! (raw Wayland via `smithay-client-toolkit`), not winit, and stays on the
//! dlopen path in `capi_runtime`.
//!
//! ## Protocol
//!
//! The binary prints exactly one line to stdout (see `region_selector::main`):
//!
//! | stdout line          | meaning                          |
//! |----------------------|----------------------------------|
//! | `cancelled`          | user pressed Esc                 |
//! | `saved:<path>`       | selection saved to `<path>`      |
//! | `fullscreen`         | record-mode, no region selected  |
//! | `region:x,y,w,h`     | record-mode region chosen        |
//! | _(no output)_        | noop                             |
//!
//! CLI: `--output <path>` · `--background <path>` · `--record`.

use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Spawn the `region-selector` binary and return its stdout protocol line.
///
/// Returns `Ok(None)` when the binary exits cleanly with no stdout output
/// (the `Noop` outcome). Returns `Ok(Some(line))` with the trimmed protocol
/// line on a clean exit with output. Returns `Err` on a non-zero exit, with
/// the captured stderr included in the error message for diagnostics.
pub async fn run(
    output_path: Option<&Path>,
    background_path: Option<&Path>,
    record_mode: bool,
) -> anyhow::Result<Option<String>> {
    let bin = selector_bin_path();
    let lib_dir = selector_lib_dir();

    let mut cmd = Command::new(&bin);
    // Inherit the session environment so the child sees WAYLAND_DISPLAY,
    // DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS, etc. — required
    // for Wayland/X11 surface creation and clipboard access.
    cmd.env_clear();
    for (k, v) in std::env::vars_os() {
        cmd.env(k, v);
    }
    // Mirror the `bin/` wrapper: prepend the private lib dir so the child
    // resolves any bundled shared libraries exactly as the wrapper would.
    if let Some(ref lib_dir) = lib_dir {
        let existing = std::env::var_os("LD_LIBRARY_PATH")
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default();
        let combined = if existing.is_empty() {
            lib_dir.to_string_lossy().into_owned()
        } else {
            format!("{}:{}", lib_dir.display(), existing)
        };
        cmd.env("LD_LIBRARY_PATH", combined);
    }

    if let Some(path) = output_path {
        cmd.arg("--output").arg(path);
    }
    if let Some(path) = background_path {
        cmd.arg("--background").arg(path);
    }
    if record_mode {
        cmd.arg("--record");
    }

    // Capture stdout (the protocol line) and stderr (diagnostics). Do not
    // inherit stdin — the selector reads no input from the daemon.
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    log::info!(
        "spawning region-selector: {} (output={:?}, background={:?}, record={})",
        bin.display(),
        output_path,
        background_path,
        record_mode
    );

    let output = cmd
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("failed to spawn region-selector ({}): {e}", bin.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let code = output.status.code();
        let stderr_block = if stderr.is_empty() {
            "<no stderr>".to_string()
        } else {
            stderr
        };
        anyhow::bail!(
            "region-selector exited {code:?} (status {}): {stderr_block}",
            output.status
        );
    }

    if stdout.is_empty() {
        Ok(None)
    } else {
        Ok(Some(stdout))
    }
}

/// Resolve the `region-selector` binary path.
///
/// Override: `SCREENSHOT_DAEMON_SELECTOR_BIN` env var (absolute path).
/// Default: `current_exe_dir()/region-selector` — the daemon binary lives in
/// `libexec/`, and `region-selector` is installed alongside it by
/// `release_folder.sh` (also into `libexec/`).
fn selector_bin_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SCREENSHOT_DAEMON_SELECTOR_BIN") {
        return PathBuf::from(p);
    }
    current_exe_dir().join("region-selector")
}

/// Resolve the private shared-library dir (`<install_root>/lib`) so it can be
/// prepended to the child's `LD_LIBRARY_PATH`, mirroring the `bin/` wrapper.
///
/// The daemon binary is at `<install_root>/libexec/screenshot-daemon`, so the
/// lib dir is `current_exe_dir().parent()/lib` when that exists.
fn selector_lib_dir() -> Option<PathBuf> {
    let lib = current_exe_dir().parent()?.join("lib");
    if lib.is_dir() {
        Some(lib)
    } else {
        None
    }
}

fn current_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(ToOwned::to_owned))
        .unwrap_or_else(|| PathBuf::from("."))
}
