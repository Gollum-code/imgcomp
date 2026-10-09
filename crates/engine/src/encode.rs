use crate::format::Format;
use anyhow::{bail, Context, Result};
use image::RgbaImage;
use std::io::Cursor;

/// Encode an RGBA image into the given format at the given quality (1-100).
///
/// - JPEG / WebP / AVIF: lossy, quality maps directly to the encoder.
/// - PNG: lossless at quality >= 90, otherwise colors are quantized (fewer
///   bits per channel) to actually shrink the file, then deflate-optimized.
pub fn encode(format: Format, image: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    let quality = quality.clamp(1, 100);
    match format {
        Format::Jpeg => encode_jpeg(image, quality),
        Format::Png => encode_png(image, quality),
        Format::WebP => encode_webp(image, quality),
        Format::Avif => encode_avif(image, quality),
    }
}

fn encode_jpeg(image: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let raw = image.as_raw();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_EXT_RGBA);
        comp.set_size(w, h);
        comp.set_scan_optimization_mode(mozjpeg::ScanMode::Auto);
        comp.set_quality(quality as f32);
        comp.set_progressive_mode();
        let mut started = comp.start_compress(Cursor::new(Vec::new()))?;
        started.write_scanlines(raw)?;
        started.finish()
    }))
    .map_err(|_| anyhow::anyhow!("mozjpeg panicked while compressing"))??;
    Ok(result.into_inner())
}

fn encode_png(image: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    let work = if quality >= 90 {
        image.clone()
    } else {
        quantize(image, quality)
    };
    let mut cursor = Cursor::new(Vec::new());
    work.write_to(&mut cursor, image::ImageFormat::Png)
        .context("cannot write PNG")?;
    let png_bytes = cursor.into_inner();
    let opts = if quality >= 90 {
        oxipng::Options::max_compression()
    } else {
        oxipng::Options::from_preset(4)
    };
    oxipng::optimize_from_memory(&png_bytes, &opts).context("oxipng optimization failed")
}

fn encode_webp(image: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    let encoder = webp::Encoder::from_rgba(image.as_raw(), image.width(), image.height());
    let memory = encoder.encode(quality as f32);
    Ok(memory.to_vec())
}

fn encode_avif(image: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let pixels: Vec<rgb::RGBA<u8>> = image
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| rgb::RGBA::new(c[0], c[1], c[2], c[3]))
        .collect();
    let imgref_img = imgref::Img::new(pixels.as_slice(), w, h);
    let encoder = ravif::Encoder::new()
        .with_quality(quality as f32)
        .with_speed(6);
    let result = encoder
        .encode_rgba(imgref_img)
        .map_err(|e| anyhow::anyhow!("ravif encode failed: {e}"))?;
    Ok(result.avif_file)
}

/// Destroy low bits of each color channel so the PNG shrinks with loss.
/// quality 90+ = untouched; 75-89 = 6 bits; 55-74 = 5 bits; below = 4 bits.
fn quantize(image: &RgbaImage, quality: u8) -> RgbaImage {
    let bits: u32 = if quality >= 75 {
        6
    } else if quality >= 55 {
        5
    } else {
        4
    };
    let levels = ((1u16 << bits) - 1) as f32; // 63 / 31 / 15
    RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let mut out = [0u8; 4];
        for c in 0..3 {
            let q = (p[c] as f32 * levels / 255.0).round();
            out[c] = (q * 255.0 / levels).round() as u8;
        }
        out[3] = p[3];
        out.into()
    })
}

/// Quality 1-100 -> PNG banding control (used by CLI help/tests).
pub fn _png_bits(quality: u8) -> u32 {
    match quality {
        90..=100 => 8,
        75..=89 => 6,
        55..=74 => 5,
        _ => 4,
    }
}

/// Guard against zero-sized images which crash C encoders.
pub fn check_dimensions(image: &RgbaImage) -> Result<()> {
    if image.width() == 0 || image.height() == 0 {
        bail!("image has zero dimensions");
    }
    Ok(())
}
