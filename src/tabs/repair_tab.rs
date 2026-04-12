use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::par2::repair::{check_feasibility, verify_then_repair};
use crate::par2::types::FileEntry;
use crate::widgets::file_picker::{pick_file_dialog, pick_files_dialog};
use crate::widgets::progress::ProgressBar;
use crate::widgets::status_list::StatusList;

struct WorkerResult {
    files: Vec<FileEntry>,
    summary: String,
}

struct RepairWorker {
    progress_pct: f32,
    message: String,
    finished: bool,
    success: bool,
    result: Option<WorkerResult>,
}

pub struct RepairTab {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub extra_files: Vec<PathBuf>,
    pub is_running: bool,
    pub is_checking: bool,
    pub worker: Option<Arc<Mutex<RepairWorker>>>,
    pub progress_bar: ProgressBar,
    pub status_list: StatusList,
    pub result_summary: Option<String>,
    pub feasibility_check: Option<String>,
}

impl Default for RepairTab {
    fn default() -> Self {
        Self {
            par2_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            extra_files: Vec::new(),
            is_running: false,
            is_checking: false,
            worker: None,
            progress_bar: ProgressBar::default(),
            status_list: StatusList::default(),
            result_summary: None,
            feasibility_check: None,
        }
    }
}

impl RepairTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        // Poll worker
        if self.is_running || self.is_checking {
            if let Some(worker) = &self.worker {
                let mut guard = worker.lock().unwrap();
                self.progress_bar.progress = guard.progress_pct;
                self.progress_bar.label.clone_from(&guard.message);
                if guard.finished {
                    if self.is_checking {
                        self.is_checking = false;
                        self.feasibility_check = Some(guard.message.clone());
                    } else {
                        self.is_running = false;
                        if let Some(result) = guard.result.take() {
                            self.status_list.files = result.files;
                            self.result_summary = Some(result.summary);
                        }
                    }
                }
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("🔧 Repair Damaged Files");
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
                        ui.label("Extra files:");
                        if ui.button("➕ Add").clicked() {
                            let files = pick_files_dialog("Select extra files", None);
                            self.extra_files.extend(files);
                        }
                        if !self.extra_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                            self.extra_files.clear();
                        }
                    });
                });

            ui.separator();

            ui.horizontal(|ui| {
                let can_run = !self.par2_path.as_os_str().is_empty()
                    && !self.data_dir.as_os_str().is_empty()
                    && !self.is_running
                    && !self.is_checking;

                if ui.add_enabled(can_run, egui::Button::new("🔍 Check Feasibility")).clicked() {
                    self.start_feasibility_check();
                }

                if ui.add_enabled(can_run, egui::Button::new(egui::RichText::new("🔧 Repair").size(14.0))).clicked() {
                    self.start_repair();
                }

                if self.is_running || self.is_checking {
                    ui.spinner();
                    ui.label(if self.is_checking { "Checking..." } else { "Repairing..." });
                }
            });

            // Feasibility result
            if let Some(ref feasibility_msg) = self.feasibility_check {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📊 Feasibility:").strong());
                    ui.label(feasibility_msg);
                });
                if ui.small_button("✖ Clear").clicked() {
                    self.feasibility_check = None;
                }
            }

            if self.is_running || self.is_checking {
                ui.add_space(6.0);
                self.progress_bar.show(ui);
            }

            if let Some(summary) = &self.result_summary {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(summary).size(12.0));
            }
            if !self.status_list.files.is_empty() {
                ui.add_space(6.0);
                self.status_list.show(ui);
            }
        });
    }

    fn start_feasibility_check(&mut self) {
        self.is_checking = true;
        self.feasibility_check = None;
        self.progress_bar = ProgressBar {
            progress: 0.0, max: 100.0,
            label: "Checking repair feasibility...".to_string(),
            show_percentage: true,
        };

        let worker = Arc::new(Mutex::new(RepairWorker {
            progress_pct: 0.0, message: "Checking...".to_string(),
            finished: false, success: false, result: None,
        }));

        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();
        let worker_clone = worker.clone();

        std::thread::spawn(move || {
            let result = check_feasibility(&par2, &dir, &extras);
            let msg = if result.all_correct {
                "All files are intact. No repair needed.".to_string()
            } else if result.repair_possible {
                format!(
                    "✅ Repair is possible. {} blocks available, {} needed.",
                    result.blocks_available, result.blocks_needed
                )
            } else {
                format!(
                    "❌ Repair NOT possible. Need {} more blocks.",
                    result.blocks_needed.saturating_sub(result.blocks_available)
                )
            };

            let files = result.files;

            {
                let mut w = worker_clone.lock().unwrap();
                w.progress_pct = 100.0;
                w.message.clone_from(&msg);
                w.finished = true;
                w.result = Some(WorkerResult { files, summary: msg });
            }
        });

        self.worker = Some(worker);
    }

    fn start_repair(&mut self) {
        self.is_running = true;
        self.result_summary = None;
        self.status_list.files.clear();
        self.progress_bar = ProgressBar {
            progress: 0.0, max: 100.0,
            label: "Starting repair...".to_string(),
            show_percentage: true,
        };

        let worker = Arc::new(Mutex::new(RepairWorker {
            progress_pct: 0.0, message: "Verifying before repair...".to_string(),
            finished: false, success: false, result: None,
        }));

        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();
        let worker_clone = worker.clone();

        std::thread::spawn(move || {
            let (verify_result, repair_result) = verify_then_repair(&par2, &dir, &extras);

            let msg = format!(
                "Verify: {}\nRepair: {}",
                verify_result.summary, repair_result.summary
            );

            let files = verify_result.files;

            {
                let mut w = worker_clone.lock().unwrap();
                w.progress_pct = 100.0;
                w.message.clone_from(&msg);
                w.finished = true;
                w.success = repair_result.success;
                w.result = Some(WorkerResult { files, summary: msg });
            }
        });

        self.worker = Some(worker);
    }
}
