#![forbid(unsafe_code)]

mod brand;
mod fonts;
mod raster;
mod search;
mod state;
mod style;
mod thumbnails;
mod ui;
mod worker;

fn main() -> eframe::Result {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--pdf-worker")) {
        if let Err(error) = pdf_worker::serve_once(pdf_worker::document::execute_document) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
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
