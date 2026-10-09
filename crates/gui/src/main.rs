use eframe::egui;
use imgcomp_gui::{fit, format_label, human, human_file, is_image, Mode, Outcome, Settings};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

enum Msg {
    ItemDone(usize, Result<Outcome, String>),
    AllDone,
}

struct Preview {
    tex: egui::TextureHandle,
    label: String,
}

struct App {
    settings: Settings,
    output_dir: String,
    files: Vec<PathBuf>,
    results: Vec<Option<Result<Outcome, String>>>,
    selected: Option<usize>,
    running: bool,
    done_count: usize,
    started: Option<Instant>,
    rx: Option<Receiver<Msg>>,
    preview_orig: Option<Preview>,
    preview_out: Option<Preview>,
    preview_index: Option<usize>,
    message: String,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            settings: Settings::default(),
            output_dir: String::new(),
            files: Vec::new(),
            results: Vec::new(),
            selected: None,
            running: false,
            done_count: 0,
            started: None,
            rx: None,
            preview_orig: None,
            preview_out: None,
            preview_index: None,
            message: String::new(),
        }
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let mut added = Vec::new();
        let recursive = self.settings.recursive;
        for p in paths {
            if p.is_dir() {
                let depth = if recursive { usize::MAX } else { 1 };
                for entry in walkdir::WalkDir::new(&p).max_depth(depth) {
                    let Ok(entry) = entry else { continue };
                    if entry.file_type().is_file() && is_image(entry.path()) {
                        added.push(entry.path().to_path_buf());
                    }
                }
            } else if is_image(&p) {
                added.push(p);
            }
        }
        added.sort();
        added.dedup();
        for a in added {
            if !self.files.contains(&a) {
                self.files.push(a);
            }
        }
        self.results.resize_with(self.files.len(), || None);
        if !self.files.is_empty() {
            self.message = format!("Loaded {} image(s)", self.files.len());
        }
        ctx.request_repaint();
    }

    fn clear(&mut self) {
        self.files.clear();
        self.results.clear();
        self.selected = None;
        self.preview_orig = None;
        self.preview_out = None;
        self.preview_index = None;
        self.message = "Cleared".into();
    }

    fn start(&mut self) {
        if self.running || self.files.is_empty() {
            return;
        }
        let out_dir_text = self.output_dir.trim().to_string();
        if out_dir_text.is_empty() {
            self.message = "Pick an output folder first".into();
            return;
        }
        let out_dir = PathBuf::from(&out_dir_text);
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            self.message = format!("Cannot create output folder: {e}");
            return;
        }

        self.results = vec![None; self.files.len()];
        self.done_count = 0;
        self.running = true;
        self.started = Some(Instant::now());
        self.selected = None;
        self.preview_orig = None;
        self.preview_out = None;
        self.preview_index = None;

        let settings = self.settings.clone();
        let files = self.files.clone();
        let (tx, rx): (Sender<Msg>, Receiver<Msg>) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            for (i, path) in files.iter().enumerate() {
                let outcome = imgcomp_gui::process_one(path, &out_dir, &settings);
                if tx.send(Msg::ItemDone(i, outcome)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::AllDone);
        });
        self.message = "Processing…".into();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(Msg::ItemDone(i, outcome)) => {
                    self.results[i] = Some(outcome);
                    self.done_count += 1;
                    changed = true;
                }
                Ok(Msg::AllDone) => {
                    self.running = false;
                    self.rx = None;
                    let secs = self
                        .started
                        .map(|s| s.elapsed().as_secs_f64())
                        .unwrap_or(0.0);
                    self.message = format!(
                        "Done in {secs:.2}s — {}/{} processed",
                        self.done_count,
                        self.files.len()
                    );
                    changed = true;
                    break;
                }
                Err(_) => break,
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }

    fn select(&mut self, index: usize, ctx: &egui::Context) {
        self.selected = Some(index);
        if self.preview_index == Some(index) {
            return;
        }
        self.preview_index = Some(index);
        self.preview_orig = load_preview(ctx, &self.files[index]);
        self.preview_out = match self.results[index].as_ref().and_then(|r| r.as_ref().ok()) {
            Some(o) if o.out_path.exists() => load_preview(ctx, &o.out_path),
            _ => None,
        };
        ctx.request_repaint();
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(dropped, ctx);
        }
    }
}

