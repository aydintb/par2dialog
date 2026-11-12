use std::path::PathBuf;

use eframe::egui;

use crate::app::SharedState;
use crate::jobs::Job;
use crate::par2::create;
use crate::par2::types::{
    BlockSpec, CreateConfig, RecoveryFileMode, RedundancySpec, SizeUnit,
};
use crate::theme;
use crate::util;
use crate::widgets::file_picker::{pick_directory_dialog, pick_files_dialog, save_file_dialog};
use crate::widgets::progress;

/// Which redundancy input the user is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RedundancyUiMode {
    Percentage,
    TargetSize,
    BlockCount,
}

pub struct CreateTab {
    // File selection
    pub output_path: PathBuf,
    pub data_files: Vec<PathBuf>,

    // Redundancy
    mode: RedundancyUiMode,
    redundancy_percent: u64,
    size_value: u64,
    size_unit: SizeUnit,
    recovery_blocks: u64,

    // Blocks
    use_block_size: bool,
    block_size: u64,
    block_count: u64,

    // Recovery files
    recovery_file_mode: RecoveryFileMode,

    // Advanced
    memory_limit: String,
    threads: String,
    hash_threads: String,
    first_recovery_block: String,
    recurse: bool,
    base_path: Option<PathBuf>,

    // Job state
    job: Option<Job<Result<String, String>>>,
    done_result: Option<Result<String, String>>,
}

impl Default for CreateTab {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            data_files: Vec::new(),
            mode: RedundancyUiMode::Percentage,
            redundancy_percent: 5,
            size_value: 100,
            size_unit: SizeUnit::MB,
            recovery_blocks: 2000,
            use_block_size: false,
            block_size: 0,
            block_count: 2000,
            recovery_file_mode: RecoveryFileMode::Auto,
            memory_limit: String::new(),
            threads: String::new(),
            hash_threads: String::new(),
            first_recovery_block: String::new(),
            recurse: false,
            base_path: None,
            job: None,
            done_result: None,
        }
    }
}

impl CreateTab {
    /// Overwrite the "advanced" inputs with the defaults from the settings.
    pub fn apply_defaults(&mut self, config: &crate::app_config::AppConfig) {
        self.redundancy_percent = config.default_redundancy_percent;
        self.threads.clone_from(&config.default_threads);
        self.hash_threads.clone_from(&config.default_hash_threads);
        self.memory_limit.clone_from(&config.default_memory_limit);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, shared: &mut SharedState) {
        // Poll the background job.
        let mut finished = None;
        if let Some(job) = &self.job {
            if !job.is_running() {
                finished = job.take_result();
            }
        }
        if let Some(result) = finished {
            self.job = None;
            self.done_result = Some(result);
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("📦 Create PAR2 Recovery Files");
            });
            ui.separator();

            // ── File Selection ──
            egui::CollapsingHeader::new("📁 File Selection")
                .default_open(true)
                .show(ui, |ui| self.file_selection_section(ui));

            // ── Redundancy ──
            egui::CollapsingHeader::new("🔒 Redundancy")
                .default_open(true)
                .show(ui, |ui| self.redundancy_section(ui));

            // ── Blocks ──
            egui::CollapsingHeader::new("🧱 Blocks")
                .show(ui, |ui| self.blocks_section(ui));

            // ── Recovery Files ──
            egui::CollapsingHeader::new("📂 Recovery Files")
                .show(ui, |ui| self.recovery_files_section(ui));

            // ── Advanced ──
            egui::CollapsingHeader::new("⚙️ Advanced")
                .show(ui, |ui| self.advanced_section(ui));

            ui.separator();

