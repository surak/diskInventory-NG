//! Disk Inventory NG — a Rust re-imagining of Disk Inventory X.

// No console window for release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod icon;
mod kinds;
mod platform;
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
        Some("--write-icon") => {
            let path = std::path::PathBuf::from(args.get(1).expect("--write-icon <path.png|path.ico> [size]"));
            if path.extension().is_some_and(|e| e == "ico") {
                icon::write_ico(&path).expect("could not write icon");
                return Ok(());
            }
            let size = args.get(2).and_then(|s| s.to_str()?.parse().ok()).unwrap_or(1024);
            icon::write_png(&path, size).expect("could not write icon");
            return Ok(());
        }
        _ => {}
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Disk Inventory NG")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(eframe::egui::IconData { rgba: icon::render(256), width: 256, height: 256 }),
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
