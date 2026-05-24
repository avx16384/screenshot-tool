/// Clipboard support — copy image to system clipboard.
/// On Wayland, the daemon has no compositor connection, so we use `wl-copy`.
/// On X11, we use `arboard` directly.
use std::path::Path;

pub async fn copy_image_to_clipboard(
    path: &Path,
    display_server: &crate::detect::DisplayServer,
) -> anyhow::Result<()> {
    match display_server {
        crate::detect::DisplayServer::Wayland | crate::detect::DisplayServer::Unknown => {
            // On Wayland the daemon has no compositor connection — use wl-copy subprocess
            let status = tokio::process::Command::new("wl-copy")
                .arg("-t")
                .arg("image/png")
                .arg(path)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("wl-copy failed with status {}", status);
            }
        }
        crate::detect::DisplayServer::X11 => {
            let img = image::open(path)?;
            let rgba = img.to_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            let data = rgba.as_raw().clone();
            let mut clipboard = arboard::Clipboard::new()?;
            let img_data = arboard::ImageData {
                width: w as usize,
                height: h as usize,
                bytes: data.into(),
            };
            clipboard.set_image(img_data)?;
        }
    }
    Ok(())
}
