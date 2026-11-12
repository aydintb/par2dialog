use std::path::PathBuf;

use eframe::egui;

use crate::app::SharedState;
use crate::jobs::Job;
use crate::par2::repair;
use crate::par2::types::{RepairOutcome, VerifyOutcome};
use crate::widgets::file_picker::{pick_file_dialog, pick_files_dialog, pick_directory_dialog};
use crate::widgets::par2_files::show_related_par2_files;
use crate::widgets::progress;
use crate::widgets::status_list::StatusList;

/// Outcome of a completed repair-tab job.
enum RepairJobResult {
    /// Result of "Check Feasibility".
    Feasibility {
        verify: VerifyOutcome,
        message: String,
        positive: bool,
    },
    /// Result of a full verify + repair run.
    Repair { verify: VerifyOutcome, repair: RepairOutcome },
}

pub struct RepairTab {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub extra_files: Vec<PathBuf>,
    job: Option<Job<Result<RepairJobResult, String>>>,
    /// Result of the last feasibility check: (message, positive).
    feasibility: Option<(String, bool)>,
    status_list: StatusList,
    /// Result summary: (message, `Some(positive)` for ok/error, `None` for a
    /// cancelled run).
    result_summary: Option<(String, Option<bool>)>,
}

impl Default for RepairTab {
    fn default() -> Self {
        Self {
            par2_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            extra_files: Vec::new(),
            job: None,
            feasibility: None,
            status_list: StatusList::default(),
            result_summary: None,
        }
    }
}

impl RepairTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _shared: &mut SharedState) {
        // Poll the background job.
        let mut finished = None;
        if let Some(job) = &self.job {
            if !job.is_running() {
                finished = job.take_result();
            }
        }
        if let Some(result) = finished {
            self.job = None;
            match result {
                Ok(job_result) => self.apply_job_result(job_result),
                Err(msg) => {
                    self.status_list.files.clear();
                    self.result_summary = Some(if msg == crate::jobs::CANCELLED {
                        (format!("⏹ {msg}"), None)
                    } else {
                        (format!("❌ {msg}"), Some(false))
                    });
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
                .show(ui, |ui| self.files_section(ui));

            ui.separator();

            self.action_section(ui);
            self.results_section(ui);
        });
    }

    fn apply_job_result(&mut self, result: RepairJobResult) {
        match result {
            RepairJobResult::Feasibility {
                verify,
                message,
                positive,
            } => {
                self.feasibility = Some((message, positive));
                self.status_list.files = verify.files;
            }
            RepairJobResult::Repair { verify, repair } => {
                self.status_list.files = verify.files;
                let message = if repair.success && repair.blocks_repaired > 0 {
                    format!(
                        "✅ Repair complete: {} blocks repaired across {} files.",
                        repair.blocks_repaired, repair.files_repaired
                    )
                } else if repair.success {
                    repair.summary.clone()
                } else {
                    format!("❌ {}", repair.summary)
                };
                self.result_summary = Some((message, Some(repair.success)));
            }
        }
    }

