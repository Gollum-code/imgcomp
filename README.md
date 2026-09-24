# imgcomp

Smart image compressor — compress to an exact size budget.
Local, zero-API-cost, single ~7MB binary. Rust / mozjpeg / oxipng / libwebp / ravif.

## Features

| Feature | Free | Pro |
|---|---|---|
| Fixed-quality compression (`quality`) | unlimited | unlimited |
| Target-size compression (`--max 500kb`) | — | ✅ |
| WebP / AVIF conversion | — | ✅ |
| Batch (recursive directory, multi-threaded) | up to 10 images | unlimited |
| SSIM perceptual quality gate | — | ✅ |
| Compression report (table + CSV) | ✅ | ✅ |

## Install / Build

```bash
cargo build --release
# binary at target/release/imgcomp(.exe)
```

## GUI

`imgcomp-gui` is a native desktop app (no web runtime, ~10 MB):

- Drag & drop files or folders
- Target-size or fixed-quality mode
- Output format conversion (jpg/png/webp/avif)
- Per-image report table (before/after/quality/dssim/status)
- Side-by-side original vs compressed preview, click a row to inspect

```bash
cargo run --release -p imgcomp-gui
```

## Usage

```bash
# Compress every image under photos/ so each output is <= 500KB
imgcomp target photos/ out/ --max 500kb -r

# Convert one file to WebP under 100KB
imgcomp target hero.png hero.webp --max 100kb --to webp

# Fixed quality batch (free tier OK)
imgcomp quality photos/ out/ -q 80 --to jpg -r

# Also write a CSV report
imgcomp target photos/ out/ --max 500kb --csv report.csv
```

### Options

- `--max <SIZE>` — e.g. `500kb`, `2mb`, `1.5MB` (1KB = 1024 bytes)
- `--to <FORMAT>` — `jpg` / `png` / `webp` / `avif`
- `--ssim-threshold <F>` — max perceptual difference (dssim), `0` = identical. Default `0.08` (conservative).
- `-r` — recurse into subdirectories
- `--threads <N>` — worker threads (default: CPU count)
- `--csv <FILE>` — write a machine-readable report

## How target-size compression works

For each image, the quality level is found by binary search over `1..=100`:

1. Encode at the midpoint quality.
2. If the output exceeds `--max`, lower the quality.
3. Otherwise decode it back and compare to the original with dssim.
   If the perceptual difference stays under `--ssim-threshold`, record it and
   try a higher quality (the goal is the *highest* quality that fits).
4. If no quality fits without violating the SSIM gate, the image is kept at
   maximum quality and flagged `UNMET` (never trade clarity for size).

AVIF skips the SSIM gate (only size-driven) because the local build has no
AVIF decoder; report shows `-`.

## License / monetization

```bash
imgcomp status      # show current tier
imgcomp upgrade     # how to buy Pro
imgcomp activate <KEY>   # activate a license
imgcomp deactivate  # remove local license
```

- Keys are validated against a license server, then cached in the OS keyring
  (Windows Credential Manager / macOS Keychain / libsecret), never in plaintext.
- Re-verification happens every 3 days online; offline grace is 30 days.

Configure the validation endpoint with environment variables:

```bash
export IMGCOMP_LICENSE_API="https://your-creem-api/v1"   # validate endpoint
export IMGCOMP_PRODUCT_ID="your-product-id"
```

Payment channels (from the plan):
- **Overseas** — Creem (store.creem.io), lifetime $12.99.
- **Domestic** — Alipay Face-to-Face QR, same activation-code system.

## Repository layout

```
crates/
  engine/   decode + encode (mozjpeg / oxipng / libwebp / ravif)
  target/   target-size binary search with dssim gating
  cli/      imgcomp binary (clap + rayon + indicatif + license gating)
  gui/      imgcomp-gui binary (eframe/egui desktop app)
  license/  Creem validation + keyring cache + re-check
scripts/
  package.ps1   build a distributable zip (CLI + GUI + README)
```

## Package / release

```powershell
.\scripts\package.ps1          # -> dist\imgcomp-YYYYMMDD.zip
```

The zip contains `imgcomp.exe` (CLI), `imgcomp-gui.exe` (desktop GUI) and
`README.md` — no runtime dependencies, copy anywhere.

## Development

```bash
cargo test --workspace      # unit + integration tests
cargo clippy --workspace --all-targets
cargo build --release       # LTO + stripped single binary
```
