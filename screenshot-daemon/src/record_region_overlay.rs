use std::ffi::CStr;
use std::os::raw::c_char;

extern "C" fn print_overlay_event(message: *const c_char) {
    if message.is_null() {
        return;
    }

    let message = unsafe { CStr::from_ptr(message) };
    if let Ok(message) = message.to_str() {
        println!("{message}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut debug = false;
    let mut region_args = Vec::new();

    for arg in &args[1..] {
        if arg == "--debug" {
            debug = true;
        } else {
            region_args.push(arg.clone());
        }
    }

    let (x, y, width, height) = if region_args.len() >= 4 {
        (
            region_args[0].parse::<i32>().unwrap_or(0),
            region_args[1].parse::<i32>().unwrap_or(0),
            region_args[2].parse::<u32>().unwrap_or(1920),
            region_args[3].parse::<u32>().unwrap_or(1080),
        )
    } else if debug {
        (100, 100, 800, 600)
    } else {
        eprintln!("Usage: record-region-overlay [--debug] <x> <y> <width> <height>");
        std::process::exit(1);
    };

    let status =
        region_overlay_capi::region_overlay_run(x, y, width, height, debug, print_overlay_event);

    std::process::exit(status);
}