    fn files_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("PAR2 file:");
            if ui.button("📂 Browse").clicked() {
                if let Some(path) = pick_file_dialog("Select PAR2 file", Some("par2")) {
                    self.par2_path.clone_from(&path);
                    if let Some(parent) = path.parent() {
                        self.data_dir = parent.to_path_buf();
                    }
                }
            }
        });
        ui.label(
            egui::RichText::new(self.par2_path.to_string_lossy().to_string())
                .size(11.0)
                .italics()
                .color(ui.visuals().weak_text_color()),
        );

        show_related_par2_files(ui, &self.par2_path);

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Data directory:");
            if ui.button("📂 Browse").clicked() {
                if let Some(dir) = pick_directory_dialog("Select data directory") {
                    self.data_dir = dir;
                }
            }
        });
        ui.label(
            egui::RichText::new(self.data_dir.to_string_lossy().to_string())
                .size(11.0)
                .italics()
                .color(ui.visuals().weak_text_color()),
        );

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Extra files (optional):");
            if ui.button("➕ Add").clicked() {
                self.extra_files
                    .extend(pick_files_dialog("Select extra files", None));
            }
            if !self.extra_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                self.extra_files.clear();
            }
        });
    }

    fn action_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let can_run = !self.par2_path.as_os_str().is_empty()
                && !self.data_dir.as_os_str().is_empty()
                && !self.is_running();

            if ui
                .add_enabled(can_run, egui::Button::new("🔍 Check Feasibility"))
                .on_hover_text("See whether enough recovery data is available to repair")
                .clicked()
            {
                self.start_feasibility_check();
            }

            if ui
                .add_enabled(
                    can_run,
                    egui::Button::new(egui::RichText::new("🔧 Repair").size(14.0)),
                )
                .on_hover_text("Verify, then repair damaged or missing files")
                .clicked()
            {
                self.start_repair();
            }

            if self.is_running() {
                ui.spinner();
                ui.label("Working...");
                if ui
                    .button("⏹ Stop")
                    .on_hover_text(
                        "Stops immediately. If it stopped while a file was being \
                         repaired, simply run Repair again to finish — recovery data \
                         is never modified.",
                    )
                    .clicked()
                {
                    if let Some(job) = &self.job {
                        job.cancel();
                    }
                }
            }
        });

        if let Some(job) = &self.job {
            ui.add_space(6.0);
            progress::bar(ui, &job.progress());
        }
    }

    fn results_section(&mut self, ui: &mut egui::Ui) {
        // Feasibility result
        if let Some((message, positive)) = &self.feasibility {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("📊 Feasibility:").strong());
                ui.label(egui::RichText::new(message).color(color_for(*positive)));
            });
            if ui.small_button("✖ Clear").clicked() {
                self.feasibility = None;
            }
        }

        if let Some((summary, positive)) = &self.result_summary {
            ui.add_space(6.0);
            let color = match positive {
                Some(true) => egui::Color32::GREEN,
                Some(false) => egui::Color32::RED,
                None => ui.visuals().warn_fg_color,
            };
            ui.label(egui::RichText::new(summary).size(12.0).color(color));
        }
        if !self.status_list.files.is_empty() {
            ui.add_space(6.0);
            self.status_list.show(ui);
        }
    }

    fn is_running(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }

    fn start_feasibility_check(&mut self) {
        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();

        self.feasibility = None;
        self.job = Some(Job::spawn("Checking repair feasibility...", move |reporter| {
            let outcome = repair::check_feasibility(&par2, &dir, &extras, reporter)?;
            let message = if outcome.all_correct {
                "All files are intact. No repair needed.".to_string()
            } else if outcome.repair_possible {
                format!(
                    "✅ Repair is possible. {} blocks available, {} needed.",
                    outcome.blocks_available, outcome.blocks_needed
                )
            } else {
                format!(
                    "❌ Repair NOT possible. Need {} more blocks.",
                    outcome.blocks_needed.saturating_sub(outcome.blocks_available)
                )
            };
            let positive = outcome.all_correct || outcome.repair_possible;
            reporter.report(100.0, message.clone());
            Ok(RepairJobResult::Feasibility {
                verify: outcome,
                message,
                positive,
            })
        }));
    }

    fn start_repair(&mut self) {
        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();

        self.result_summary = None;
        self.status_list.files.clear();
        self.feasibility = None;
        self.job = Some(Job::spawn("Verifying before repair...", move |reporter| {
            let (verify_outcome, repair_outcome) =
                repair::verify_then_repair(&par2, &dir, &extras, reporter)?;
            Ok(RepairJobResult::Repair {
                verify: verify_outcome,
                repair: repair_outcome,
            })
        }));
    }
}

/// Color used for positive / negative result labels.
fn color_for(positive: bool) -> egui::Color32 {
    if positive {
        egui::Color32::GREEN
    } else {
        egui::Color32::RED
    }
}
