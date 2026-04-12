use eframe::egui;

use crate::par2::types::ProgressUpdate;

pub struct ProgressBar {
    pub progress: f32,
    pub max: f32,
    pub label: String,
    pub show_percentage: bool,
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self {
            progress: 0.0,
            max: 100.0,
            label: String::new(),
            show_percentage: true,
        }
    }
}

impl ProgressBar {
    pub fn show(&self, ui: &mut egui::Ui) {
        let fraction = if self.max > 0.0 {
            (self.progress / self.max).clamp(0.0, 1.0)
        } else {
            0.0
        };

        ui.vertical(|ui| {
            if !self.label.is_empty() {
                ui.label(&self.label);
            }

            ui.horizontal(|ui| {
                let pw = ui.available_width() - if self.show_percentage { 50.0 } else { 0.0 };

                let visuals = ui.visuals();
                let rect = ui.painter().clip_rect();
                let bar_rect = egui::Rect::from_min_size(rect.min, egui::vec2(pw, 16.0));

                ui.painter().rect_filled(
                    bar_rect,
                    egui::CornerRadius::same(4),
                    visuals.widgets.inactive.bg_fill,
                );

                let fill_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    egui::vec2(bar_rect.width() * fraction, bar_rect.height()),
                );

                ui.painter().rect_filled(
                    fill_rect,
                    egui::CornerRadius::same(4),
                    visuals.selection.bg_fill,
                );

                // Consume space for the progress bar
                ui.allocate_space(egui::vec2(pw, 16.0));

                if self.show_percentage {
                    ui.label(format!("{:.0}%", fraction * 100.0));
                }
            });
        });
    }

    pub fn update(&mut self, update: &ProgressUpdate) {
        self.progress = update.current;
        self.max = update.total;
        self.label.clone_from(&update.message);
    }
}

pub fn indeterminate_progress(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(label);
    });
}
