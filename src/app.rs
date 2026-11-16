use std::path::{Path, PathBuf};

use eframe::egui;

use crate::photo_ext::{apply_renames, scan_folder, CandidateStatus, RenameCandidate};

pub struct DemultimediaApp {
    selected_folder: Option<PathBuf>,
    recursive: bool,
    candidates: Vec<RenameCandidate>,
    status_line: String,
}

impl Default for DemultimediaApp {
    fn default() -> Self {
        Self {
            selected_folder: None,
            recursive: true,
            candidates: Vec::new(),
            status_line: "Pick a folder to scan for uppercase photo extensions".to_string(),
        }
    }
}

impl DemultimediaApp {
    fn run_scan(&mut self) {
        let Some(folder) = self.selected_folder.as_deref() else {
            self.candidates.clear();
            self.status_line = "No folder selected".to_string();
            return;
        };

        self.candidates = scan_folder(folder, self.recursive);
        let ready = self
            .candidates
            .iter()
            .filter(|c| c.status == CandidateStatus::Ready)
            .count();
        let conflicts = self.candidates.len().saturating_sub(ready);

        if self.candidates.is_empty() {
            self.status_line = "Everything looks lowercase already".to_string();
        } else {
            self.status_line = format!("{ready} ready to rename, {conflicts} conflicts");
        }
    }

    fn run_apply(&mut self) {
        if self.candidates.is_empty() {
            return;
        }

        let summary = apply_renames(&self.candidates);
        let renamed = summary.renamed;
        let skipped = summary.skipped_conflicts;
        let err_count = summary.errors.len();

        self.run_scan();

        if err_count == 0 {
            self.status_line = format!("Renamed {renamed} files, skipped {skipped} conflicts");
        } else {
            self.status_line = format!(
                "Renamed {renamed} files, {err_count} errors ({})",
                summary.errors[0]
            );
        }
    }

    fn handle_dropped_folder(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path) = file.path
                && path.is_dir()
            {
                self.selected_folder = Some(path);
                self.run_scan();
                break;
            }
        }
    }
}

impl eframe::App for DemultimediaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_folder(ctx);

        egui::TopBottomPanel::top("top_controls")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("demultimedia");
                    ui.label("Lowercase photo extensions without touching a terminal");
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Pick Folder").clicked()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        self.selected_folder = Some(folder);
                        self.run_scan();
                    }

                    let folder_text = self
                        .selected_folder
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Drop a folder here or click Pick Folder".to_string());
                    ui.add(egui::Label::new(folder_text).truncate());
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut self.recursive, "Include subfolders")
                        .changed()
                        && self.selected_folder.is_some()
                    {
                        self.run_scan();
                    }

                    let can_scan = self.selected_folder.is_some();
                    if ui
                        .add_enabled(can_scan, egui::Button::new("Rescan"))
                        .clicked()
                    {
                        self.run_scan();
                    }
                });
            });

        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status_line);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ready_count = self
                            .candidates
                            .iter()
                            .filter(|c| c.status == CandidateStatus::Ready)
                            .count();
                        let label = if ready_count > 0 {
                            format!("Lowercase {ready_count} Files")
                        } else {
                            "Lowercase Files".to_string()
                        };

                        if ui
                            .add_enabled(ready_count > 0, egui::Button::new(label))
                            .clicked()
                        {
                            self.run_apply();
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.candidates.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("No uppercase photo extensions queued");
                });
                return;
            }

            let root = self.selected_folder.as_deref();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("preview_grid")
                        .num_columns(3)
                        .striped(true)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.strong("Original");
                            ui.strong("New");
                            ui.strong("Status");
                            ui.end_row();

                            for item in &self.candidates {
                                ui.label(display_rel(root, &item.source));
                                ui.label(display_rel(root, &item.target));
                                match item.status {
                                    CandidateStatus::Ready => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(80, 200, 120),
                                            "Ready",
                                        );
                                    }
                                    CandidateStatus::Conflict => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(235, 160, 60),
                                            "Conflict",
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

// Show paths relative to the picked folder so the preview stays readable
fn display_rel(root: Option<&Path>, path: &Path) -> String {
    if let Some(base) = root
        && let Ok(rel) = path.strip_prefix(base)
    {
        return rel.display().to_string();
    }
    path.display().to_string()
}
