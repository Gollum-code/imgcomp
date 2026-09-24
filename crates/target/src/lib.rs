use anyhow::Result;
use dssim::Dssim;
use engine::{encode as encoder, Format, RgbaImage};
use image::imageops::FilterType;

pub const DEFAULT_SSIM_THRESHOLD: f32 = 0.08;
const SSIM_MAX_EDGE: u32 = 1024;

#[derive(Debug, Clone, Copy)]
pub struct TargetOptions {
    pub max_bytes: u64,
    pub ssim_threshold: f32,
    pub min_quality: u8,
    pub max_quality: u8,
}

impl Default for TargetOptions {
    fn default() -> Self {
        Self {
            max_bytes: 500 * 1024,
            ssim_threshold: DEFAULT_SSIM_THRESHOLD,
            min_quality: 1,
            max_quality: 100,
        }
    }
}

#[derive(Debug)]
pub struct TargetResult {
    pub data: Vec<u8>,
    pub quality: u8,
    pub ssim: f32,
    pub met_target: bool,
}

/// Binary-search the quality level that yields the largest output no bigger
/// than `max_bytes` while keeping the perceptual difference under the SSIM
/// threshold. Falls back to the highest quality when no hit is found.
pub fn compress_to_target(
    image: &RgbaImage,
    format: Format,
    opts: &TargetOptions,
) -> Result<TargetResult> {
    let dssim = Dssim::new();
    let orig_small = needs_downscale(image).then(|| downscale(image));
    let orig_ref = orig_small.as_ref().unwrap_or(image);
    let orig_dssim_img = to_dssim_image(&dssim, orig_ref);

    let (mut lo, mut hi) = (opts.min_quality.max(1), opts.max_quality.min(100));
    if format == Format::Png {
        hi = hi.min(90);
    }
    if lo > hi {
        hi = lo;
    }

    let mut best: Option<(Vec<u8>, u8, f32)> = None;

    while lo <= hi {
        let q = lo + (hi - lo) / 2;
        let out = encoder::encode(format, image, q)?;

        if out.len() as u64 > opts.max_bytes {
            hi = q.saturating_sub(1);
            continue;
        }

        let dssim_val = if format == Format::Avif {
            0.0
        } else {
            let decoded = engine::decode::load_from_memory(&out)?;
            measure_ssim(&dssim, &orig_dssim_img, orig_ref, &decoded)
        };

        if dssim_val <= opts.ssim_threshold {
            best = Some((out, q, dssim_val));
            lo = q.saturating_add(1);
        } else {
            hi = q.saturating_sub(1);
        }
    }

    if let Some((data, quality, ssim)) = best {
        return Ok(TargetResult {
            data,
            quality,
            ssim,
            met_target: true,
        });
    }

    let fallback_quality = opts.max_quality.min(100);
    let data = encoder::encode(format, image, fallback_quality)?;
    let met = data.len() as u64 <= opts.max_bytes;
    let ssim = if format == Format::Avif {
        0.0
    } else {
        let decoded = engine::decode::load_from_memory(&data)?;
        measure_ssim(&dssim, &orig_dssim_img, orig_ref, &decoded)
    };
    Ok(TargetResult {
        data,
        quality: fallback_quality,
        ssim,
        met_target: met,
    })
}

fn to_dssim_image(dssim: &Dssim, image: &RgbaImage) -> dssim::DssimImage<f32> {
    let pixels: Vec<rgb::RGBA<u8>> = image
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| rgb::RGBA::new(c[0], c[1], c[2], c[3]))
        .collect();
    dssim
        .create_image_rgba(&pixels, image.width() as usize, image.height() as usize)
        .expect("buffer size matches image dimensions")
}

fn measure_ssim(
    dssim: &Dssim,
    orig_prepared: &dssim::DssimImage<f32>,
    orig_ref: &RgbaImage,
    candidate: &RgbaImage,
) -> f32 {
    let candidate_ref = if orig_ref.width() != candidate.width() || orig_ref.height() != candidate.height()
    {
        resize_to_same(candidate, orig_ref.width(), orig_ref.height())
    } else {
        candidate.clone()
    };
    let cand_img = to_dssim_image(dssim, &candidate_ref);
    let (val, _) = dssim.compare(orig_prepared, &cand_img);
    f64::from(val) as f32
}

fn needs_downscale(image: &RgbaImage) -> bool {
    image.width() * image.height() > SSIM_MAX_EDGE * SSIM_MAX_EDGE
}

fn downscale(image: &RgbaImage) -> RgbaImage {
    let (w, h) = fit_within(image.width(), image.height(), SSIM_MAX_EDGE);
    image::imageops::resize(image, w, h, FilterType::Triangle)
}

fn resize_to_same(image: &RgbaImage, orig_w: u32, orig_h: u32) -> RgbaImage {
    let (w, h) = fit_within(orig_w, orig_h, SSIM_MAX_EDGE);
    image::imageops::resize(image, w, h, FilterType::Triangle)
}

fn fit_within(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_edge {
        return (w, h);
    }
    let scale = max_edge as f64 / longest as f64;
    (
        ((w as f64 * scale).round() as u32).max(1),
        ((h as f64 * scale).round() as u32).max(1),
    )
}
