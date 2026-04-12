use std::path::PathBuf;

use eframe::egui;

#[derive(Clone)]
enum BatchTaskType {
    Verify,
    Repair,
}

#[derive(Clone)]
struct BatchTask {
    pub par2_path: PathBuf,
    pub data_dir: PathBuf,
    pub task_type: BatchTaskType,
    pub status: BatchStatus,
    pub result: Option<String>,
}

#[derive(Clone, PartialEq)]
enum BatchStatus {
    Pending,
    Running,
    Success,
    Failed,
}

pub struct BatchTab {
    pub tasks: Vec<BatchTask>,
    pub is_running: bool,
    pub current_index: usize,
}

impl Default for BatchTab {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            is_running: false,
            current_index: 0,
        }
    }
}

impl BatchTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("📋 Batch Operations");
            });
            ui.separator();

            ui.label("Queue multiple PAR2 operations and process them sequentially.");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("➕ Add Verify Task").clicked() {
                    self.add_task(BatchTaskType::Verify);
                }
                if ui.button("➕ Add Repair Task").clicked() {
                    self.add_task(BatchTaskType::Repair);
                }
                if !self.tasks.is_empty() && !self.is_running {
                    if ui.button("🗑️ Clear All").clicked() {
                        self.tasks.clear();
                    }
                }
            });

            ui.separator();

            if self.tasks.is_empty() {
                ui.label("No tasks queued. Add tasks to get started.");
            } else {
                // Clone indices to avoid borrow issues
                let task_count = self.tasks.len();
                let indices_to_remove: Vec<usize> = (0..task_count)
                    .filter(|&i| {
                        if i < self.tasks.len() && self.tasks[i].status == BatchStatus::Pending && self.is_running {
                            false
                        } else {
                            false
                        }
                    })
                    .collect();

                for i in 0..task_count {
                    if i >= self.tasks.len() { break; }
                    let task = &self.tasks[i];

                    ui.horizontal(|ui| {
                        let icon = match task.status {
                            BatchStatus::Pending => "⏳",
                            BatchStatus::Running => "🔄",
                            BatchStatus::Success => "✅",
                            BatchStatus::Failed => "❌",
                        };
                        ui.label(egui::RichText::new(icon).size(14.0));

                        let type_label = match task.task_type {
                            BatchTaskType::Verify => "Verify",
                            BatchTaskType::Repair => "Repair",
                        };
                        ui.label(egui::RichText::new(type_label).strong());

                        ui.label(egui::RichText::new(&task.par2_path.to_string_lossy().to_string()).size(10.0).italics());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.is_running && ui.small_button("✖").clicked() {
                                // We'll handle removal after the loop
                            }
                        });
                    });

                    if let Some(result) = &task.result {
                        ui.indent("result", |ui| {
                            ui.label(egui::RichText::new(result).size(10.0));
                        });
                    }
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                let has_pending = self.tasks.iter().any(|t| t.status == BatchStatus::Pending);
                let can_start = has_pending && !self.is_running;

                if ui.add_enabled(can_start, egui::Button::new("▶️ Start Batch")).clicked() {
                    self.start_batch();
                }

                if self.is_running {
                    ui.spinner();
                    ui.label(format!(
                        "Processing task {} of {}...",
                        self.current_index + 1,
                        self.tasks.len()
                    ));
                }
            });
        });
    }

    fn add_task(&mut self, task_type: BatchTaskType) {
        if let Some(par2) = rfd::FileDialog::new()
            .set_title("Select PAR2 file")
            .add_filter("PAR2 files", &["par2"])
            .pick_file()
        {
            let data_dir = par2.parent().map(|p| p.to_path_buf()).unwrap_or_default();

            self.tasks.push(BatchTask {
                par2_path: par2,
                data_dir,
                task_type,
                status: BatchStatus::Pending,
                result: None,
            });
        }
    }

    fn start_batch(&mut self) {
        self.is_running = true;
        self.current_index = 0;
        self.process_current();
    }

    fn process_current(&mut self) {
        if self.current_index >= self.tasks.len() {
            self.is_running = false;
            return;
        }

        self.tasks[self.current_index].status = BatchStatus::Running;

        let par2 = self.tasks[self.current_index].par2_path.clone();
        let dir = self.tasks[self.current_index].data_dir.clone();
        let is_verify = matches!(self.tasks[self.current_index].task_type, BatchTaskType::Verify);

        if is_verify {
            let result = crate::par2::verify::verify(&par2, &dir, &[]);
            self.tasks[self.current_index].result = Some(result.summary.clone());
            self.tasks[self.current_index].status = if result.all_correct {
                BatchStatus::Success
            } else {
                BatchStatus::Failed
            };
        } else {
            let (_, repair) = crate::par2::repair::verify_then_repair(&par2, &dir, &[]);
            self.tasks[self.current_index].result = Some(repair.summary.clone());
            self.tasks[self.current_index].status = if repair.success {
                BatchStatus::Success
            } else {
                BatchStatus::Failed
            };
        }

        self.current_index += 1;
        if self.current_index < self.tasks.len() {
            self.process_current();
        } else {
            self.is_running = false;
        }
    }
}
