/// Detect which display server is running (X11, Wayland, or Unknown).
use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

pub fn detect_display_server() -> DisplayServer {
    let session = env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase();

    match session.as_str() {
        "x11" => DisplayServer::X11,
        "wayland" => DisplayServer::Wayland,
        _ => {
            // Fallback: check WAYLAND_DISPLAY / DISPLAY env vars
            if env::var("WAYLAND_DISPLAY").is_ok() {
                DisplayServer::Wayland
            } else if env::var("DISPLAY").is_ok() {
                DisplayServer::X11
            } else {
                DisplayServer::Unknown
            }
        }
    }
}
