use engine::{Format, RgbaImage};
use image::ImageFormat;
use imgcomp_gui::{Mode, Settings, parse_max, process_one};
use std::path::Path;

fn make_image_file(path: &Path) {
    let img: RgbaImage = RgbaImage::from_fn(64, 48, |x, y| {
        [(x * 4) as u8, (y * 5) as u8, 128, 255].into()
    });
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    std::fs::write(path, buf.into_inner()).unwrap();
}

fn tmpdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("imgcomp_gui_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parses_target_sizes() {
    assert_eq!(parse_max("500kb"), Some(500 * 1024));
    assert_eq!(parse_max("1.5MB"), Some((1.5 * 1024.0 * 1024.0) as u64));
    assert_eq!(parse_max("100"), Some(100));
    assert_eq!(parse_max("2 mib"), Some(2 * 1024 * 1024));
    assert_eq!(parse_max("abc"), None);
    assert_eq!(parse_max(""), None);
}

#[test]
fn process_one_quality_mode_writes_output() {
    let dir = tmpdir();
    let src = dir.join("photo.png");
    let out = dir.join("out");
    make_image_file(&src);

    let settings = Settings {
        mode: Mode::Quality,
        max_size: "500kb".into(),
        quality: 70,
        to: Some(Format::Jpeg),
        ssim_threshold: 0.08,
        recursive: false,
    };
    let outcome = process_one(&src, &out, &settings).unwrap();
    assert!(outcome.after > 0);
    assert!(outcome.out_path.exists());
    assert_eq!(outcome.out_path.extension().unwrap(), "jpg");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn process_one_target_mode_respects_budget() {
    let dir = tmpdir();
    let src = dir.join("photo.png");
    let out = dir.join("out");
    make_image_file(&src);

    let settings = Settings {
        mode: Mode::Target,
        max_size: "2kb".into(),
        quality: 80,
        to: None,
        ssim_threshold: 0.5,
        recursive: false,
    };
    let outcome = process_one(&src, &out, &settings).unwrap();
    if outcome.met {
        assert!(outcome.after <= 2 * 1024);
    }
    assert!(outcome.out_path.exists());
    std::fs::remove_dir_all(&dir).ok();
}
