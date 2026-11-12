use std::path::PathBuf;

use eframe::egui;

use crate::app::SharedState;
use crate::jobs::Job;
use crate::par2::types::{FileStatus, VerifyOutcome};
use crate::par2::verify;
use crate::widgets::file_picker::{
    pick_directory_dialog, pick_file_dialog, pick_files_dialog,
};
use crate::widgets::par2_files::show_related_par2_files;
use crate::widgets::progress;
use crate::widgets::status_list::StatusList;

/// Request to jump to the Repair tab with pre-filled paths.
pub struct RepairRequest {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub extra_files: Vec<PathBuf>,
}

pub struct VerifyTab {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub extra_files: Vec<PathBuf>,
    job: Option<Job<VerifyOutcome>>,
    status_list: StatusList,
    result_summary: Option<String>,
    /// PAR2 path the current (or last) result belongs to, to clear stale results.
    last_result_par2: PathBuf,
}

impl Default for VerifyTab {
    fn default() -> Self {
        Self {
            par2_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            extra_files: Vec::new(),
            job: None,
            status_list: StatusList::default(),
            result_summary: None,
            last_result_par2: PathBuf::new(),
        }
    }
}

impl VerifyTab {
    /// Draw the tab. Returns a request to switch to Repair when the user
    /// clicked "Switch to Repair" with damage present.
    pub fn ui(&mut self, ui: &mut egui::Ui, _shared: &mut SharedState) -> Option<RepairRequest> {
        // Poll the background job.
        let mut finished = None;
        if let Some(job) = &self.job {
            if !job.is_running() {
                finished = job.take_result();
            }
        }
        if let Some(outcome) = finished {
            self.job = None;
            self.last_result_par2.clone_from(&self.par2_path);
            self.status_list.files.clone_from(&outcome.files);
            self.result_summary = Some(outcome.summary);
        }

        // Stale results (paths changed since the verify ran) are misleading.
        if !self.last_result_par2.is_empty() && self.last_result_par2 != self.par2_path {
            self.status_list.files.clear();
            self.result_summary = None;
            self.last_result_par2.clear();
        }

        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("✅ Verify File Integrity");
                });
                ui.separator();

                egui::CollapsingHeader::new("📁 Files")
                    .default_open(true)
                    .show(ui, |ui| self.files_section(ui));

                ui.separator();

                self.action_section(ui)
            })
            .inner
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
                    .extend(pick_files_dialog("Select extra files to check", None));
            }
            if !self.extra_files.is_empty() && ui.button("🗑️ Clear").clicked() {
                self.extra_files.clear();
            }
        });
        if !self.extra_files.is_empty() {
            ui.label(format!("{} extra files", self.extra_files.len()));
        }
    }

    /// Verify button, progress and results. Returns a `RepairRequest` when the
    /// user chose to switch to the Repair tab.
    fn action_section(&mut self, ui: &mut egui::Ui) -> Option<RepairRequest> {
        let mut repair_request = None;

        ui.horizontal(|ui| {
            let can_run = !self.par2_path.as_os_str().is_empty()
                && !self.data_dir.as_os_str().is_empty()
                && !self.is_running();
            let btn = ui.add_enabled(
                can_run,
                egui::Button::new(egui::RichText::new("🔍 Verify").size(16.0)),
            );
            if btn.clicked() {
                self.start_verify();
            }
            if self.is_running() {
                ui.spinner();
                ui.label("Verifying...");
            }
        });

        if let Some(job) = &self.job {
            ui.add_space(6.0);
            progress::bar(ui, &job.progress());
        }

        if self.status_list.files.is_empty() {
            if let Some(summary) = &self.result_summary {
                ui.add_space(6.0);
                ui.label(summary);
            }
            return repair_request;
        }

        ui.add_space(6.0);
        self.status_list.show(ui);
        if let Some(summary) = &self.result_summary {
            ui.separator();
            ui.label(egui::RichText::new(summary).size(12.0));
        }

        // Offer a one-click switch to Repair when damage is detected.
        let has_damage = self
            .status_list
            .files
            .iter()
            .any(|f| matches!(f.status, FileStatus::Damaged | FileStatus::Missing));
        if has_damage
            && ui
                .button(egui::RichText::new("🔧 Switch to Repair").size(14.0))
                .clicked()
        {
            repair_request = Some(RepairRequest {
                par2_path: self.par2_path.clone(),
                data_dir: self.data_dir.clone(),
                extra_files: self.extra_files.clone(),
            });
        }

        if ui.small_button("🔄 Clear Results").clicked() {
            self.status_list.files.clear();
            self.result_summary = None;
            self.last_result_par2.clear();
        }

        repair_request
    }

    fn is_running(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }

    fn start_verify(&mut self) {
        let par2 = self.par2_path.clone();
        let dir = self.data_dir.clone();
        let extras = self.extra_files.clone();

        self.result_summary = None;
        self.status_list.files.clear();
        self.last_result_par2.clear();
        self.job = Some(Job::spawn("Parsing PAR2 file...", move |reporter| {
            reporter.report(20.0, "Verifying files...");
            verify::verify(&par2, &dir, &extras)
        }));
    }
}
