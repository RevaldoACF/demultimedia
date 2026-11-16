#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod photo_ext;

use app::DemultimediaApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([760.0, 500.0])
            .with_min_inner_size([520.0, 340.0]),
        ..Default::default()
    };

    eframe::run_native(
        "demultimedia",
        options,
        Box::new(|_cc| Ok(Box::new(DemultimediaApp::default()))),
    )
}
