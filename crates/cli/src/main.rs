mod license_cmd;
mod run;
mod size;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use engine::Format;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "imgcomp",
    version,
    about = "Smart image compressor — compress to an exact size budget"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compress to a maximum size (e.g. --max 500kb)
    Target {
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,
        #[arg(long, value_name = "SIZE")]
        max: String,
        #[arg(long, value_name = "FORMAT", help = "output format: jpg/png/webp/avif (Pro: webp/avif)")]
        to: Option<String>,
        #[arg(long, value_name = "F", help = "max perceptual difference (dssim), 0 = identical [default: 0.08]")]
        ssim_threshold: Option<f32>,
        #[arg(long, short = 'r', help = "recurse into subdirectories")]
        recursive: bool,
        #[arg(long, default_value_t = default_threads())]
        threads: usize,
        #[arg(long, value_name = "FILE", help = "also write a CSV report")]
        csv: Option<PathBuf>,
    },
    /// Compress with a fixed quality (1-100)
    Quality {
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,
        #[arg(long, short = 'q', default_value_t = 80)]
        quality: u8,
        #[arg(long, value_name = "FORMAT", help = "output format: jpg/png/webp/avif (Pro: webp/avif)")]
        to: Option<String>,
        #[arg(long, short = 'r', help = "recurse into subdirectories")]
        recursive: bool,
        #[arg(long, default_value_t = default_threads())]
        threads: usize,
        #[arg(long, value_name = "FILE", help = "also write a CSV report")]
        csv: Option<PathBuf>,
    },
    /// Activate a Pro license key
    Activate { key: String },
    /// Show current license status
    Status,
    /// Show how to buy a Pro license
    Upgrade,
    /// Remove the locally stored license
    Deactivate,
}

fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn parse_format(s: &str) -> Result<Format> {
    Format::from_str(s).map_err(|e| anyhow::anyhow!(e))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Activate { key } => return license_cmd::activate(&key),
        Cmd::Status => return license_cmd::status(),
        Cmd::Upgrade => return license_cmd::upgrade(),
        Cmd::Deactivate => return license_cmd::deactivate(),
        Cmd::Target { .. } | Cmd::Quality { .. } => {}
    }

    let opts = match cli.cmd {
        Cmd::Target {
            input,
            output,
            max,
            to,
            ssim_threshold,
            recursive,
            threads,
            csv,
        } => {
            let max_bytes = size::parse_size(&max)
                .with_context(|| format!("invalid --max value: {max} (use e.g. 500kb, 2mb)"))?;
            if max_bytes == 0 {
                bail!("--max must be greater than zero");
            }
            run::RunOptions {
                input,
                output,
                to: to.as_deref().map(parse_format).transpose()?,
                max: Some(max_bytes),
                quality: None,
                ssim_threshold: ssim_threshold.unwrap_or(target::DEFAULT_SSIM_THRESHOLD),
                recursive,
                threads,
                csv,
            }
        }
        Cmd::Quality {
            input,
            output,
            quality,
            to,
            recursive,
            threads,
            csv,
        } => {
            let q = quality.clamp(1, 100);
            run::RunOptions {
                input,
                output,
                to: to.as_deref().map(parse_format).transpose()?,
                max: None,
                quality: Some(q),
                ssim_threshold: target::DEFAULT_SSIM_THRESHOLD,
                recursive,
                threads,
                csv,
            }
        }
        Cmd::Activate { .. } | Cmd::Status | Cmd::Upgrade | Cmd::Deactivate => unreachable!(),
    };

    let summary = run::run(&opts)?;
    let failed = summary.files.iter().filter(|f| !f.ok).count();
    if failed > 0 {
        bail!("{} file(s) failed", failed);
    }
    Ok(())
}