use engine::{Format, RgbaImage};
use std::path::{Path, PathBuf};

pub const PREVIEW_MAX_EDGE: u32 = 520;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Target,
    Quality,
}

#[derive(Clone)]
pub struct Settings {
    pub mode: Mode,
    pub max_size: String,
    pub quality: u8,
    pub to: Option<Format>,
    pub ssim_threshold: f32,
    pub recursive: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Target,
            max_size: "500kb".into(),
            quality: 80,
            to: None,
            ssim_threshold: 0.08,
            recursive: true,
        }
    }
}

#[derive(Clone)]
pub struct Outcome {
    pub before: u64,
    pub after: u64,
    pub quality: Option<u8>,
    pub ssim: Option<f32>,
    pub met: bool,
    pub out_path: PathBuf,
}

pub fn is_image(p: &Path) -> bool {
    matches!(Format::from_extension(p), Some(f) if f != Format::Avif)
}

pub fn format_label(f: Format) -> &'static str {
    match f {
        Format::Jpeg => "jpg",
        Format::Png => "png",
        Format::WebP => "webp",
        Format::Avif => "avif",
    }
}

pub fn process_one(path: &Path, out_dir: &Path, settings: &Settings) -> Result<Outcome, String> {
    let before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let img = engine::decode::load(path).map_err(|e| format!("decode failed: {e}"))?;
    let fmt = settings.to.unwrap_or(Format::from_extension(path).unwrap_or(Format::Jpeg));

    let (data, quality, ssim, met) = match settings.mode {
        Mode::Target => {
            let max_bytes = parse_max(&settings.max_size)
                .ok_or_else(|| "invalid max size (e.g. 500kb)".to_string())?;
            let opts = target::TargetOptions {
                max_bytes,
                ssim_threshold: settings.ssim_threshold,
                ..Default::default()
            };
            let r = target::compress_to_target(&img, fmt, &opts)
                .map_err(|e| format!("compress failed: {e}"))?;
            let ssim = (fmt != Format::Avif).then_some(r.ssim);
            (r.data, r.quality, ssim, r.met_target)
        }
        Mode::Quality => {
            let d = engine::encode::encode(fmt, &img, settings.quality)
                .map_err(|e| format!("encode failed: {e}"))?;
            (d, settings.quality, None, true)
        }
    };

    if let Err(e) = std::fs::create_dir_all(out_dir) {
        return Err(format!("cannot create output folder: {e}"));
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let out_path = out_dir.join(format!("{}.{}", stem, fmt.extension()));
    std::fs::write(&out_path, &data).map_err(|e| format!("write failed: {e}"))?;

    Ok(Outcome {
        before,
        after: data.len() as u64,
        quality: Some(quality),
        ssim,
        met,
        out_path,
    })
}

pub fn parse_max(s: &str) -> Option<u64> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let pos = t
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.' && *c != ',' && *c != ' ')
        .map(|(i, _)| i)
        .unwrap_or(t.len());
    let (num, suffix) = t.split_at(pos);
    let val: f64 = num.trim().replace(',', ".").parse().ok()?;
    let mult = match suffix.trim() {
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    if val < 0.0 {
        return None;
    }
    Some((val * mult) as u64)
}

pub fn fit(img: &RgbaImage, max_edge: u32) -> RgbaImage {
    let longest = img.width().max(img.height());
    if longest <= max_edge {
        return img.clone();
    }
    let scale = max_edge as f32 / longest as f32;
    let w = ((img.width() as f32 * scale).round() as u32).max(1);
    let h = ((img.height() as f32 * scale).round() as u32).max(1);
    image::imageops::resize(img, w, h, image::imageops::FilterType::Triangle)
}

pub fn human(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    }
}

pub fn human_file(path: &Path) -> String {
    std::fs::metadata(path)
        .map(|m| human(m.len()))
        .unwrap_or_else(|_| "-".into())
}