use std::path::PathBuf;

use eframe::egui;

/// A widget that shows a path with a browse button
pub struct FilePicker {
    pub path: PathBuf,
    pub label: String,
    pub dialog_title: String,
    pub pick_mode: PickMode,
}

pub enum PickMode {
    File { save: bool, filter: Option<(String, Vec<String>)> },
    Directory,
    Files { multi: bool, filter: Option<(String, Vec<String>)> },
}

impl Default for FilePicker {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            label: "Path".to_string(),
            dialog_title: "Select".to_string(),
            pick_mode: PickMode::Directory,
        }
    }
}

impl FilePicker {
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.label(&self.label);
            ui.add(
                egui::TextEdit::singleline(&mut self.path.to_string_lossy().into_owned()).desired_width(300.0),
            );
            if ui.button("📂 Browse").clicked() {
                self.open_dialog();
            }
        });
        false
    }

    fn open_dialog(&mut self) {
        match &self.pick_mode {
            PickMode::Directory => {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title(&self.dialog_title)
                    .pick_folder()
                {
                    self.path = dir;
                }
            }
            PickMode::File { save, filter } => {
                let mut dialog = rfd::FileDialog::new().set_title(&self.dialog_title);
                if let Some((name, exts)) = filter {
                    dialog = dialog.add_filter(name, exts.as_slice());
                }
                if *save {
                    if let Some(file) = dialog.save_file() {
                        self.path = file;
                    }
                } else if let Some(file) = dialog.pick_file() {
                    self.path = file;
                }
            }
            PickMode::Files { multi, filter } => {
                let mut dialog = rfd::FileDialog::new().set_title(&self.dialog_title);
                if let Some((name, exts)) = filter {
                    dialog = dialog.add_filter(name, exts.as_slice());
                }
                if *multi {
                    if let Some(files) = dialog.pick_files() {
                        if !files.is_empty() {
                            if let Some(parent) = files[0].parent() {
                                self.path = parent.to_path_buf();
                            }
                        }
                    }
                } else if let Some(file) = dialog.pick_file() {
                    self.path = file;
                }
            }
        }
    }
}

pub fn pick_files_dialog(title: &str, filter_ext: Option<&str>) -> Vec<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    dialog.pick_files().unwrap_or_default()
}

pub fn pick_file_dialog(title: &str, filter_ext: Option<&str>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    dialog.pick_file()
}

pub fn pick_directory_dialog(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .pick_folder()
}

pub fn save_file_dialog(title: &str, filter_ext: Option<&str>, default_name: Option<&str>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(ext) = filter_ext {
        dialog = dialog.add_filter("Files", &[ext]);
    }
    if let Some(name) = default_name {
        dialog = dialog.set_file_name(name);
    }
    dialog.save_file()
}
