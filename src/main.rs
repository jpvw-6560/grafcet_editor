mod app;
mod gemma;
mod grafcet;
mod gui;
mod persistence;
mod project;

use app::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Gemma Suite")
            .with_inner_size([1400.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "gemma-suite",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
