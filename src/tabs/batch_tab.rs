use std::path::PathBuf;

use eframe::egui;

use crate::app::SharedState;
use crate::jobs::Job;
use crate::par2::repair;
use crate::par2::verify;
use crate::widgets::file_picker::{pick_directory_dialog, pick_file_dialog};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchTaskType {
    Verify,
    Repair,
}

impl BatchTaskType {
    fn label(self) -> &'static str {
        match self {
            Self::Verify => "Verify",
            Self::Repair => "Repair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    Pending,
    Running,
    Success,
    Failed,
}

pub struct BatchTask {
    par2_path: PathBuf,
    data_dir: PathBuf,
    task_type: BatchTaskType,
    status: BatchStatus,
    result: Option<String>,
}

/// Per-task outcome returned by the batch job.
struct TaskOutcome {
    success: bool,
    summary: String,
}

#[derive(Default)]
pub struct BatchTab {
    tasks: Vec<BatchTask>,
    job: Option<Job<Vec<TaskOutcome>>>,
    /// Summary line shown after a batch run: `(message, all_succeeded)`.
    last_summary: Option<(String, bool)>,
}

impl BatchTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _shared: &mut SharedState) {
        // Poll the background job.
        let mut finished = None;
        if let Some(job) = &self.job {
            if !job.is_running() {
                finished = job.take_result();
            }
        }
        if let Some(outcomes) = finished {
            self.job = None;
            self.apply_batch_results(outcomes);
        }

