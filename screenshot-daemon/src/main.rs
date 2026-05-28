use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

mod capture;
mod clipboard;
mod deps;
mod detect;
mod hotkey;
mod notify;
mod recorder;

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
        hotkey::Hotkey::parse("fullscreen", "Ctrl+Shift+P")?,
        hotkey::Hotkey::parse("region", "Ctrl+Alt+A")?,
        hotkey::Hotkey::parse("record", "Ctrl+Alt+R")?,
    ];
    for hk in &hotkeys {
        log::info!("hotkey: {} → {:?}", hk.label, hk);
    }

    let config = Config { save_dir };

    let shared_recorder: recorder::SharedRecorder = Arc::new(Mutex::new(recorder::Recorder::new(&display_server)));

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
                            match rec.start().await {
                                Ok(()) => {
                                    log::info!("recording started");
                                    let output_path = rec.output_path().to_string_lossy().to_string();
                                    drop(rec);
                                    spawn_record_control(&output_path, shared_recorder.clone());
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

fn spawn_record_control(
    _output_path: &str,
    shared_recorder: recorder::SharedRecorder,
) {
    let self_exe = std::env::current_exe().ok();
    let dir = self_exe
        .as_ref()
        .and_then(|e| e.parent())
        .unwrap_or_else(|| std::path::Path::new("."));
    let control_bin = dir.join("record-control");

    let recorder_clone = shared_recorder.clone();
    tokio::spawn(async move {
        let mut child = match tokio::process::Command::new(&control_bin)
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                log::error!("failed to spawn record-control: {e}");
                return;
            }
        };

        if let Some(pid) = child.id() {
            log::info!("record-control spawned (pid {})", pid);
            let mut rec = recorder_clone.lock().await;
            rec.set_control_pid(pid);
            drop(rec);
        }

        let stdout = child.stdout.take();
        if let Some(out) = stdout {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let reader = BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match line.as_str() {
                    "stopped" => {
                        log::info!("record-control reported stop");
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
                        log::info!("record-control reported pause");
                        let mut rec = recorder_clone.lock().await;
                        if let Err(e) = rec.toggle_pause().await {
                            log::error!("pause recording failed: {e}");
                        }
                    }
                    "resumed" => {
                        log::info!("record-control reported resume");
                        let mut rec = recorder_clone.lock().await;
                        if let Err(e) = rec.toggle_pause().await {
                            log::error!("resume recording failed: {e}");
                        }
                    }
                    "cancelled" => {
                        log::info!("record-control cancelled");
                    }
                    other => {
                        log::debug!("record-control output: {}", other);
                    }
                }
            }
        }

        let _ = child.wait().await;
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