            // ── Action ──
            self.action_section(ui, shared);
        });
    }

    fn file_selection_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Data files to protect:");
            if ui.button("➕ Add Files").clicked() {
                self.data_files
                    .extend(pick_files_dialog("Select data files", None));
            }
            if ui
                .add_enabled(
                    !self.is_running(),
                    egui::Button::new("📁 Add Directory"),
                )
                .clicked()
            {
                if let Some(dir) = pick_directory_dialog("Select data directory") {
                    self.add_directory(&dir);
                }
            }
            if !self.data_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                self.data_files.clear();
            }
        });

        if !self.data_files.is_empty() {
            if self.output_path.as_os_str().is_empty() {
                self.auto_fill_output();
            }

            let total_size: u64 = self
                .data_files
                .iter()
                .filter_map(|f| f.metadata().ok().map(|m| m.len()))
                .sum();
            ui.label(format!(
                "{} files ({})",
                self.data_files.len(),
                util::human_size(total_size)
            ));
            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                let mut to_remove = None;
                for (i, file) in self.data_files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{i:>3}. {}",
                            util::human_size(file.metadata().map_or(0, |m| m.len()))
                        ));
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
            egui::RichText::new(self.output_path.to_string_lossy().to_string())
                .size(11.0)
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
    }

    fn redundancy_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Mode:");
            ui.radio_value(&mut self.mode, RedundancyUiMode::Percentage, "Percentage");
            ui.radio_value(&mut self.mode, RedundancyUiMode::TargetSize, "Target Size");
            ui.radio_value(&mut self.mode, RedundancyUiMode::BlockCount, "Block Count");
        });
        match self.mode {
            RedundancyUiMode::Percentage => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.redundancy_percent, 1..=200)
                            .text("Redundancy"),
                    );
                    ui.label(egui::RichText::new("%").strong());
                });
            }
            RedundancyUiMode::TargetSize => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.size_value)
                            .range(1..=1_000_000)
                            .speed(1.0)
                            .prefix("Target: "),
                    );
                    ui.radio_value(&mut self.size_unit, SizeUnit::KB, "KB");
                    ui.radio_value(&mut self.size_unit, SizeUnit::MB, "MB");
                    ui.radio_value(&mut self.size_unit, SizeUnit::GB, "GB");
                });
            }
            RedundancyUiMode::BlockCount => {
                ui.add(
                    egui::DragValue::new(&mut self.recovery_blocks)
                        .range(1..=1_000_000)
                        .speed(10.0)
                        .prefix("Recovery blocks: "),
                );
            }
        }
    }

    fn blocks_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.radio(self.use_block_size, "Block Size:").clicked() {
                self.use_block_size = true;
            }
            if self.use_block_size {
                ui.add(
                    egui::DragValue::new(&mut self.block_size)
                        .range(0..=1_000_000_000)
                        .speed(1024.0)
                        .suffix(" B"),
                );
                ui.label(egui::RichText::new("(0 = auto)").size(10.0).italics());
            }
        });
        ui.horizontal(|ui| {
            if ui.radio(!self.use_block_size, "Block Count:").clicked() {
                self.use_block_size = false;
            }
            if !self.use_block_size {
                ui.add(
                    egui::DragValue::new(&mut self.block_count)
                        .range(0..=1_000_000)
                        .speed(100.0),
                );
                ui.label(egui::RichText::new("(0 = default 2000)").size(10.0).italics());
            }
        });
    }

    fn recovery_files_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Sizing:");
            ui.radio_value(&mut self.recovery_file_mode, RecoveryFileMode::Auto, "Auto");
            ui.radio_value(
                &mut self.recovery_file_mode,
                RecoveryFileMode::Uniform,
                "Uniform",
            );
            ui.radio_value(
                &mut self.recovery_file_mode,
                RecoveryFileMode::LimitSize,
                "Limit Size",
            );
            ui.radio_value(
                &mut self.recovery_file_mode,
                RecoveryFileMode::ExactCount(0),
                "Exact Count",
            );
        });
        if let RecoveryFileMode::ExactCount(count) = &mut self.recovery_file_mode {
            ui.add(egui::DragValue::new(count).range(1..=31).prefix("Count: "));
        }
    }

    fn advanced_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.memory_limit)
                    .desired_width(80.0)
                    .hint_text("Auto"),
            );
            ui.label("Memory (MB)")
                .on_hover_text("Memory limit in MB (default: 50% RAM)");
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.threads)
                    .desired_width(80.0)
                    .hint_text("Auto"),
            );
            ui.label("Threads")
                .on_hover_text("Processing threads (default: auto)");
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.hash_threads)
                    .desired_width(80.0)
                    .hint_text("2"),
            );
            ui.label("Hash threads")
                .on_hover_text("Files hashed in parallel (default: 2)");
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.first_recovery_block)
                    .desired_width(80.0)
                    .hint_text("0"),
            );
            ui.label("First recovery block")
                .on_hover_text("Starting block for appending recovery data");
        });
        ui.separator();
        ui.checkbox(&mut self.recurse, "Recurse subdirectories (-R)")
            .on_hover_text("Include files from nested directories");
        ui.horizontal(|ui| {
            ui.label("Base path:");
            if ui.button("📂").clicked() {
                if let Some(dir) = pick_directory_dialog("Select base path") {
                    self.base_path = Some(dir);
                }
            }
            if let Some(bp) = &self.base_path {
                ui.label(theme::tooltip_text(&bp.to_string_lossy()));
                if ui.small_button("✖").clicked() {
                    self.base_path = None;
                }
            }
        });
    }

    fn action_section(&mut self, ui: &mut egui::Ui, shared: &mut SharedState) {
        ui.horizontal(|ui| {
            let can_run = !self.output_path.as_os_str().is_empty()
                && !self.data_files.is_empty()
                && !self.is_running();
            let btn = ui.add_enabled(
                can_run,
                egui::Button::new(egui::RichText::new("🚀 Create PAR2 Files").size(16.0)),
            );
            if btn.clicked() {
                self.start_creation(shared);
            }
            if self.is_running() {
                ui.spinner();
                ui.label("Working...");
            }
        });

        if let Some(job) = &self.job {
            ui.add_space(6.0);
            progress::bar(ui, &job.progress());
        }

        match &self.done_result {
            Some(Ok(msg)) => {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::GREEN, format!("✅ {msg}"));
                self.success_panel(ui);
                if ui.small_button("🔄 Reset").clicked() {
                    self.done_result = None;
                }
            }
            Some(Err(msg)) => {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::RED, format!("❌ {msg}"));
                if ui.small_button("🔄 Reset").clicked() {
                    self.done_result = None;
                }
            }
            None => {}
        }
    }

    fn is_running(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }

    /// After a successful create: quick access to the output plus the
    /// generated `.par2` files.
    fn success_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .small_button("📂 Open output folder")
                .on_hover_text("Show the generated files in the file manager")
                .clicked()
            {
                crate::util::open_in_file_manager(&self.output_path);
            }
        });

        // List the PAR2 files that were just created (stem-matched).
        if let Some(parent) = self.output_path.parent() {
            if let Some(stem) = self.output_path.file_stem() {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    let created: Vec<PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.file_stem()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| {
                                    s.starts_with(stem.to_string_lossy().as_ref())
                                })
                                && p.extension().and_then(|e| e.to_str()) == Some("par2")
                        })
                        .collect();
                    if created.len() > 1 {
                        ui.indent("created_files", |ui| {
                            ui.label(theme::tooltip_text(&format!(
                                "Generated {} files:",
                                created.len()
                            )));
                            for p in &created {
                                ui.label(
                                    egui::RichText::new(p.to_string_lossy())
                                        .size(10.0)
                                        .family(egui::FontFamily::Monospace),
                                );
                            }
                        });
                    }
                }
            }
        }
    }

    /// Add all files below `dir` (recursive when the recurse option is on),
    /// skipping PAR2 files so parity data is never fed back as input.
    fn add_directory(&mut self, dir: &PathBuf) {
        if self.output_path.as_os_str().is_empty() {
            match dir.file_name() {
                Some(dir_name) => {
                    self.output_path = dir.join(format!("{}.par2", dir_name.to_string_lossy()));
                }
                None => self.output_path = dir.join("output.par2"),
            }
        }
        for path in collect_files(dir, self.recurse) {
            if !self.data_files.contains(&path) {
                self.data_files.push(path);
            }
        }
    }

    /// Derive a default output path from the first selected data file.
    fn auto_fill_output(&mut self) {
        if let Some(first_file) = self.data_files.first() {
            if let Some(parent) = first_file.parent() {
                let name = first_file.file_stem().map_or_else(
                    || "output".to_string(),
                    |s| s.to_string_lossy().to_string(),
                );
                self.output_path = parent.join(format!("{name}.par2"));
            }
        }
    }

    fn start_creation(&mut self, shared: &SharedState) {
        let config = CreateConfig {
            output_path: self.output_path.clone(),
            data_files: self.data_files.clone(),
            redundancy: match self.mode {
                RedundancyUiMode::Percentage => RedundancySpec::Percent(self.redundancy_percent),
                RedundancyUiMode::TargetSize => RedundancySpec::Size {
                    unit: self.size_unit,
                    value: self.size_value,
                },
                RedundancyUiMode::BlockCount => RedundancySpec::Blocks(self.recovery_blocks),
            },
            blocks: if self.use_block_size {
                if self.block_size > 0 {
                    BlockSpec::Size(self.block_size)
                } else {
                    BlockSpec::Auto
                }
            } else if self.block_count > 0 {
                BlockSpec::Count(self.block_count)
            } else {
                BlockSpec::Auto
            },
            recovery_file_mode: self.recovery_file_mode.clone(),
            memory_limit: self.memory_limit.parse().ok(),
            threads: self.threads.parse().ok(),
            hash_threads: self.hash_threads.parse().ok(),
            recurse: self.recurse,
            base_path: self.base_path.clone(),
            first_recovery_block: self.first_recovery_block.parse().ok(),
        };
        let par2_path = shared.par2_binary_override();

        self.done_result = None;
        self.job = Some(Job::spawn("Starting par2cmdline...", move |reporter| {
            create::create(&config, par2_path.as_deref(), reporter)
        }));
    }
}

/// List files under `dir`, optionally recursing; `.par2` files are skipped.
fn collect_files(dir: &PathBuf, recurse: bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("par2") {
            continue;
        }
        if path.is_dir() {
            if recurse {
                dirs.push(path);
            }
        } else {
            files.push(path);
        }
    }
    for sub in dirs {
        files.extend(collect_files(&sub, recurse));
    }
    files.sort();
    files
}
