use eframe::egui;

use crate::par2::create;
use crate::theme;

pub struct SettingsTab {
    pub default_redundancy: f64,
    pub default_threads: String,
    pub default_hash_threads: String,
    pub default_memory_limit: String,
    pub auto_detect_par2cmdline: bool,
    pub par2cmdline_path: String,
    pub log_filter: String,
}

impl Default for SettingsTab {
    fn default() -> Self {
        let cmdline = create::find_par2cmdline();
        Self {
            default_redundancy: 5.0,
            default_threads: String::new(),
            default_hash_threads: "2".to_string(),
            default_memory_limit: String::new(),
            auto_detect_par2cmdline: true,
            par2cmdline_path: cmdline.unwrap_or_else(|| "not found".to_string()),
            log_filter: "info".to_string(),
        }
    }
}

impl SettingsTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("⚙️ Settings");
            });
            ui.separator();

            // ── Defaults ──
            egui::CollapsingHeader::new("🔧 Default Values")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.default_redundancy, 1.0..=200.0).text("Default redundancy"));
                        ui.label(format!("{}%", self.default_redundancy as u64));
                    });
                    ui.label(egui::RichText::new("ℹ️ Applied to new Create operations").size(11.0).italics().color(ui.visuals().weak_text_color()));

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.default_threads).desired_width(80.0).hint_text("Auto"));
                        ui.label("Processing threads").on_hover_text("Default thread count for creation");
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.default_hash_threads).desired_width(80.0));
                        ui.label("Hash threads").on_hover_text("Default parallel hashing threads");
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.default_memory_limit).desired_width(80.0).hint_text("Auto"));
                        ui.label("Memory limit (MB)").on_hover_text("Default memory limit");
                    });
                });

            // ── PAR2 Command Line ──
            egui::CollapsingHeader::new("🖥️ PAR2 Command Line")
                .show(ui, |ui| {
                    ui.checkbox(&mut self.auto_detect_par2cmdline, "Auto-detect par2cmdline");
                    ui.horizontal(|ui| {
                        ui.label("Path:");
                        ui.label(egui::RichText::new(&self.par2cmdline_path).family(egui::FontFamily::Monospace).size(11.0));
                    });

                    let available = self.par2cmdline_path != "not found";
                    if available {
                        ui.colored_label(egui::Color32::GREEN, "✅ par2cmdline is available");
                    } else {
                        ui.colored_label(egui::Color32::RED, "❌ par2cmdline not found in PATH");
                        ui.label(
                            egui::RichText::new("Install it via: pacman -S par2cmdline / apt install par2 / brew install par2")
                                .size(11.0)
                                .italics(),
                        );
                    }
                });

            // ── Appearance ──
            egui::CollapsingHeader::new("🎨 Appearance")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        let is_dark = ui.style().visuals.dark_mode;
                        if ui.radio(!is_dark, "Light").clicked() {
                            theme::set_theme(ctx, false);
                        }
                        if ui.radio(is_dark, "Dark").clicked() {
                            theme::set_theme(ctx, true);
                        }
                    });
                });

            // ── About ──
            egui::CollapsingHeader::new("ℹ️ About")
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Par2Dialog").heading());
                    ui.label("A modern cross-platform PAR2 GUI application");
                    ui.label("Built with Rust and egui");
                    ui.separator();
                    ui.label(egui::RichText::new("Features:").strong());
                    ui.label("• Create PAR2 recovery files with full parameter control");
                    ui.label("• Verify file integrity with detailed status reporting");
                    ui.label("• Repair damaged/missing files using Reed-Solomon error correction");
                    ui.label("• Batch processing for multiple PAR2 sets");
                    ui.label("• Dark and light themes");
                    ui.separator();
                    ui.label(egui::RichText::new("Backend:").strong());
                    ui.label("• rust-par2 for verify/repair (pure Rust, SIMD-accelerated)");
                    ui.label("• par2cmdline for creation (spawned as subprocess)");
                });

            // ── Keyboard Shortcuts ──
            egui::CollapsingHeader::new("⌨️ Keyboard Shortcuts")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Tab navigation:").family(egui::FontFamily::Monospace));
                        ui.label("Use the tab bar at the top to switch between operations");
                    });
                });
        });
    }
}
