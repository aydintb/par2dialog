use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::par2::create;
use crate::par2::types::{CreateConfig, RedundancyMode, RecoveryFileMode, SizeUnit};
use crate::theme;
use crate::widgets::file_picker::{pick_files_dialog, pick_directory_dialog, save_file_dialog};
use crate::widgets::progress::ProgressBar;

struct CreateWorker {
    progress_pct: f32,
    message: String,
    finished: bool,
    success: bool,
}

pub struct CreateTab {
    // File selection
    pub output_path: PathBuf,
    pub data_files: Vec<PathBuf>,

    // Redundancy
    pub redundancy_mode: RedundancyMode,
    pub redundancy_value: f64,

    // Block settings
    pub block_size: String,
    pub block_count: String,
    pub use_block_size: bool,

    // Recovery files
    pub recovery_file_mode: RecoveryFileMode,
    pub recovery_file_count: u64,

    // Advanced
    pub memory_limit: String,
    pub threads: String,
    pub hash_threads: String,
    pub recurse: bool,
    pub base_path: Option<PathBuf>,
    pub purge_on_success: bool,
    pub first_recovery_block: String,

    // State
    pub is_running: bool,
    pub done_result: Option<Result<String, String>>,
    pub worker: Option<Arc<Mutex<CreateWorker>>>,
    pub progress_bar: ProgressBar,
    pub log_lines: Vec<String>,
}

impl Default for CreateTab {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            data_files: Vec::new(),
            redundancy_mode: RedundancyMode::Percentage,
            redundancy_value: 5.0,
            block_size: String::new(),
            block_count: String::new(),
            use_block_size: true,
            recovery_file_mode: RecoveryFileMode::Auto,
            recovery_file_count: 4,
            memory_limit: String::new(),
            threads: String::new(),
            hash_threads: String::new(),
            recurse: false,
            base_path: None,
            purge_on_success: false,
            first_recovery_block: String::new(),
            is_running: false,
            done_result: None,
            worker: None,
            progress_bar: ProgressBar::default(),
            log_lines: Vec::new(),
        }
    }
}

