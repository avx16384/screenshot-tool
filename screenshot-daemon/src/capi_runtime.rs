use std::ffi::CStr;
use std::fmt;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

type OverlayRunFn =
    unsafe extern "C" fn(i32, i32, u32, u32, bool, extern "C" fn(*const c_char)) -> c_int;
type OverlayRunWithConfigFn =
    unsafe extern "C" fn(i32, i32, u32, u32, bool, bool, extern "C" fn(*const c_char)) -> c_int;
type OverlayRequestStopFn = unsafe extern "C" fn();

/// The private C API library kind that failed to load.
///
/// Only the region-overlay library is loaded via `dlopen` by this daemon
/// (the region *selector* is now spawned as a subprocess — see
/// `selector_proc` — so it has no in-process `dlopen` path here).
#[derive(Debug, Clone, Copy)]
pub enum LibKind {
    Overlay,
}

impl LibKind {
    fn label(&self) -> &'static str {
        match self {
            LibKind::Overlay => "region-overlay",
        }
    }

    fn summary(&self) -> &'static str {
        match self {
            LibKind::Overlay => "Region overlay library missing or broken",
        }
    }

    fn env_var(&self) -> &'static str {
        match self {
            LibKind::Overlay => "SCREENSHOT_DAEMON_OVERLAY_CAPI",
        }
    }
}

/// Error returned when a private C API library (`.so`) cannot be loaded or
/// a required symbol cannot be resolved.
///
/// Constructing this error via [`LibLoadError::new`] fires a clear D-Bus
/// notification (fire-and-forget on the current tokio runtime) so the user
/// is told *which* library failed and *why*, instead of the error being
/// buried in the log. The error is then wrapped into `anyhow::Error` and
/// propagated through the normal call chain; the main loop recognises it
/// via `downcast_ref` and suppresses its own generic "Screenshot failed"
/// notification to avoid duplicates.
#[derive(Debug)]
pub struct LibLoadError {
    kind: LibKind,
    path: PathBuf,
    error: String,
}

impl fmt::Display for LibLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "load {} capi {} failed: {}",
            self.kind.label(),
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for LibLoadError {}

impl LibLoadError {
    /// Build a `LibLoadError`, fire a clear D-Bus notification, and wrap it
    /// into `anyhow::Error` for return through the existing call chain.
    pub fn new(kind: LibKind, path: PathBuf, error: impl fmt::Display) -> anyhow::Error {
        let err = LibLoadError {
            kind,
            path,
            error: error.to_string(),
        };
        err.fire_notification();
        anyhow::Error::new(err)
    }

    fn fire_notification(&self) {
        let summary = self.kind.summary().to_string();
        let body = format!(
            "Could not load the {} C library.\n\n\
             Expected path: {}\n\
             Error: {}\n\n\
             Reinstall screenshot-tool, or set {} to a valid path.",
            self.kind.label(),
            self.path.display(),
            self.error,
            self.kind.env_var(),
        );
        log::error!("{} | {}", summary, body);
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    if let Err(notify_err) =
                        crate::notify::send_notification(&summary, &body).await
                    {
                        log::warn!("lib-load D-Bus notification failed: {notify_err}");
                    }
                });
            }
            Err(_) => log::error!(
                "no tokio runtime available; cannot fire D-Bus notification for {} capi failure",
                self.kind.label()
            ),
        }
    }
}

static OVERLAY_SENDER: OnceLock<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>> =
    OnceLock::new();

extern "C" fn overlay_callback(message: *const c_char) {
    if let Some(message) = read_message(message) {
        let slot = OVERLAY_SENDER.get_or_init(|| Mutex::new(None));
        if let Ok(slot) = slot.lock() {
            if let Some(sender) = slot.as_ref() {
                let _ = sender.send(message);
            }
        }
    }
}

pub fn run_record_overlay(
    region: (i32, i32, u32, u32),
    controlbar_draggable: bool,
    sender: tokio::sync::mpsc::UnboundedSender<String>,
) -> anyhow::Result<()> {
    let lib_path = overlay_capi_path();
    let slot = OVERLAY_SENDER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(sender);
    }

    let status = unsafe {
        let library = libloading::Library::new(&lib_path)
            .map_err(|error| LibLoadError::new(LibKind::Overlay, lib_path.clone(), error))?;
        match library.get::<OverlayRunWithConfigFn>(b"region_overlay_run_with_config") {
            Ok(run) => run(
                region.0,
                region.1,
                region.2,
                region.3,
                false,
                controlbar_draggable,
                overlay_callback,
            ),
            Err(_) => {
                // Soft version-compat fallback: the modern `with_config`
                // symbol is optional. Only the legacy `region_overlay_run`
                // is a hard requirement — its absence is a real lib error.
                let run: libloading::Symbol<OverlayRunFn> = library
                    .get(b"region_overlay_run")
                    .map_err(|error| LibLoadError::new(LibKind::Overlay, lib_path.clone(), error))?;
                run(
                    region.0,
                    region.1,
                    region.2,
                    region.3,
                    false,
                    overlay_callback,
                )
            }
        }
    };

    if let Ok(mut slot) = slot.lock() {
        *slot = None;
    }

    if status != 0 {
        anyhow::bail!("overlay capi returned status {status}");
    }

    Ok(())
}

pub fn request_record_overlay_stop() -> anyhow::Result<()> {
    let lib_path = overlay_capi_path();
    unsafe {
        let library = libloading::Library::new(&lib_path)
            .map_err(|error| LibLoadError::new(LibKind::Overlay, lib_path.clone(), error))?;
        let stop: libloading::Symbol<OverlayRequestStopFn> = library
            .get(b"region_overlay_request_stop")
            .map_err(|error| LibLoadError::new(LibKind::Overlay, lib_path.clone(), error))?;
        stop();
    }
    Ok(())
}

fn overlay_capi_path() -> PathBuf {
    std::env::var_os("SCREENSHOT_DAEMON_OVERLAY_CAPI")
        .map(PathBuf::from)
        .unwrap_or_else(|| current_exe_dir().join("libregion_overlay_capi.so"))
}

fn current_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(ToOwned::to_owned))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_message(message: *const c_char) -> Option<String> {
    if message.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(message) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}
