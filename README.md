<div align="center">

# 🖼️ imgcomp

**按目标体积压缩图片的本地工具** — 说"压到 500KB 以内"，它就给你压到刚好达标。

`CLI` · `桌面 GUI` · `纯本地` · `开源免费`

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform: Windows/macOS/Linux](https://img.shields.io/badge/platform-Windows%20·%20macOS%20·%20Linux-lightgrey)]()

</div>

---

## 为什么需要它

电商主图要 ≤500KB、网页图片要 ≤100KB、社媒图要 ≤2MB……手调质量参数猜半天大小，压完要么超了要么糊了。

**imgcomp 自动帮你二分质量**，直到体积达标；同时内置**感知质量门控（SSIM）**——体积和画质冲突时，宁大勿糊。

```
照片 1.2MB → 99.7KB  ≤500KB ✅  质量 80
照片 2.4MB → 98.1KB  ≤500KB ✅  质量 72
```

## 核心特性

- 🎯 **目标体积压缩** `--max 500kb` — 自动二分，精准达标
- 👁️ **SSIM 质量门控** — 感知对比防过度压缩，绝不为了体积毁图
- 🔄 **格式转换** — JPG/PNG → WebP/AVIF（比 JPEG 小 30–70%）
- 📁 **批量 + 递归目录** — 多线程并行，进度条 + 前后对比报告
- 🖥️ **桌面 GUI** — 拖拽、原图/压缩后并排预览对比
- 🔒 **纯本地** — 不联网、不上传、无 API 成本，图片不出你的电脑

## 快速开始

### 下载

- **Windows**: 从 [Releases](https://github.com/Gollum-code/imgcomp/releases) 下载 zip，解压即用（无需安装）
- **源码编译**: 见下方 [从源码构建](#-从源码构建)

### 命令行

```bash
# 把 photos/ 下所有图片压到 ≤500KB（递归子目录）
imgcomp target photos/ out/ --max 500kb -r

# 单张图转 WebP，压到 ≤100KB
imgcomp target hero.png hero.webp --max 100kb --to webp

# 固定质量批量压缩
imgcomp quality photos/ out/ -q 80 --to jpg -r

# 生成 CSV 报告
imgcomp target photos/ out/ --max 500kb --csv report.csv
```

### 桌面 GUI

```bash
imgcomp-gui
```

拖入图片/文件夹 → 选目标体积或质量 → 点 Compress → 点任意行对比原图与压缩结果。

## 从源码构建

```bash
git clone https://github.com/Gollum-code/imgcomp.git
cd imgcomp

# CLI + GUI
cargo build --release

# 二进制
target/release/imgcomp        # CLI
target/release/imgcomp-gui    # 桌面版
```

需要 [Rust 1.75+](https://rustup.rs)。Windows 编译 GUI 无需额外依赖；AVIF 编码为纯 Rust 实现（无需 NASM）。

## 命令参考

```
imgcomp <COMMAND>

Commands:
  target   按目标体积压缩（如 --max 500kb）
  quality  固定质量压缩（1-100）
  status   查看当前版本/授权状态
  help     帮助
```

### target 模式参数

| 参数 | 说明 |
|---|---|
| `--max <SIZE>` | 目标上限：`500kb`、`2mb`、`1.5MB`（1KB = 1024 字节） |
| `--to <FORMAT>` | 输出格式：`jpg` / `png` / `webp` / `avif` |
| `--ssim-threshold <F>` | 最大感知差异（dssim），`0` = 与原图完全一致，默认 `0.08`（保守） |
| `-r` | 递归子目录 |
| `--threads <N>` | 并行线程数（默认 = CPU 核数） |
| `--csv <FILE>` | 输出机器可读报告 |

## 工作原理

对每张图，在质量 `1..=100` 上二分查找：

1. 以中点质量编码
2. 若体积超过 `--max` → 降低质量
3. 否则解码回原尺寸，用 **dssim** 与原始图对比
4. 感知差异在阈值内 → 记下该解，继续尝试更高质量（目标是"能达标的最大质量"）
5. 若无解满足体积且不超阈值 → 保留最大质量，标记 `UNMET`（宁大勿糊）

> AVIF 跳过 SSIM 门控（仅按体积二分），因本地构建未含 AVIF 解码器。

## 项目结构

```
crates/
  engine/   解码 + 编码（mozjpeg / oxipng / libwebp / ravif）
  target/   目标体积二分 + SSIM 门控（核心算法）
  cli/      命令行（clap + rayon + indicatif）
  gui/      桌面版（eframe/egui）
  license/  可选授权层（keyring 存储）
scripts/
  package.ps1  一键打包发布 zip
```

## 开发

```bash
cargo test --workspace             # 测试
cargo clippy --workspace --all-targets   # lint
cargo build --release              # 构建（LTO + strip，CLI ~8MB）
```

## License

[MIT](LICENSE) — 随便用，注明出处即可。

---

<p align="center">Made with ❤️ · 有问题欢迎 <a href="https://github.com/Gollum-code/imgcomp/issues">提 issue</a></p>
