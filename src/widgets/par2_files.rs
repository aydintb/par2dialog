//! Shared UI snippets used by more than one tab.

use std::path::Path;

use eframe::egui;

/// Show the sibling `.par2` files that belong to the same PAR2 set.
pub fn show_related_par2_files(ui: &mut egui::Ui, par2_path: &Path) {
    let Some(parent) = par2_path.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };

    let related: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("par2") && p != par2_path
        })
        .collect();

    if related.is_empty() {
        return;
    }

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
            ui.label(
                egui::RichText::new(p.to_string_lossy())
                    .size(10.0)
                    .family(egui::FontFamily::Monospace),
            );
        }
    });
}
