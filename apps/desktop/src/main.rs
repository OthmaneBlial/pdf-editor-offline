#![forbid(unsafe_code)]

mod state;
mod ui;
mod worker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("PDF Editor Offline")
            .with_inner_size([1120.0, 800.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PDF Editor Offline",
        options,
        Box::new(|context| Ok(Box::new(ui::EditorApp::new(context)))),
    )
}