        let running = self.is_running();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("📋 Batch Operations");
            });
            ui.separator();

            ui.label("Queue multiple PAR2 operations; they run sequentially in the background.");
            ui.separator();

            self.toolbar_section(ui, running);
            ui.separator();
            self.task_list_section(ui, running);
            ui.separator();
            self.controls_section(ui, running);
        });
    }

    fn apply_batch_results(&mut self, outcomes: Vec<TaskOutcome>) {
        let succeeded = outcomes.iter().filter(|o| o.success).count();
        for (task, outcome) in self.tasks.iter_mut().zip(outcomes) {
            task.status = if outcome.success {
                BatchStatus::Success
            } else {
                BatchStatus::Failed
            };
            task.result = Some(outcome.summary);
        }
        let failed = self.tasks.len() - succeeded;
        self.last_summary = Some((
            format!("Batch finished: {succeeded} succeeded, {failed} failed."),
            failed == 0,
        ));
    }

    fn toolbar_section(&mut self, ui: &mut egui::Ui, running: bool) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!running, egui::Button::new("➕ Add Verify Task"))
                .clicked()
            {
                self.add_task(BatchTaskType::Verify);
            }
            if ui
                .add_enabled(!running, egui::Button::new("➕ Add Repair Task"))
                .clicked()
            {
                self.add_task(BatchTaskType::Repair);
            }
            if ui
                .add_enabled(!running, egui::Button::new("📁 Add PAR2 Sets from Folder"))
                .on_hover_text("Queue a Verify task for every PAR2 index file in a directory")
                .clicked()
            {
                self.add_tasks_from_folder();
            }
            if !self.tasks.is_empty()
                && ui
                    .add_enabled(!running, egui::Button::new("🗑️ Clear All"))
                    .clicked()
            {
                self.tasks.clear();
                self.last_summary = None;
            }
        });
    }

    fn task_list_section(&mut self, ui: &mut egui::Ui, running: bool) {
        if self.tasks.is_empty() {
            ui.label("No tasks queued. Add tasks to get started.");
            return;
        }

        let mut to_remove: Option<usize> = None;
        for (i, task) in self.tasks.iter().enumerate() {
            let icon = match task.status {
                BatchStatus::Pending => "⏳",
                BatchStatus::Running => "🔄",
                BatchStatus::Success => "✅",
                BatchStatus::Failed => "❌",
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).size(14.0));
                ui.label(egui::RichText::new(task.task_type.label()).strong());
                ui.label(
                    egui::RichText::new(task.par2_path.to_string_lossy().to_string())
                        .size(10.0)
                        .italics(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !running
                        && ui
                            .small_button("✖")
                            .on_hover_text("Remove this task")
                            .clicked()
                    {
                        to_remove = Some(i);
                    }
                });
            });
            if let Some(result) = &task.result {
                ui.indent("batch_result", |ui| {
                    ui.label(egui::RichText::new(result).size(10.0));
                });
            }
        }
        if let Some(i) = to_remove {
            self.tasks.remove(i);
            self.last_summary = None;
        }
    }

    fn controls_section(&mut self, ui: &mut egui::Ui, running: bool) {
        ui.horizontal(|ui| {
            let has_pending = self.tasks.iter().any(|t| t.status == BatchStatus::Pending);
            let can_start = has_pending && !running;

            if ui
                .add_enabled(can_start, egui::Button::new("▶️ Start Batch"))
                .clicked()
            {
                self.start_batch();
            }

            if running {
                ui.spinner();
                if let Some(job) = &self.job {
                    ui.label(job.progress().message);
                }
            }
        });

        if let Some((summary, all_ok)) = &self.last_summary {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(summary).color(if *all_ok {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            }));
        }
    }

    fn is_running(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }

    fn add_task(&mut self, task_type: BatchTaskType) {
        if let Some(par2) =
            pick_file_dialog("Select PAR2 file", Some("par2"))
        {
            let data_dir = par2
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            self.tasks.push(BatchTask {
                par2_path: par2,
                data_dir,
                task_type,
                status: BatchStatus::Pending,
                result: None,
            });
            self.last_summary = None;
        }
    }

    /// Queue a Verify task for every PAR2 index file (not .vol*.par2) in a folder.
    fn add_tasks_from_folder(&mut self) {
        if let Some(dir) = pick_directory_dialog("Select folder with PAR2 sets") {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return;
            };
            let mut added = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                let is_index = path.extension().and_then(|e| e.to_str()) == Some("par2")
                    && path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| !s.contains(".vol"));
                if !is_index {
                    continue;
                }
                self.tasks.push(BatchTask {
                    data_dir: dir.clone(),
                    par2_path: path,
                    task_type: BatchTaskType::Verify,
                    status: BatchStatus::Pending,
                    result: None,
                });
                added += 1;
            }
            if added > 0 {
                self.last_summary = None;
            } else {
                self.last_summary = Some((
                    "No PAR2 index files found in that folder.".to_string(),
                    false,
                ));
            }
        }
    }

    fn start_batch(&mut self) {
        // Snapshot the queue; the job works on its own copy so the user can
        // keep editing the list for the next run.
        let tasks: Vec<(PathBuf, PathBuf, BatchTaskType)> = self
            .tasks
            .iter()
            .map(|t| (t.par2_path.clone(), t.data_dir.clone(), t.task_type))
            .collect();
        let total = tasks.len();

        for task in &mut self.tasks {
            if task.status == BatchStatus::Pending {
                task.status = BatchStatus::Running;
            }
            task.result = None;
        }
        self.last_summary = None;

        self.job = Some(Job::spawn("Starting batch...", move |reporter| {
            let mut outcomes = Vec::with_capacity(total);
            for (i, (par2, dir, task_type)) in tasks.into_iter().enumerate() {
                #[allow(clippy::cast_precision_loss)] // progress percent; precision irrelevant
                let done =
                    (i.min(u32::MAX as usize) as f32)
                        / (total.max(1).min(u32::MAX as usize) as f32);
                reporter.report(
                    done * 100.0,
                    format!("Task {}/{}: {}ing {}", i + 1, total, task_type.label(),
                        par2.display()),
                );
                let outcome = match task_type {
                    BatchTaskType::Verify => {
                        let result = verify::verify(&par2, &dir, &[]);
                        TaskOutcome {
                            success: result.all_correct,
                            summary: result.summary,
                        }
                    }
                    BatchTaskType::Repair => {
                        let (_, repair_outcome) = repair::verify_then_repair(&par2, &dir, &[]);
                        TaskOutcome {
                            success: repair_outcome.success,
                            summary: repair_outcome.summary,
                        }
                    }
                };
                outcomes.push(outcome);
            }
            reporter.report(100.0, "Batch finished");
            outcomes
        }));
    }
}