fn load_preview(ctx: &egui::Context, path: &Path) -> Option<Preview> {
    let img = engine::decode::load(path).ok()?;
    let sized = fit(&img, imgcomp_gui::PREVIEW_MAX_EDGE);
    let size = [sized.width() as usize, sized.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, sized.as_raw());
    let tex = ctx.load_texture(
        format!("preview_{}", std::process::id()),
        color,
        egui::TextureOptions::LINEAR,
    );
    Some(Preview {
        tex,
        label: format!(
            "{} × {} — {}",
            sized.width(),
            sized.height(),
            human_file(path)
        ),
    })
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_drops(&ctx);
        self.poll(&ctx);

        egui::panel::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("imgcomp");
                ui.separator();
                if ui.button("Add files").clicked() {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp"])
                        .pick_files()
                    {
                        self.add_paths(paths, &ctx);
                    }
                }
                if ui.button("Add folder").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.add_paths(vec![dir], &ctx);
                    }
                }
                if ui.button("Clear").clicked() {
                    self.clear();
                }
                ui.separator();
                ui.label("output:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.output_dir)
                        .hint_text("pick a folder…")
                        .desired_width(220.0),
                );
                if ui.button("Browse").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.output_dir = dir.to_string_lossy().into_owned();
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::panel::Panel::left("settings")
            .exact_size(260.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings.mode, Mode::Target, "Target size");
                    ui.selectable_value(&mut self.settings.mode, Mode::Quality, "Quality");
                });
                ui.separator();

                match self.settings.mode {
                    Mode::Target => {
                        ui.label("Max size (500kb / 2mb / 1.5MB):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.max_size)
                                .desired_width(120.0),
                        );
                        ui.add_space(6.0);
                        ui.label("Max perceptual diff (dssim):");
                        ui.add(
                            egui::Slider::new(&mut self.settings.ssim_threshold, 0.0..=0.5)
                                .text("0 = identical"),
                        );
                        ui.small(
                            "If no quality fits without exceeding the diff, the file is kept at max quality and marked UNMET.",
                        );
                    }
                    Mode::Quality => {
                        ui.label("Quality:");
                        ui.add(
                            egui::Slider::new(&mut self.settings.quality, 1..=100)
                                .text("1 = smallest / 100 = best"),
                        );
                    }
                }

                ui.separator();
                ui.label("Output format:");
                egui::ComboBox::from_id_salt("fmt")
                    .selected_text(self.settings.to.map(format_label).unwrap_or("keep input"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.settings.to, None, "keep input");
                        for f in [
                            engine::Format::Jpeg,
                            engine::Format::Png,
                            engine::Format::WebP,
                            engine::Format::Avif,
                        ] {
                            ui.selectable_value(&mut self.settings.to, Some(f), format_label(f));
                        }
                    });
                ui.checkbox(&mut self.settings.recursive, "Recurse folders when adding");

                ui.separator();
                let can_start =
                    !self.running && !self.files.is_empty() && !self.output_dir.is_empty();
                let compressed = ui
                    .add_enabled(
                        can_start,
                        egui::Button::new("Compress").min_size(egui::vec2(230.0, 36.0)),
                    )
                    .clicked();
                if compressed {
                    self.start();
                }
                if self.running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("processing…");
                    });
                }
                if !self.message.is_empty() {
                    ui.separator();
                    ui.label(&self.message);
                }
            });

        egui::panel::Panel::bottom("progress").show(ui, |ui| {
            let total = self.files.len();
            let done = self.done_count;
            let pct = if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            };
            ui.add(egui::ProgressBar::new(pct).show_percentage());
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if self.files.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("Drop images or folders here");
                        ui.label("or use the buttons above.");
                    });
                });
                return;
            }

            let mut clicked = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("files").striped(true).show(ui, |ui| {
                    ui.heading("file");
                    ui.heading("before");
                    ui.heading("after");
                    ui.heading("saved");
                    ui.heading("quality");
                    ui.heading("dssim");
                    ui.heading("status");
                    ui.end_row();

                    for (i, path) in self.files.iter().enumerate() {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        let is_sel = self.selected == Some(i);
                        ui.horizontal(|ui| {
                            if ui.selectable_label(is_sel, name).clicked() {
                                clicked = Some(i);
                            }
                        });

                        match &self.results[i] {
                            Some(Ok(o)) => {
                                ui.label(human(o.before));
                                ui.label(human(o.after));
                                let saved = if o.before > 0 {
                                    (1.0 - o.after as f64 / o.before as f64) * 100.0
                                } else {
                                    0.0
                                };
                                ui.label(format!("{saved:.0}%"));
                                ui.label(o.quality.map(|q| q.to_string()).unwrap_or_default());
                                ui.label(o.ssim.map(|s| format!("{s:.4}")).unwrap_or_default());
                                let (txt, color) = if o.met {
                                    ("OK".to_string(), egui::Color32::from_rgb(110, 210, 120))
                                } else {
                                    ("UNMET".to_string(), egui::Color32::from_rgb(240, 190, 90))
                                };
                                ui.label(egui::RichText::new(txt).color(color));
                            }
                            Some(Err(e)) => {
                                ui.label("-");
                                ui.label("-");
                                ui.label("-");
                                ui.label("-");
                                ui.label("-");
                                ui.label(
                                    egui::RichText::new("FAIL")
                                        .color(egui::Color32::from_rgb(240, 110, 110)),
                                );
                                ui.label(egui::RichText::new(e).small());
                            }
                            None => {
                                for _ in 0..6 {
                                    ui.label("…");
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            });

            if let Some(i) = clicked {
                self.select(i, &ctx);
            }

            if self.preview_index.is_some() {
                ui.separator();
                ui.heading("Preview");
                ui.horizontal(|ui| {
                    if let Some(p) = &self.preview_orig {
                        show_one(ui, "original", p);
                    }
                    if let Some(p) = &self.preview_out {
                        show_one(ui, "compressed", p);
                    } else {
                        ui.label("(no output yet)");
                    }
                });
            }
        });
    }
}

fn show_one(ui: &mut egui::Ui, title: &str, p: &Preview) {
    ui.vertical(|ui| {
        ui.label(title);
        ui.add(egui::Image::new(&p.tex).fit_to_exact_size(egui::vec2(240.0, 240.0)));
        ui.small(&p.label);
    });
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("imgcomp — smart image compressor"),
        ..Default::default()
    };
    eframe::run_native("imgcomp", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
