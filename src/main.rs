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

    let app_title = format!("Par2Dialog v{}", env!("CARGO_PKG_VERSION"));
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 550.0])
            .with_title(&app_title),
        ..Default::default()
    };

    eframe::run_native(
        &app_title,
        native_options,
        Box::new(|cc| Ok(Box::new(app::ParParApp::new(cc)))),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skip update check if we just self-updated (set in restart_process)
    let skip_update = std::env::var("PAR2DIALOG_SKIP_UPDATE") == Ok("1".to_string());

    if !skip_update {
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
    }

    real_main()?;
    Ok(())
}
