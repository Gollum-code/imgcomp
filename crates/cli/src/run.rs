use crate::size::human_bytes;
use anyhow::{bail, Context, Result};
use engine::{Format, RgbaImage};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub to: Option<Format>,
    pub max: Option<u64>,
    pub quality: Option<u8>,
    pub ssim_threshold: f32,
    pub recursive: bool,
    pub threads: usize,
    pub csv: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub files: Vec<FileResult>,
    pub elapsed: f64,
}

#[derive(Debug)]
pub struct FileResult {
    pub source: PathBuf,
    pub target: PathBuf,
    pub format: Format,
    pub before: u64,
    pub after: u64,
    pub quality: Option<u8>,
    pub ssim: Option<f32>,
    pub ok: bool,
    pub unmet: bool,
    pub error: Option<String>,
}

pub fn run(opts: &RunOptions) -> Result<Summary> {
    let files = collect_files(&opts.input, opts.recursive)?;
    if files.is_empty() {
        bail!("no supported images found under {}", opts.input.display());
    }

    let input_is_file = opts.input.is_file();
    let output_is_file = input_is_file && looks_like_file_path(&opts.output);
    if !output_is_file {
        fs::create_dir_all(&opts.output).with_context(|| {
            format!("cannot create output dir {}", opts.output.display())
        })?;
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg} {percent}%",
        )?
        .progress_chars("##-"),
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.threads.max(1))
        .build()
        .context("cannot build thread pool")?;

    let started = Instant::now();
    let results: Vec<FileResult> = pool.install(|| {
        files
            .par_iter()
            .map(|(src, rel)| {
                pb.inc(1);
                let msg = format!(
                    "{}",
                    src.file_name().unwrap_or_default().to_string_lossy()
                );
                pb.set_message(msg);
                process_file(src, rel, opts, output_is_file)
            })
            .collect()
    });
    pb.finish_and_clear();

    let summary = Summary {
        files: results,
        elapsed: started.elapsed().as_secs_f64(),
    };
    print_report(&summary);
    if let Some(csv_path) = &opts.csv {
        write_csv(csv_path, &summary)?;
    }
    Ok(summary)
}

fn collect_files(input: &Path, recursive: bool) -> Result<Vec<(PathBuf, PathBuf)>> {
    if input.is_file() {
        let fmt = Format::from_extension(input)
            .filter(|f| *f != Format::Avif)
            .context("input file must be jpg/png/webp")?;
        let _ = fmt;
        return Ok(vec![(input.to_path_buf(), PathBuf::from(
            input.file_name().unwrap_or_default(),
        ))]);
    }
    if input.is_dir() {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(input)
            .follow_links(false)
            .max_depth(if recursive { usize::MAX } else { 1 });
        for entry in walker {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(f) = Format::from_extension(path) {
                if f == Format::Avif {
                    continue;
                }
                let rel = path
                    .strip_prefix(input)
                    .unwrap_or(Path::new(""))
                    .to_path_buf();
                out.push((path.to_path_buf(), rel));
            }
        }
        out.sort();
        return Ok(out);
    }
    bail!("input does not exist: {}", input.display())
}

fn process_file(
    src: &Path,
    rel: &Path,
    opts: &RunOptions,
    output_is_file: bool,
) -> FileResult {
    let fmt = opts.to.unwrap_or_else(|| {
        Format::from_extension(src).unwrap_or(Format::Jpeg)
    });
    let before = fs::metadata(src).map(|m| m.len()).unwrap_or(0);

    let mut result = FileResult {
        source: src.to_path_buf(),
        target: PathBuf::new(),
        format: fmt,
        before,
        after: 0,
        quality: None,
        ssim: None,
        ok: false,
        unmet: false,
        error: None,
    };
match process_inner(src, fmt, opts) {
        Ok((data, quality, ssim, unmet)) => {
            let out_path = if output_is_file {
                opts.output.clone()
            } else {
                opts.output.join(rel).with_extension(fmt.extension())
            };
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            if fs::write(&out_path, &data).is_err() {
                result.error = Some(format!("cannot write {}", out_path.display()));
            } else {
                result.target = out_path;
                result.after = data.len() as u64;
                result.quality = Some(quality);
                result.ssim = ssim;
                result.unmet = unmet;
                result.ok = true;
            }
        }
        Err(e) => {
            result.error = Some(e.to_string());
        }
    }
    result
}

