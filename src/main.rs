//! Disk Inventory NG — a Rust re-imagining of Disk Inventory X.

mod app;
mod kinds;
mod scan;
mod treemap;
mod volumes;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Disk Inventory NG")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    // A path given on the command line is scanned right away.
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        "Disk Inventory NG",
        options,
        Box::new(move |_cc| Ok(Box::new(app::App::new(initial)))),
    )
}
