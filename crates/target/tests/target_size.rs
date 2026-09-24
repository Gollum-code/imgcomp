use engine::RgbaImage;
use target::{TargetOptions, compress_to_target};

fn test_image(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;
        let bump = ((x * 7 + y * 13) % 17) as u8;
        [
            (fx * 255.0) as u8,
            (fy * 255.0) as u8,
            ((fx * fy * 255.0) as u8).saturating_add(bump),
            255,
        ]
        .into()
    })
}

#[test]
fn jpeg_hits_target_or_falls_back_cleanly() {
    let img = test_image(800, 600);
    let opts = TargetOptions {
        max_bytes: 60 * 1024,
        ..Default::default()
    };
    let result = compress_to_target(&img, engine::Format::Jpeg, &opts).unwrap();
    if result.met_target {
        assert!(result.data.len() as u64 <= opts.max_bytes);
        assert!(result.ssim <= opts.ssim_threshold);
    }
    assert!(!result.data.is_empty());
}

#[test]
fn webp_hits_target_or_falls_back_cleanly() {
    let img = test_image(640, 480);
    let opts = TargetOptions {
        max_bytes: 30 * 1024,
        ..Default::default()
    };
    let result = compress_to_target(&img, engine::Format::WebP, &opts).unwrap();
    if result.met_target {
        assert!(result.data.len() as u64 <= opts.max_bytes);
    }
    assert!(!result.data.is_empty());
}

#[test]
fn impossible_target_returns_original_quality_without_lying() {
    let img = test_image(2000, 1500);
    let opts = TargetOptions {
        max_bytes: 1,
        ..Default::default()
    };
    let result = compress_to_target(&img, engine::Format::Jpeg, &opts).unwrap();
    assert!(!result.met_target);
    assert!(result.data.len() as u64 > 1);
    assert_eq!(result.quality, 100);
}
