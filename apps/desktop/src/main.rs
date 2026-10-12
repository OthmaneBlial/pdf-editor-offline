#![forbid(unsafe_code)]

mod fonts;
mod raster;
mod search;
mod state;
mod style;
mod ui;
mod worker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("PDF Editor Offline")
            .with_inner_size([1260.0, 860.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    let options = {
        let mut options = options;
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            // Winit's default Quit menu terminates directly, bypassing our unsaved-change guard.
            builder.with_default_menu(false);
        }));
        options
    };
    eframe::run_native(
        "PDF Editor Offline",
        options,
        Box::new(|context| Ok(Box::new(ui::EditorApp::new(context)))),
    )
}
