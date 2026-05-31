use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

#[path = "region_selector.rs"]
mod region_selector;

pub type ScreenshotSelectorCallback = extern "C" fn(*const c_char);

#[no_mangle]
pub extern "C" fn screenshot_region_selector_run(
    output_path: *const c_char,
    background_path: *const c_char,
    record_mode: bool,
    callback: ScreenshotSelectorCallback,
) -> c_int {
    let output_path = optional_c_string(output_path);
    let background_path = optional_c_string(background_path);

    let result = region_selector::run_region_selector(region_selector::RegionSelectorOptions {
        output_path,
        background_path,
        record_mode,
    });

    match result {
        Ok(region_selector::RegionSelectorOutcome::Cancelled) => {
            send_callback(callback, "cancelled");
            0
        }
        Ok(region_selector::RegionSelectorOutcome::Saved(path)) => {
            send_callback(callback, &format!("saved:{path}"));
            0
        }
        Ok(region_selector::RegionSelectorOutcome::Fullscreen) => {
            send_callback(callback, "fullscreen");
            0
        }
        Ok(region_selector::RegionSelectorOutcome::Region(x, y, w, h)) => {
            send_callback(callback, &format!("region:{x},{y},{w},{h}"));
            0
        }
        Ok(region_selector::RegionSelectorOutcome::Noop) => 0,
        Err(error) => {
            eprintln!("screenshot-region-selector error: {error}");
            1
        }
    }
}

fn optional_c_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn send_callback(callback: ScreenshotSelectorCallback, message: &str) {
    if let Ok(message) = CString::new(message) {
        callback(message.as_ptr());
    }
}
