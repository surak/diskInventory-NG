//! Disk Inventory NG — a Rust re-imagining of Disk Inventory X.

mod app;
mod kinds;
mod scan;
mod treemap;
mod volumes;

fn main() -> eframe::Result<()> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    // Headless helpers, used by the build scripts and for benchmarking.
    match args.first().and_then(|a| a.to_str()) {
        Some("--scan-only") => {
            let path = std::path::PathBuf::from(args.get(1).expect("--scan-only <path>"));
            let start = std::time::Instant::now();
            let tree = scan::scan(&path, &scan::Progress::default()).expect("scan cancelled");
            let root = &tree.nodes[0];
            println!("{} bytes, {} files, {} nodes in {:.2?}", root.size, root.files, tree.nodes.len(), start.elapsed());
            return Ok(());
        }
        _ => {}
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Disk Inventory NG")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    // A path given on the command line is scanned right away.
    let initial = args.first().map(std::path::PathBuf::from);
    eframe::run_native(
        "Disk Inventory NG",
        options,
        Box::new(move |_cc| Ok(Box::new(app::App::new(initial)))),
    )
}
