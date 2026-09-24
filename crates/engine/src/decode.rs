use anyhow::{Context, Result};
use image::RgbaImage;
use std::path::Path;

/// Decode any supported image (jpg/png/webp) into an RGBA buffer.
pub fn load(path: &Path) -> Result<RgbaImage> {
    let reader = image::ImageReader::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?
        .with_guessed_format()
        .context("cannot guess image format")?;
    let img = reader
        .decode()
        .with_context(|| format!("cannot decode {}", path.display()))?;
    Ok(img.to_rgba8())
}

/// Decode raw encoded bytes (jpg/png/webp) into an RGBA buffer.
pub fn load_from_memory(bytes: &[u8]) -> Result<RgbaImage> {
    let img = image::load_from_memory(bytes).context("cannot decode encoded image")?;
    Ok(img.to_rgba8())
}
