use std::path::{Path, PathBuf};
use std::sync::Arc;

mod capture;
mod clipboard;
mod detect;
mod hotkey;
mod notify;

/// Configuration
struct Config {
    save_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let save_dir = dirs::picture_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("screenshots");
    std::fs::create_dir_all(&save_dir)?;

    log::info!("screenshot-daemon starting");
    log::info!("save dir: {}", save_dir.display());

    let display_server = detect::detect_display_server();
    log::info!("display server: {:?}", display_server);

    let hotkeys = vec![
        hotkey::Hotkey::parse("fullscreen", "Ctrl+Shift+P")?,
        hotkey::Hotkey::parse("region", "Ctrl+Alt+A")?,
    ];
    for hk in &hotkeys {
        log::info!("hotkey: {} → {:?}", hk.label, hk);
    }

    let config = Config { save_dir };

    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        log::info!("shutting down");
        shutdown_clone.notify_one();
    });

    // mpsc channel for hotkey events (each event consumed once)
    let (tx, mut rx) = tokio::sync::mpsc::channel::<hotkey::HotkeyAction>(4);

    let hotkeys_clone = hotkeys.clone();
    std::thread::spawn(move || {
        if let Err(e) = hotkey::listen(&hotkeys_clone, &tx) {
            log::error!("hotkey listener failed: {e}");
        }
    });

    // Main loop
    loop {
        tokio::select! {
            Some(action) = rx.recv() => {
                match action {
                    hotkey::HotkeyAction::Fullscreen => {
                        log::info!("fullscreen capture triggered");
                        match capture_screenshot(&config.save_dir, &display_server).await {
                            Ok(path) => {
                                log::info!("saved: {}", path.display());
                                if let Err(e) = clipboard::copy_image_to_clipboard(&path, &display_server).await {
                                    log::warn!("clipboard copy failed: {e}");
                                }
                                if let Err(e) = notify::send_notification_with_open(
                                    "Screenshot saved",
                                    &format!("Saved to {} (copied to clipboard)", path.display()),
                                    Some(path.to_string_lossy().as_ref()),
                                ).await {
                                    log::warn!("notification failed: {e}");
                                }
                            }
                            Err(e) => {
                                log::error!("capture failed: {e}");
                                if let Err(ne) = notify::send_notification(
                                    "Screenshot failed",
                                    &e.to_string(),
                                ).await {
                                    log::warn!("notification failed: {ne}");
                                }
                            }
                        }
                    }
                    hotkey::HotkeyAction::Region => {
                        log::info!("region capture triggered");
                        match capture_region_screenshot(&config.save_dir, &display_server).await {
                            Ok(Some(path)) => {
                                log::info!("saved: {}", path.display());
                                if let Err(e) = notify::send_notification_with_open(
                                    "Screenshot saved",
                                    &format!("Region saved to {} (copied to clipboard)", path.display()),
                                    Some(path.to_string_lossy().as_ref()),
                                ).await {
                                    log::warn!("notification failed: {e}");
                                }
                            }
                            Ok(None) => {
                                log::info!("region selection cancelled");
                            }
                            Err(e) => {
                                log::error!("region capture failed: {e}");
                                if let Err(ne) = notify::send_notification(
                                    "Screenshot failed",
                                    &e.to_string(),
                                ).await {
                                    log::warn!("notification failed: {ne}");
                                }
                            }
                        }
                    }
                }
            }
            _ = shutdown.notified() => {
                log::info!("exiting");
                break;
            }
        }
    }

    Ok(())
}

async fn capture_screenshot(
    save_dir: &Path,
    display_server: &detect::DisplayServer,
) -> anyhow::Result<PathBuf> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("screenshot_{}.png", timestamp);
    let path = save_dir.join(&filename);

    match display_server {
        detect::DisplayServer::X11 => {
            let pixels = capture::capture_x11()?;
            let img = image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.data)
                .ok_or_else(|| anyhow::anyhow!("failed to create image from raw pixels"))?;
            img.save(&path)?;
        }
        detect::DisplayServer::Wayland => {
            capture::capture_wayland(&path).await?;
        }
        detect::DisplayServer::Unknown => match capture::capture_x11() {
            Ok(pixels) => {
                let img = image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.data)
                    .ok_or_else(|| anyhow::anyhow!("failed to create image from raw pixels"))?;
                img.save(&path)?;
            }
            Err(_) => {
                capture::capture_wayland(&path).await?;
            }
        },
    }

    Ok(path)
}

async fn capture_region_screenshot(
    save_dir: &Path,
    _display_server: &detect::DisplayServer,
) -> anyhow::Result<Option<PathBuf>> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("region_{}.png", timestamp);
    let path = save_dir.join(&filename);

    // region-selector (egui/eframe) works on both X11 and Wayland
    match capture::capture_region(&path, _display_server).await? {
        Some(()) => Ok(Some(path)),
        None => Ok(None),
    }
}
