/// Screenshot capture for X11 and Wayland.
/// Supports fullscreen and region selection.
use std::path::Path;

pub struct RawPixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

// ── X11 Fullscreen ────────────────────────────────────────────────────

pub fn capture_x11() -> anyhow::Result<RawPixels> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    let width = screen.width_in_pixels as u32;
    let height = screen.height_in_pixels as u32;

    let reply = get_image(
        &conn,
        ImageFormat::Z_PIXMAP,
        screen.root,
        0,
        0,
        width as u16,
        height as u16,
        u32::MAX,
    )?
    .reply()?;

    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for chunk in reply.data.chunks_exact(4) {
        rgba.push(chunk[2]);
        rgba.push(chunk[1]);
        rgba.push(chunk[0]);
        rgba.push(chunk[3]);
    }

    Ok(RawPixels {
        width,
        height,
        data: rgba,
    })
}

// ── Wayland Fullscreen (native wlr-screencopy via libwayshot) ────────

pub fn capture_wayland_image() -> anyhow::Result<image::RgbaImage> {
    let conn = libwayshot::WayshotConnection::new()?;
    let img = conn.screenshot_all(false)?;
    Ok(img.to_rgba8())
}

// ── Region (spawn region-selector subprocess) ────────────────────────

/// Capture region: pre-capture fullscreen as background, then spawn region-selector.
/// On Wayland, uses native wlr-screencopy API; on X11, region-selector captures itself.
pub async fn capture_region(
    path: &Path,
    display_server: &crate::detect::DisplayServer,
) -> anyhow::Result<Option<()>> {
    let self_exe = std::env::current_exe()?;
    let dir = self_exe
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let selector = dir.join("region-selector");

    // On Wayland, capture fullscreen BEFORE spawning the overlay window
    let bg_path = std::env::temp_dir().join("screenshot-daemon-bg.png");
    let _ = std::fs::remove_file(&bg_path);
    let has_bg = match display_server {
        crate::detect::DisplayServer::Wayland | crate::detect::DisplayServer::Unknown => {
            match capture_wayland_image() {
                Ok(img) => {
                    img.save(&bg_path)?;
                    true
                }
                Err(e) => {
                    log::warn!(
                        "Wayland native capture failed ({}), falling back to grim",
                        e
                    );
                    let status = tokio::process::Command::new("grim")
                        .arg(&bg_path)
                        .status()
                        .await?;
                    status.success() && bg_path.exists()
                }
            }
        }
        crate::detect::DisplayServer::X11 => false,
    };

    let mut cmd = tokio::process::Command::new(&selector);
    cmd.arg("--output").arg(path);
    if has_bg {
        cmd.arg("--background").arg(&bg_path);
    }

    let output = cmd.output().await?;
    let _ = std::fs::remove_file(&bg_path);

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout == "cancelled" {
        return Ok(None);
    }

    if stdout.starts_with("saved:") {
        return Ok(Some(()));
    }

    if !output.status.success() {
        anyhow::bail!(
            "region-selector failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    anyhow::bail!("unexpected region-selector output: {}", stdout);
}

// ── Wayland Fullscreen save to file ──────────────────────────────────

pub async fn capture_wayland(path: &Path) -> anyhow::Result<()> {
    // Try native API first, fall back to grim
    match capture_wayland_image() {
        Ok(img) => {
            img.save(path)?;
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "Wayland native capture failed ({}), falling back to grim",
                e
            );
            let path_str = path.to_string_lossy().to_string();
            let status = tokio::process::Command::new("grim")
                .arg(&path_str)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("grim exited with status {}", status);
            }
            Ok(())
        }
    }
}
