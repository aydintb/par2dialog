//! Small shared utilities.

use std::path::{Path, PathBuf};

/// Display name used for window titles, config paths and log lines.
pub const APP_NAME: &str = "par2dialog";

/// Format a byte count for human display.
#[must_use]
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;

    let bytes = f64::from(u32::try_from(bytes).unwrap_or(u32::MAX));
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes / KB)
    } else if bytes < GB {
        format!("{:.1} MB", bytes / MB)
    } else {
        format!("{:.2} GB", bytes / GB)
    }
}

/// Open a path in the platform file manager (best effort).
pub fn open_in_file_manager(path: &std::path::Path) {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    };

    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&dir).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&dir).spawn()
    };

    if let Err(e) = result {
        tracing::warn!("Could not open {}: {e}", dir.display());
    }
}