fn process_inner(
    src: &Path,
    fmt: Format,
    opts: &RunOptions,
) -> Result<(Vec<u8>, u8, Option<f32>, bool)> {
    let image: RgbaImage = engine::decode::load(src)
        .with_context(|| format!("cannot load {}", src.display()))?;
    engine::encode::check_dimensions(&image)?;

    match opts.max {
        Some(max_bytes) => {
            let target_opts = target::TargetOptions {
                max_bytes,
                ssim_threshold: opts.ssim_threshold,
                ..Default::default()
            };
            let res = target::compress_to_target(&image, fmt, &target_opts)?;
            let ssim = (fmt != Format::Avif).then_some(res.ssim);
            Ok((res.data, res.quality, ssim, !res.met_target))
        }
        None => {
            let q = opts.quality.unwrap_or(80);
            let data = engine::encode::encode(fmt, &image, q)?;
            Ok((data, q, None, false))
        }
    }
}

fn looks_like_file_path(path: &Path) -> bool {
    path.extension().is_some()
}

fn print_report(summary: &Summary) {
    let ok_count = summary.files.iter().filter(|f| f.ok && !f.unmet).count();
    let unmet_count = summary.files.iter().filter(|f| f.ok && f.unmet).count();
    let failed_count = summary.files.iter().filter(|f| !f.ok).count();
    let total_before: u64 = summary.files.iter().map(|f| f.before).sum();
    let total_after: u64 = summary.files.iter().map(|f| f.after).sum();

    println!();
    println!(
        "{:<40} {:>18} {:>9} {:>7} {:>7} {:>8}",
        "file", "before -> after", "ratio", "quality", "ssim", "status"
    );
    println!("{}", "-".repeat(96));
    for f in &summary.files {
        let name = f.source.file_name().unwrap_or_default().to_string_lossy();
        let size_pair = format!(
            "{} -> {}",
            human_bytes(f.before),
            if f.after > 0 { human_bytes(f.after) } else { "-".into() }
        );
        let ratio = if f.before > 0 && f.after > 0 {
            format!("{:>+.1}%", (1.0 - f.after as f64 / f.before as f64) * 100.0)
        } else {
            "-".into()
        };
        let quality = f.quality.map(|q| q.to_string()).unwrap_or_else(|| "-".into());
        let ssim = f.ssim.map(|s| format!("{s:.4}")).unwrap_or_else(|| "-".into());
        let status = if f.unmet {
            "UNMET".to_string()
        } else if f.ok {
            "OK".to_string()
        } else {
            "FAIL".to_string()
        };
        println!(
            "{:<40} {:>18} {:>9} {:>7} {:>7} {:>8}",
            name, size_pair, ratio, quality, ssim, status
        );
        if let Some(err) = &f.error {
            println!("    error: {err}");
        }
    }
    println!("{}", "-".repeat(96));
    println!(
        "Processed: {}  OK: {}  Unmet: {}  Failed: {}",
        summary.files.len(),
        ok_count,
        unmet_count,
        failed_count
    );
    if total_before > 0 {
        println!(
            "Total: {} -> {} ({:+.1}%)   elapsed: {:.2}s",
            human_bytes(total_before),
            human_bytes(total_after),
            (1.0 - total_after as f64 / total_before as f64) * 100.0,
            summary.elapsed
        );
    }
}

fn write_csv(path: &Path, summary: &Summary) -> Result<()> {
    let mut out = String::from("file,format,before_bytes,after_bytes,ratio,quality,ssim,status,error\n");
    for f in &summary.files {
        let name = f.source.to_string_lossy();
        let ratio = if f.before > 0 && f.after > 0 {
            (1.0 - f.after as f64 / f.before as f64) * 100.0
        } else {
            0.0
        };
        let status = if f.unmet {
            "unmet"
        } else if f.ok {
            "ok"
        } else {
            "failed"
        };
        let error = f.error.clone().unwrap_or_default().replace(',', ";");
        out.push_str(&format!(
            "{},{},{},{},{:.1},{},{:.4},{},{}\n",
            name,
            f.format.extension(),
            f.before,
            f.after,
            ratio,
            f.quality.map(|q| q.to_string()).unwrap_or_default(),
            f.ssim.unwrap_or(0.0),
            status,
            error
        ));
    }
    fs::write(path, out).with_context(|| format!("cannot write csv {}", path.display()))
}

#[allow(dead_code)]
pub fn total_bytes_grouped(summary: &Summary) -> BTreeMap<Format, (u64, u64)> {
    let mut m = BTreeMap::new();
    for f in &summary.files {
        let e = m.entry(f.format).or_insert((0, 0));
        e.0 += f.before;
        e.1 += f.after;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_file_path_detects_extension() {
        assert!(looks_like_file_path(Path::new("out.jpg")));
        assert!(!looks_like_file_path(Path::new("outdir")));
    }
}
