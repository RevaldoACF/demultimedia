#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod photo_ops;

use app::DemultimediaApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([820.0, 540.0])
            .with_min_inner_size([560.0, 380.0]),
        ..Default::default()
    };

    eframe::run_native(
        "demultimedia",
        options,
        Box::new(|_cc| Ok(Box::new(DemultimediaApp::default()))),
    )
}
