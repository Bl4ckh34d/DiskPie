#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
use std::{path::PathBuf, sync::atomic::AtomicBool};

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--benchmark") {
        if args.len() < 2 {
            std::process::exit(2);
        }
        let workers = args
            .get(2)
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);
        match diskpie::scan::scan(
            &PathBuf::from(&args[1]),
            workers,
            &AtomicBool::new(false),
            &diskpie::scan::Progress::default(),
        ) {
            Ok(s) => println!(
                "files={} folders={} bytes={} errors={} skipped={} seconds={:.6}",
                s.files, s.folders, s.nodes[0].bytes, s.errors, s.skipped, s.seconds
            ),
            Err(_) => {
                eprintln!("Scan failed (path details withheld)");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let root = if args.first().is_some_and(|s| s == "--scan") {
        args.get(1).map(PathBuf::from)
    } else {
        None
    };
    let demo = args.first().is_some_and(|s| s == "--demo");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([850.0, 580.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "DiskPie",
        options,
        Box::new(move |cc| Ok(Box::new(app::DiskPieApp::new(cc, root, demo)))),
    )
}
