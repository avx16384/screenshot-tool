use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

mod capi_runtime;
mod capture;
mod clipboard;
mod config;
mod deps;
mod detect;
mod hotkey;
mod naming;
mod notify;
mod recorder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = config::Config::load()?;
    std::fs::create_dir_all(&config.save_dir)?;

    log::info!("screenshot-daemon starting");
    log::info!("save dir: {}", config.save_dir.display());
    log::info!("controlbar draggable: {}", config.controlbar_draggable);

    let display_server = detect::detect_display_server();
    log::info!("display server: {:?}", display_server);

    let dep_results = deps::check_dependencies();
    let dep_report = deps::format_dep_report(&dep_results);
    for line in dep_report.lines() {
        log::info!("{}", line);
    }

    if deps::has_missing_required(&dep_results) {
        log::warn!("missing required dependencies!");
        spawn_deps_dialog(&dep_report);
    }

    let hotkeys = vec![
        hotkey::Hotkey::parse("fullscreen", "Print")?,
        hotkey::Hotkey::parse("region", "Ctrl+Alt+A")?,
        hotkey::Hotkey::parse("record", "Ctrl+Alt+R")?,
    ];
    for hk in &hotkeys {
        log::info!("hotkey: {} → {:?}", hk.label, hk);
    }

    let shared_recorder: recorder::SharedRecorder =
        Arc::new(Mutex::new(recorder::Recorder::new(&display_server)));

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
                    hotkey::HotkeyAction::Record => {
                        log::info!("record video triggered");
                        let mut rec = shared_recorder.lock().await;
                        if rec.is_recording() {
                            log::info!("already recording, stopping");
                            if let Err(e) = capi_runtime::request_record_overlay_stop() {
                                log::warn!("failed to request record overlay stop: {e}");
                            }
                            match rec.stop().await {
                                Ok(path) => {
                                    log::info!("recording saved: {}", path.display());
                                    if let Err(e) = notify::send_notification_with_open(
                                        "Recording saved",
                                        &format!("Saved to {}", path.display()),
                                        Some(path.to_string_lossy().as_ref()),
                                    ).await {
                                        log::warn!("notification failed: {e}");
                                    }
                                }
                                Err(e) => {
                                    log::error!("stop recording failed: {e}");
                                }
                            }
                        } else {
                            drop(rec);
                            match capture::select_record_region(&display_server).await {
                                Ok(Some(region)) => {
                                    let mut rec = shared_recorder.lock().await;
                                    rec.set_region(region);
                                    match rec.start().await {
                                        Ok(()) => {
                                            log::info!("recording started");
                                            let output_path = rec.output_path().to_string_lossy().to_string();
                                            let region = rec.region();
                                            drop(rec);
                                            spawn_record_overlay(
                                                &output_path,
                                                region,
                                                shared_recorder.clone(),
                                                config.controlbar_draggable,
                                            );
                                        }
                                        Err(e) => {
                                            log::error!("start recording failed: {e}");
                                            if let Err(ne) = notify::send_notification(
                                                "Recording failed",
                                                &e.to_string(),
                                            ).await {
                                                log::warn!("notification failed: {ne}");
                                            }
                                        }
                                    }
                                }
                                Ok(None) => {
                                    log::info!("record region selection cancelled");
                                }
                                Err(e) => {
                                    log::error!("record region selection failed: {e}");
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
    let path = crate::naming::unique_path(save_dir, "screenshot", "png");

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
    let path = crate::naming::unique_path(save_dir, "region", "png");

    // region selector C API works on both X11 and Wayland
    match capture::capture_region(&path, _display_server).await? {
        Some(()) => Ok(Some(path)),
        None => Ok(None),
    }
}

fn spawn_record_overlay(
    _output_path: &str,
    region: Option<(i32, i32, u32, u32)>,
    shared_recorder: recorder::SharedRecorder,
    controlbar_draggable: bool,
) {
    let recorder_clone = shared_recorder.clone();
    tokio::spawn(async move {
        let region = match region {
            Some((rx, ry, rw, rh)) => (rx, ry, rw, rh),
            None => {
                let (sw, sh) = detect_screen_size_sync(&crate::detect::DisplayServer::Wayland);
                (0, 0, sw, sh)
            }
        };

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut overlay_task = tokio::task::spawn_blocking(move || {
            crate::capi_runtime::run_record_overlay(region, controlbar_draggable, sender)
        });

        loop {
            tokio::select! {
                Some(line) = receiver.recv() => {
                    match line.as_str() {
                    "stopped" => {
                        log::info!("record-overlay reported stop");
                        let mut rec = recorder_clone.lock().await;
                        if rec.is_recording() {
                            match rec.stop().await {
                                Ok(path) => {
                                    log::info!("recording saved: {}", path.display());
                                    if let Err(e) = notify::send_notification_with_open(
                                        "Recording saved",
                                        &format!("Saved to {}", path.display()),
                                        Some(path.to_string_lossy().as_ref()),
                                    ).await {
                                        log::warn!("notification failed: {e}");
                                    }
                                }
                                Err(e) => {
                                    log::error!("stop recording failed: {e}");
                                }
                            }
                        }
                    }
                    "paused" => {
                        log::info!("record-overlay reported pause");
                        let mut rec = recorder_clone.lock().await;
                        if let Err(e) = rec.toggle_pause().await {
                            log::error!("pause recording failed: {e}");
                        }
                    }
                    "resumed" => {
                        log::info!("record-overlay reported resume");
                        let mut rec = recorder_clone.lock().await;
                        if let Err(e) = rec.toggle_pause().await {
                            log::error!("resume recording failed: {e}");
                        }
                    }
                    other => {
                        log::debug!("record-overlay output: {}", other);
                    }
                    }
                },
                result = &mut overlay_task => {
                    match result {
                        Ok(Ok(())) => log::info!("record overlay capi exited"),
                        Ok(Err(e)) => log::error!("record overlay capi failed: {e}"),
                        Err(e) => log::error!("record overlay task failed: {e}"),
                    }
                    break;
                },
            }
        }
    });
}

fn spawn_deps_dialog(report: &str) {
    let self_exe = std::env::current_exe().ok();
    let dir = self_exe
        .as_ref()
        .and_then(|e| e.parent())
        .unwrap_or_else(|| std::path::Path::new("."));
    let dialog_bin = dir.join("deps-dialog");

    let report = report.to_string();
    tokio::spawn(async move {
        match tokio::process::Command::new(&dialog_bin)
            .arg(&report)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                let _ = child.wait().await;
            }
            Err(e) => {
                log::error!("failed to spawn deps-dialog: {e}");
            }
        }
    });
}

fn detect_screen_size_sync(display_server: &detect::DisplayServer) -> (u32, u32) {
    match display_server {
        detect::DisplayServer::X11 => {
            use x11rb::connection::Connection;
            if let Ok((conn, screen_num)) = x11rb::rust_connection::RustConnection::connect(None) {
                let screen = &conn.setup().roots[screen_num];
                return (
                    screen.width_in_pixels as u32,
                    screen.height_in_pixels as u32,
                );
            }
        }
        detect::DisplayServer::Wayland | detect::DisplayServer::Unknown => {
            if let Ok(conn) = libwayshot::WayshotConnection::new() {
                if let Ok(img) = conn.screenshot_all(false) {
                    return (img.width() as u32, img.height() as u32);
                }
            }
        }
    }
    (1920, 1080)
}
