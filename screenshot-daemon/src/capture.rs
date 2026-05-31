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

    let path = path.to_path_buf();
    let bg_arg = has_bg.then(|| bg_path.clone());
    let selector_result = tokio::task::spawn_blocking(move || {
        crate::capi_runtime::run_region_selector(Some(&path), bg_arg.as_deref(), false)
    })
    .await
    .map_err(|error| anyhow::anyhow!("selector capi task failed: {error}"))??;
    let _ = std::fs::remove_file(&bg_path);

    let Some(stdout) = selector_result else {
        return Ok(None);
    };

    if stdout == "cancelled" {
        return Ok(None);
    }
    if stdout.starts_with("saved:") {
        return Ok(Some(()));
    }

    anyhow::bail!("unexpected region-selector output: {}", stdout);
}

pub async fn capture_wayland(path: &Path) -> anyhow::Result<()> {
    let img = capture_wayland_image()?;
    img.save(path)?;
    Ok(())
}

pub async fn select_record_region(
    display_server: &crate::detect::DisplayServer,
) -> anyhow::Result<Option<Option<(i32, i32, u32, u32)>>> {
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

    let bg_arg = has_bg.then(|| bg_path.clone());
    let selector_result = tokio::task::spawn_blocking(move || {
        crate::capi_runtime::run_region_selector(None, bg_arg.as_deref(), true)
    })
    .await
    .map_err(|error| anyhow::anyhow!("selector capi task failed: {error}"))??;
    let _ = std::fs::remove_file(&bg_path);

    let Some(stdout) = selector_result else {
        return Ok(None);
    };

    if stdout == "cancelled" {
        return Ok(None);
    }

    if stdout == "fullscreen" {
        return Ok(Some(None));
    }

    if stdout.starts_with("region:") {
        let parts: Vec<&str> = stdout[7..].split(',').collect();
        if parts.len() >= 4 {
            let x: i32 = parts[0].parse()?;
            let y: i32 = parts[1].parse()?;
            let w: u32 = parts[2].parse()?;
            let h: u32 = parts[3].parse()?;
            return Ok(Some(Some((x, y, w, h))));
        }
    }

    anyhow::bail!("unexpected region-selector output: {}", stdout)
}
