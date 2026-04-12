use eframe::egui;

use crate::par2::types::{FileEntry, FileStatus};
use crate::theme;

/// A scrollable list showing file statuses with color coding
pub struct StatusList {
    pub files: Vec<FileEntry>,
    pub show_damage_detail: bool,
}

impl Default for StatusList {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            show_damage_detail: true,
        }
    }
}

impl StatusList {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.files.is_empty() {
            ui.label("No files to display.");
            return;
        }

        let ok_count = self.files.iter().filter(|f| matches!(f.status, FileStatus::Ok)).count();
        let damaged_count = self.files.iter().filter(|f| matches!(f.status, FileStatus::Damaged)).count();
        let missing_count = self.files.iter().filter(|f| matches!(f.status, FileStatus::Missing)).count();

        // Summary bar
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} files total", self.files.len())).strong());
            ui.separator();
            ui.label(egui::RichText::new(format!("✅ {} OK", ok_count)).color(egui::Color32::GREEN));
            if damaged_count > 0 {
                ui.label(egui::RichText::new(format!("❌ {} damaged", damaged_count)).color(egui::Color32::RED));
            }
            if missing_count > 0 {
                ui.label(egui::RichText::new(format!("❓ {} missing", missing_count)).color(egui::Color32::YELLOW));
            }
        });

        ui.separator();

        // File list
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for file in &self.files {
                ui.horizontal(|ui| {
                    let (icon, color) = match file.status {
                        FileStatus::Ok => ("✅", egui::Color32::GREEN),
                        FileStatus::Damaged => ("❌", egui::Color32::RED),
                        FileStatus::Missing => ("❓", egui::Color32::YELLOW),
                        FileStatus::Unknown => ("❔", egui::Color32::GRAY),
                    };

                    ui.label(egui::RichText::new(icon).size(14.0));
                    ui.label(theme::tooltip_text(&file.path.to_string_lossy()));

                    if self.show_damage_detail {
                        if let Some(detail) = &file.damage_detail {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(detail).size(10.0).color(color));
                            });
                        }
                    }
                });
            }
        });
    }
}