impl CreateTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        // Poll worker
        if self.is_running {
            if let Some(worker) = &self.worker {
                let guard = worker.lock().unwrap();
                self.progress_bar.progress = guard.progress_pct;
                self.progress_bar.label.clone_from(&guard.message);
                if guard.finished {
                    self.is_running = false;
                    if guard.success {
                        self.done_result = Some(Ok(guard.message.clone()));
                    } else {
                        self.done_result = Some(Err(guard.message.clone()));
                    }
                }
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("📦 Create PAR2 Recovery Files");
            });
            ui.separator();

            // ── File Selection ──
            egui::CollapsingHeader::new("📁 File Selection")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Data files to protect:");
                        if ui.button("➕ Add Files").clicked() {
                            let files = pick_files_dialog("Select data files", None);
                            self.data_files.extend(files);
                        }
                        if ui.button("📁 Add Directory").clicked() {
                            if let Some(dir) = pick_directory_dialog("Select data directory") {
                                // Auto-set output to dir_name.par2 inside the directory
                                if self.output_path.as_os_str().is_empty() {
                                    if let Some(dir_name) = dir.file_name() {
                                        self.output_path = dir.join(format!("{}.par2", dir_name.to_string_lossy()));
                                    } else {
                                        self.output_path = dir.join("output.par2");
                                    }
                                }
                                if let Ok(entries) = std::fs::read_dir(&dir) {
                                    for entry in entries.flatten() {
                                        if entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
                                            self.data_files.push(entry.path());
                                        }
                                    }
                                }
                            }
                        }
                        if !self.data_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                            self.data_files.clear();
                        }
                    });
                    if !self.data_files.is_empty() {
                        // Auto-set output path if not already set
                        if self.output_path.as_os_str().is_empty() {
                            if let Some(first_file) = self.data_files.first() {
                                if let Some(parent) = first_file.parent() {
                                    let name = first_file.file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "output".to_string());
                                    self.output_path = parent.join(format!("{name}.par2"));
                                }
                            }
                        }

                        let total_size: u64 = self.data_files.iter()
                            .filter_map(|f| f.metadata().ok().map(|m| m.len()))
                            .sum();
                        ui.label(format!("{} files ({})", self.data_files.len(), theme::human_size(total_size)));
                        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                            let mut to_remove = None;
                            for (i, file) in self.data_files.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{i:>3}. {}", theme::human_size(file.metadata().map(|m| m.len()).unwrap_or(0))));
                                    ui.label(theme::tooltip_text(&file.to_string_lossy()));
                                    if ui.small_button("✖").clicked() {
                                        to_remove = Some(i);
                                    }
                                });
                            }
                            if let Some(idx) = to_remove {
                                self.data_files.remove(idx);
                            }
                        });
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Output PAR2 file:");
                        if ui.button("📂 Browse").clicked() {
                            if let Some(path) = save_file_dialog("Save PAR2 file as", Some("par2"), None) {
                                self.output_path = path;
                            }
                        }
                    });
                    ui.label(
                        egui::RichText::new(&self.output_path.to_string_lossy().to_string())
                            .size(11.0)
                            .italics()
                            .color(ui.visuals().weak_text_color()),
                    );
                });

            // ── Redundancy ──
            egui::CollapsingHeader::new("🔒 Redundancy")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Mode:");
                        ui.radio_value(&mut self.redundancy_mode, RedundancyMode::Percentage, "Percentage");
                        ui.radio_value(&mut self.redundancy_mode, RedundancyMode::Size { unit: SizeUnit::MB, value: 0 }, "Target Size");
                        ui.radio_value(&mut self.redundancy_mode, RedundancyMode::BlockCount, "Block Count");
                    });
                    match &mut self.redundancy_mode {
                        RedundancyMode::Percentage => {
                            ui.horizontal(|ui| {
                                ui.add(egui::Slider::new(&mut self.redundancy_value, 1.0..=200.0).text("Redundancy"));
                                ui.label(egui::RichText::new(format!("{}%", self.redundancy_value as u64)).strong());
                            });
                        }
                        RedundancyMode::Size { unit, value } => {
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(value).range(0..=100_000).speed(1.0).prefix("Target: "));
                                ui.radio_value(unit, SizeUnit::KB, "KB");
                                ui.radio_value(unit, SizeUnit::MB, "MB");
                                ui.radio_value(unit, SizeUnit::GB, "GB");
                            });
                        }
                        RedundancyMode::BlockCount => {
                            ui.add(egui::DragValue::new(&mut self.redundancy_value).range(0..=1_000_000).speed(10.0).prefix("Recovery blocks: "));
                        }
                    }
                });

            // ── Blocks ──
            egui::CollapsingHeader::new("🧱 Blocks").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.radio(self.use_block_size, "Block Size:").clicked() {
                        self.use_block_size = true;
                    }
                    if self.use_block_size {
                        ui.add(egui::DragValue::new(&mut self.block_size.parse::<u64>().unwrap_or(0)).range(0..=1_000_000_000).speed(1024.0).suffix(" B"));
                        ui.label(egui::RichText::new("(0 = auto)").size(10.0).italics());
                    }
                });
                ui.horizontal(|ui| {
                    if ui.radio(!self.use_block_size, "Block Count:").clicked() {
                        self.use_block_size = false;
                    }
                    if !self.use_block_size {
                        ui.add(egui::DragValue::new(&mut self.block_count.parse::<u64>().unwrap_or(0)).range(0..=1_000_000).speed(100.0));
                        ui.label(egui::RichText::new("(default 2000)").size(10.0).italics());
                    }
                });
            });

            // ── Recovery Files ──
            egui::CollapsingHeader::new("📂 Recovery Files").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Sizing:");
                    ui.radio_value(&mut self.recovery_file_mode, RecoveryFileMode::Auto, "Auto");
                    ui.radio_value(&mut self.recovery_file_mode, RecoveryFileMode::Uniform, "Uniform");
                    ui.radio_value(&mut self.recovery_file_mode, RecoveryFileMode::LimitSize, "Limit Size");
                    ui.radio_value(&mut self.recovery_file_mode, RecoveryFileMode::ExactCount, "Exact Count");
                });
                if self.recovery_file_mode == RecoveryFileMode::ExactCount {
                    ui.add(egui::DragValue::new(&mut self.recovery_file_count).range(1..=31).prefix("Count: "));
                }
            });

            // ── Advanced ──
            egui::CollapsingHeader::new("⚙️ Advanced").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.memory_limit).desired_width(80.0).hint_text("Auto"));
                    ui.label("Memory (MB)").on_hover_text("Memory limit in MB (default: 50% RAM)");
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.threads).desired_width(80.0).hint_text("Auto"));
                    ui.label("Threads").on_hover_text("Processing threads (default: auto)");
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.hash_threads).desired_width(80.0).hint_text("2"));
                    ui.label("Hash threads").on_hover_text("Parallel hashing threads (default: 2)");
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.first_recovery_block).desired_width(80.0).hint_text("0"));
                    ui.label("First recovery block").on_hover_text("Starting block for appending recovery data");
                });
                ui.separator();
                ui.checkbox(&mut self.recurse, "Recurse subdirectories (-R)").on_hover_text("Include files from nested directories");
                ui.checkbox(&mut self.purge_on_success, "Purge on success (-p)").on_hover_text("Delete PAR2 files after successful verification");
                ui.horizontal(|ui| {
                    ui.label("Base path:");
                    if ui.button("📂").clicked() {
                        if let Some(dir) = pick_directory_dialog("Select base path") {
                            self.base_path = Some(dir);
                        }
                    }
                    if let Some(bp) = &self.base_path {
                        ui.label(theme::tooltip_text(&bp.to_string_lossy()));
                        if ui.small_button("✖").clicked() { self.base_path = None; }
                    }
                });
            });

            ui.separator();

            // ── Action ──
            ui.horizontal(|ui| {
                let can_run = !self.output_path.as_os_str().is_empty()
                    && !self.data_files.is_empty()
                    && !self.is_running;
                let btn = ui.add_enabled(
                    can_run,
                    egui::Button::new(egui::RichText::new("🚀 Create PAR2 Files").size(16.0)),
                );
                if btn.clicked() {
                    self.start_creation();
                }
                if self.is_running {
                    ui.spinner();
                    ui.label("Working...");
                }
            });

            // ── Progress ──
            if self.is_running {
                ui.add_space(6.0);
                self.progress_bar.show(ui);
            }

            if let Some(Ok(msg)) = &self.done_result {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::GREEN, format!("✅ {msg}"));
                if ui.small_button("🔄 Reset").clicked() { self.done_result = None; }
            }
            if let Some(Err(msg)) = &self.done_result {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::RED, format!("❌ {msg}"));
                if ui.small_button("🔄 Reset").clicked() { self.done_result = None; }
            }
        });
    }

    fn start_creation(&mut self) {
        self.is_running = true;
        self.done_result = None;
        self.log_lines.clear();
        self.progress_bar = ProgressBar {
            progress: 0.0,
            max: 100.0,
            label: "Starting par2cmdline...".to_string(),
            show_percentage: true,
        };

        let worker = Arc::new(Mutex::new(CreateWorker {
            progress_pct: 0.0,
            message: "Initializing...".to_string(),
            finished: false,
            success: false,
        }));

        let worker_clone = worker.clone();

        let config = CreateConfig {
            output_path: self.output_path.clone(),
            data_files: self.data_files.clone(),
            redundancy_value: self.redundancy_value,
            redundancy_mode: self.redundancy_mode.clone(),
            block_size: self.block_size.parse().unwrap_or(0),
            block_count: if !self.use_block_size { self.block_count.parse().unwrap_or(0) } else { 0 },
            recovery_file_count: self.recovery_file_count,
            recovery_file_mode: self.recovery_file_mode.clone(),
            memory_limit: self.memory_limit.parse().ok(),
            threads: self.threads.parse().ok(),
            hash_threads: self.hash_threads.parse().ok(),
            recurse: self.recurse,
            base_path: self.base_path.clone(),
            purge_on_success: self.purge_on_success,
            first_recovery_block: self.first_recovery_block.parse().ok(),
        };

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel::<crate::par2::types::ProgressUpdate>();

            // Clone for the result handler
            let worker_result = worker_clone.clone();

            // Spawn progress reader
            std::thread::spawn(move || {
                for update in rx {
                    let mut w = worker_clone.lock().unwrap();
                    w.progress_pct = update.current;
                    w.message.clone_from(&update.message);
                    w.finished = update.done;
                    w.success = update.done && update.current >= 100.0;
                    if update.done {
                        break;
                    }
                }
            });

            let result = create::create_async(config, tx).join();
            let mut w = worker_result.lock().unwrap();
            match result {
                Ok(Ok(msg)) => {
                    w.message = msg;
                    w.finished = true;
                    w.success = true;
                    w.progress_pct = 100.0;
                }
                Ok(Err(e)) => {
                    w.message = e;
                    w.finished = true;
                    w.success = false;
                }
                Err(_) => {
                    w.message = "Creation thread panicked".to_string();
                    w.finished = true;
                    w.success = false;
                }
            }
        });

        self.worker = Some(worker);
    }
}
