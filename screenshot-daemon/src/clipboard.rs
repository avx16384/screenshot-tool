use std::path::Path;

pub async fn copy_image_to_clipboard(
    path: &Path,
    _display_server: &crate::detect::DisplayServer,
) -> anyhow::Result<()> {
    let img = image::open(path)?;
    let rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    let data = rgba.as_raw().clone();

    tokio::task::spawn_blocking(move || {
        let mut clipboard = arboard::Clipboard::new()?;
        let img_data = arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: data.into(),
        };
        clipboard.set_image(img_data)?;
        Ok::<(), anyhow::Error>(())
    })
    .await??;

    Ok(())
}
