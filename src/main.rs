mod grafcet;
mod gui;
mod persistence;

use gui::GrafcetEditor;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Éditeur GRAFCET")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "grafcet-editor",
        options,
        Box::new(|_cc| Ok(Box::new(GrafcetEditor::default()))),
    )
}
