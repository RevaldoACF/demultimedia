use std::path::{Path, PathBuf};

use eframe::egui;

use crate::photo_ops::{
    items_from_paths, process_one, scan_folder, ItemStatus, OutputFormat, PhotoItem,
    ProcessOptions,
};

pub struct DemultimediaApp {
    selected_root: Option<PathBuf>,
    recursive: bool,
    quality: u8,
    scale_percent: u8,
    target_format: OutputFormat,
    overwrite_originals: bool,
    items: Vec<PhotoItem>,
    status_line: String,
}

impl Default for DemultimediaApp {
    fn default() -> Self {
        Self {
            selected_root: None,
            recursive: true,
            quality: 75,
            scale_percent: 100,
            target_format: OutputFormat::KeepOriginal,
            overwrite_originals: false,
            items: Vec::new(),
            status_line: "Pick a folder, pick files, or drop photos here".to_string(),
        }
    }
}

impl DemultimediaApp {
    fn load_folder(&mut self, folder: PathBuf) {
        self.selected_root = Some(folder.clone());
        self.items = scan_folder(&folder, self.recursive);
        self.refresh_summary();
    }

    fn load_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.len() == 1 && paths[0].is_dir() {
            self.load_folder(paths[0].clone());
            return;
        }
        self.selected_root = None;
        self.items = items_from_paths(&paths, self.recursive);
        self.refresh_summary();
    }

    fn refresh_summary(&mut self) {
        if self.items.is_empty() {
            self.status_line = "No supported photos found".to_string();
            return;
        }
        let total_bytes: u64 = self.items.iter().map(|i| i.original_bytes).sum();
        self.status_line = format!(
            "{} photos queued ({})",
            self.items.len(),
            format_bytes(total_bytes)
        );
    }

    fn run_all(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let opts = ProcessOptions {
            quality: self.quality,
            scale_percent: self.scale_percent,
            target_format: self.target_format,
            overwrite_originals: self.overwrite_originals,
            output_dir: None,
        };

        for item in &mut self.items {
            process_one(item, &opts);
        }

        let done = self
            .items
            .iter()
            .filter(|i| i.status == ItemStatus::Done)
            .count();
        let errors = self
            .items
            .iter()
            .filter(|i| matches!(i.status, ItemStatus::Error(_)))
            .count();
        let old_total: u64 = self.items.iter().map(|i| i.original_bytes).sum();
        let new_total: u64 = self
            .items
            .iter()
            .map(|i| i.new_bytes.unwrap_or(i.original_bytes))
            .sum();

        if old_total >= new_total {
            let saved = old_total - new_total;
            self.status_line = format!(
                "Done {done}, errors {errors}, saved {}",
                format_bytes(saved)
            );
        } else {
            self.status_line = format!("Done {done}, errors {errors}");
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        let paths: Vec<PathBuf> = dropped.into_iter().filter_map(|f| f.path).collect();
        if !paths.is_empty() {
            self.load_paths(paths);
        }
    }
}

impl eframe::App for DemultimediaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_drops(ctx);

        egui::TopBottomPanel::top("top_controls")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("demultimedia");
                    ui.label("Lower photo quality, downscale, and convert formats");
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Pick Folder").clicked()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        self.load_folder(folder);
                    }

                    if ui.button("Pick Files").clicked()
                        && let Some(files) = rfd::FileDialog::new().pick_files()
                    {
                        self.load_paths(files);
                    }

                    if !self.items.is_empty() && ui.button("Clear").clicked() {
                        self.items.clear();
                        self.selected_root = None;
                        self.status_line = "Cleared".to_string();
                    }

                    if let Some(root) = &self.selected_root {
                        ui.add(egui::Label::new(root.display().to_string()).truncate());
                    }
                });

                ui.add_space(8.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label("Quality");
                    ui.add(egui::Slider::new(&mut self.quality, 1..=100));

                    ui.separator();

                    ui.label("Scale %");
                    ui.add(egui::Slider::new(&mut self.scale_percent, 10..=100));

                    ui.separator();

                    ui.label("Format");
                    egui::ComboBox::from_id_salt("format_select")
                        .selected_text(self.target_format.label())
                        .show_ui(ui, |ui| {
                            for fmt in OutputFormat::ALL {
                                ui.selectable_value(&mut self.target_format, *fmt, fmt.label());
                            }
                        });
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut self.recursive, "Include subfolders")
                        .changed()
                        && let Some(root) = self.selected_root.clone()
                    {
                        self.load_folder(root);
                    }

                    ui.checkbox(&mut self.overwrite_originals, "Overwrite originals");
                });
            });

        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status_line);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let count = self.items.len();
                        let label = if count > 0 {
                            format!("Process {count} Photos")
                        } else {
                            "Process Photos".to_string()
                        };

                        if ui
                            .add_enabled(count > 0, egui::Button::new(label))
                            .clicked()
                        {
                            self.run_all();
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.items.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Drop photos or a folder here to start");
                });
                return;
            }

            let root = self.selected_root.as_deref();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("items_grid")
                        .num_columns(5)
                        .striped(true)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            ui.strong("File");
                            ui.strong("Original");
                            ui.strong("New");
                            ui.strong("Change");
                            ui.strong("Status");
                            ui.end_row();

                            for item in &self.items {
                                ui.label(display_rel(root, &item.source));
                                ui.label(format_bytes(item.original_bytes));

                                if let Some(new_b) = item.new_bytes {
                                    ui.label(format_bytes(new_b));
                                    ui.label(format_delta(item.original_bytes, new_b));
                                } else {
                                    ui.label("-");
                                    ui.label("-");
                                }

                                match &item.status {
                                    ItemStatus::Ready => {
                                        ui.label("Ready");
                                    }
                                    ItemStatus::Done => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(80, 200, 120),
                                            "Done",
                                        );
                                    }
                                    ItemStatus::Error(err) => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(235, 95, 95),
                                            err,
                                        );
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });
        });
    }
}

// Show paths relative to the picked folder when available
fn display_rel(root: Option<&Path>, path: &Path) -> String {
    if let Some(base) = root
        && let Ok(rel) = path.strip_prefix(base)
    {
        return rel.display().to_string();
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    let mb = kb / 1024.0;
    format!("{mb:.2} MB")
}

fn format_delta(old_bytes: u64, new_bytes: u64) -> String {
    if old_bytes == 0 {
        return "0%".to_string();
    }
    let diff = new_bytes as f64 - old_bytes as f64;
    let pct = (diff / old_bytes as f64) * 100.0;
    format!("{pct:+.0}%")
}
