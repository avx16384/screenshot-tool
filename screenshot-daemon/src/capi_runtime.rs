use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

type SelectorRunFn =
    unsafe extern "C" fn(*const c_char, *const c_char, bool, extern "C" fn(*const c_char)) -> c_int;
type OverlayRunFn =
    unsafe extern "C" fn(i32, i32, u32, u32, bool, extern "C" fn(*const c_char)) -> c_int;
type OverlayRunWithConfigFn =
    unsafe extern "C" fn(i32, i32, u32, u32, bool, bool, extern "C" fn(*const c_char)) -> c_int;
type OverlayRequestStopFn = unsafe extern "C" fn();

static SELECTOR_RESULT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static OVERLAY_SENDER: OnceLock<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>> =
    OnceLock::new();

extern "C" fn selector_callback(message: *const c_char) {
    if let Some(message) = read_message(message) {
        let slot = SELECTOR_RESULT.get_or_init(|| Mutex::new(None));
        if let Ok(mut slot) = slot.lock() {
            *slot = Some(message);
        }
    }
}

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

pub fn run_region_selector(
    output_path: Option<&std::path::Path>,
    background_path: Option<&std::path::Path>,
    record_mode: bool,
) -> anyhow::Result<Option<String>> {
    let lib_path = selector_capi_path();
    let output = path_to_cstring(output_path)?;
    let background = path_to_cstring(background_path)?;

    let slot = SELECTOR_RESULT.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = None;
    }

    let status = unsafe {
        let library = libloading::Library::new(&lib_path).map_err(|error| {
            anyhow::anyhow!("load selector capi {} failed: {error}", lib_path.display())
        })?;
        let run: libloading::Symbol<SelectorRunFn> = library
            .get(b"screenshot_region_selector_run")
            .map_err(|error| anyhow::anyhow!("load selector symbol failed: {error}"))?;
        run(
            output
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            background
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            record_mode,
            selector_callback,
        )
    };

    if status != 0 {
        anyhow::bail!("selector capi returned status {status}");
    }

    let slot = SELECTOR_RESULT.get_or_init(|| Mutex::new(None));
    Ok(slot.lock().ok().and_then(|mut slot| slot.take()))
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
        let library = libloading::Library::new(&lib_path).map_err(|error| {
            anyhow::anyhow!("load overlay capi {} failed: {error}", lib_path.display())
        })?;
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
                let run: libloading::Symbol<OverlayRunFn> = library
                    .get(b"region_overlay_run")
                    .map_err(|error| anyhow::anyhow!("load overlay symbol failed: {error}"))?;
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
        let library = libloading::Library::new(&lib_path).map_err(|error| {
            anyhow::anyhow!("load overlay capi {} failed: {error}", lib_path.display())
        })?;
        let stop: libloading::Symbol<OverlayRequestStopFn> = library
            .get(b"region_overlay_request_stop")
            .map_err(|error| anyhow::anyhow!("load overlay stop symbol failed: {error}"))?;
        stop();
    }
    Ok(())
}

fn selector_capi_path() -> PathBuf {
    std::env::var_os("SCREENSHOT_DAEMON_SELECTOR_CAPI")
        .map(PathBuf::from)
        .unwrap_or_else(|| current_exe_dir().join("libregion_selector.so"))
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

fn path_to_cstring(path: Option<&std::path::Path>) -> anyhow::Result<Option<CString>> {
    path.map(|path| {
        CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| anyhow::anyhow!("path contains nul byte: {}", path.display()))
    })
    .transpose()
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
