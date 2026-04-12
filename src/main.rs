#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

mod app;
mod par2;
mod tabs;
mod theme;
mod util;
mod widgets;

use eframe::egui;
use self_updater_helper::{run_update, UpdaterConfig};

fn real_main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 550.0])
            .with_title("Par2Dialog"),
        ..Default::default()
    };

    eframe::run_native(
        "Par2Dialog",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ParParApp::new(cc)))),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = UpdaterConfig {
        owner: "aydintb".into(),
        repo: "par2dialog".into(),
        bin_name: "par2dialog".into(),
        current_version: env!("CARGO_PKG_VERSION").into(),
        ..Default::default()
    };

    if let Err(e) = run_update(&config) {
        eprintln!("Update check failed: {e} (app will continue normally)");
    }

    real_main()?;
    Ok(())
}
