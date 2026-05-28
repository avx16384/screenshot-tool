use std::path::Path;

pub struct RawPixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

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

pub fn capture_wayland_image() -> anyhow::Result<image::RgbaImage> {
    let conn = libwayshot::WayshotConnection::new()?;
    let img = conn.screenshot_all(false)?;
    Ok(img.to_rgba8())
}

pub async fn capture_region(
    path: &Path,
    display_server: &crate::detect::DisplayServer,
) -> anyhow::Result<Option<()>> {
    let self_exe = std::env::current_exe()?;
    let dir = self_exe
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let selector = dir.join("region-selector");

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
                    log::warn!("Wayland native capture failed: {e}");
                    false
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

pub async fn capture_wayland(path: &Path) -> anyhow::Result<()> {
    let img = capture_wayland_image()?;
    img.save(path)?;
    Ok(())
}
