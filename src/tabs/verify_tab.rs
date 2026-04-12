use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::par2::verify;
use crate::par2::types::FileEntry;
use crate::widgets::file_picker::{pick_file_dialog, pick_files_dialog};
use crate::widgets::progress::ProgressBar;
use crate::widgets::status_list::StatusList;

struct VerifyResult {
    files: Vec<FileEntry>,
    summary: String,
}

struct VerifyWorker {
    progress_pct: f32,
    message: String,
    finished: bool,
    result: Option<VerifyResult>,
}

pub struct VerifyTab {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub extra_files: Vec<PathBuf>,
    pub is_running: bool,
    pub worker: Option<Arc<Mutex<VerifyWorker>>>,
    pub progress_bar: ProgressBar,
    pub status_list: StatusList,
    pub result_summary: Option<String>,
}

impl Default for VerifyTab {
    fn default() -> Self {
        Self {
            par2_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            extra_files: Vec::new(),
            is_running: false,
            worker: None,
            progress_bar: ProgressBar::default(),
            status_list: StatusList::default(),
            result_summary: None,
        }
    }
}

impl VerifyTab {
    /// Returns true if the user clicked "Switch to Repair"
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) -> bool {
        let mut repair_requested = false;

        // Poll worker
        if self.is_running {
            if let Some(worker) = &self.worker {
                let mut guard = worker.lock().unwrap();
                self.progress_bar.progress = guard.progress_pct;
                self.progress_bar.label.clone_from(&guard.message);
                if guard.finished {
                    self.is_running = false;
                    if let Some(result) = guard.result.take() {
                        self.status_list.files = result.files;
                        self.result_summary = Some(result.summary);
                    }
                }
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("✅ Verify File Integrity");
            });
            ui.separator();

            egui::CollapsingHeader::new("📁 Files")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("PAR2 file:");
                        if ui.button("📂 Browse").clicked() {
                            if let Some(path) = pick_file_dialog("Select PAR2 file", Some("*.par2")) {
                                self.par2_path = path.clone();
                                if let Some(parent) = path.parent() {
                                    self.data_dir = parent.to_path_buf();
                                }
                            }
                        }
                    });
                    ui.label(
                        egui::RichText::new(&self.par2_path.to_string_lossy().to_string())
                            .size(11.0)
                            .italics()
                            .color(ui.visuals().weak_text_color()),
                    );

                    // Show discovered related PAR2 files
                    if !self.par2_path.as_os_str().is_empty() {
                        if let Some(parent) = self.par2_path.parent() {
                            if let Ok(entries) = std::fs::read_dir(parent) {
                                let related: Vec<_> = entries
                                    .filter_map(|e| e.ok())
                                    .map(|e| e.path())
                                    .filter(|p| {
                                        p.extension().and_then(|e| e.to_str()) == Some("par2")
                                            && p != &self.par2_path
                                    })
                                    .collect();
                                if !related.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "🔗 Found {} related PAR2 files in same directory",
                                            related.len()
                                        ))
                                        .size(11.0)
                                        .italics()
                                        .color(egui::Color32::LIGHT_BLUE),
                                    );
                                    egui::ScrollArea::vertical().max_height(60.0).show(ui, |ui| {
                                        for p in &related {
                                            ui.label(egui::RichText::new(p.to_string_lossy()).size(10.0).family(egui::FontFamily::Monospace));
                                        }
                                    });
                                }
                            }
                        }
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Data directory:");
                        if ui.button("📂 Browse").clicked() {
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Select data directory")
                                .pick_folder()
                            {
                                self.data_dir = dir;
                            }
                        }
                    });
                    ui.label(
                        egui::RichText::new(&self.data_dir.to_string_lossy().to_string())
                            .size(11.0)
                            .italics()
                            .color(ui.visuals().weak_text_color()),
                    );

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Extra files (optional):");
                        if ui.button("➕ Add").clicked() {
                            let files = pick_files_dialog("Select extra files to check", None);
                            self.extra_files.extend(files);
                        }
                        if !self.extra_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                            self.extra_files.clear();
                        }
                    });
                    if !self.extra_files.is_empty() {
                        ui.label(format!("{} extra files", self.extra_files.len()));
                    }
                });

            ui.separator();

            ui.horizontal(|ui| {
                let can_run = !self.par2_path.as_os_str().is_empty()
                    && !self.data_dir.as_os_str().is_empty()
                    && !self.is_running;
                let btn = ui.add_enabled(
                    can_run,
                    egui::Button::new(egui::RichText::new("🔍 Verify").size(16.0)),
                );
                if btn.clicked() {
                    self.start_verify();
                }
                if self.is_running {
                    ui.spinner();
                    ui.label("Verifying...");
                }
            });

            if self.is_running {
                ui.add_space(6.0);
                self.progress_bar.show(ui);
            }

            if !self.is_running && self.status_list.files.is_empty() && self.result_summary.is_some() {
                if let Some(summary) = &self.result_summary {
                    ui.add_space(6.0);
                    ui.label(summary);
                }
            }

            if !self.status_list.files.is_empty() {
                ui.add_space(6.0);
                self.status_list.show(ui);
                if let Some(summary) = &self.result_summary {
                    ui.separator();
                    ui.label(egui::RichText::new(summary).size(12.0));
                }

                // Show repair button if damage detected
                let has_damage = self.status_list.files.iter()
                    .any(|f| matches!(f.status, crate::par2::types::FileStatus::Damaged | crate::par2::types::FileStatus::Missing));
                if has_damage {
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new("🔧 Switch to Repair").size(14.0)).clicked() {
                            repair_requested = true;
                        }
                    });
                }

                ui.horizontal(|ui| {
                    if ui.small_button("🔄 Clear Results").clicked() {
                        self.status_list.files.clear();
                        self.result_summary = None;
                    }
                });
            }
        });

        repair_requested
    }

    fn start_verify(&mut self) {
        self.is_running = true;
        self.result_summary = None;
        self.status_list.files.clear();
        self.progress_bar = ProgressBar {
            progress: 0.0,
            max: 100.0,
            label: "Verifying files...".to_string(),
            show_percentage: true,
        };

        let worker = Arc::new(Mutex::new(VerifyWorker {
            progress_pct: 0.0,
            message: "Parsing PAR2 file...".to_string(),
            finished: false,
            result: None,
        }));

        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();
        let worker_clone = worker.clone();

        std::thread::spawn(move || {
            {
                let mut w = worker_clone.lock().unwrap();
                w.progress_pct = 20.0;
                w.message = "Verifying files...".to_string();
            }

            let vresult = verify::verify(&par2, &dir, &extras);

            let files = vresult.files;
            let summary = vresult.summary.clone();

            {
                let mut w = worker_clone.lock().unwrap();
                w.progress_pct = 100.0;
                w.message.clone_from(&summary);
                w.finished = true;
                w.result = Some(VerifyResult { files, summary });
            }
        });

        self.worker = Some(worker);
    }
}
