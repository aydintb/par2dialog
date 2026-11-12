//! Thin wrappers around `rfd` file dialogs so all tabs open dialogs the same way.

use std::path::PathBuf;

/// Pick one or more existing files (optionally filtered by extension).
pub fn pick_files_dialog(title: &str, filter_ext: Option<&str>) -> Vec<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    dialog.pick_files().unwrap_or_default()
}

/// Pick a single existing file (optionally filtered by extension).
pub fn pick_file_dialog(title: &str, filter_ext: Option<&str>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    dialog.pick_file()
}

/// Pick an existing directory.
pub fn pick_directory_dialog(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

/// Pick a target path for saving (optionally filtered by extension).
pub fn save_file_dialog(
    title: &str,
    filter_ext: Option<&str>,
    default_name: Option<&str>,
) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    if let Some(name) = default_name {
        dialog = dialog.set_file_name(name);
    }
    dialog.save_file()
}
