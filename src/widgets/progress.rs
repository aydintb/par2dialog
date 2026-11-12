//! Progress bar widget.

use eframe::egui;

use crate::jobs::Progress;

pub fn bar(ui: &mut egui::Ui, progress: &Progress) {
    ui.vertical(|ui| {
        if !progress.message.is_empty() {
            ui.label(&progress.message);
        }

        ui.horizontal(|ui| {
            let bar_width = (ui.available_width() - 50.0).max(40.0);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(bar_width, 14.0),
                egui::Sense::hover(),
            );

            let visuals = ui.visuals();
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(4),
                visuals.widgets.inactive.bg_fill,
            );

            let fraction = progress.pct / 100.0;
            let fill_width = rect.width() * fraction.clamp(0.0, 1.0);
            if fill_width > 0.5 {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, rect.height())),
                    egui::CornerRadius::same(4),
                    visuals.selection.bg_fill,
                );
            }

            ui.label(format!("{:.0}%", fraction * 100.0));
        });
    });
}
