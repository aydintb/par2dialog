use eframe::egui;

use crate::app::SharedState;
use crate::par2::create::find_par2cmdline;
use crate::theme;
use crate::util::APP_NAME;
use crate::widgets::file_picker::pick_file_dialog;

#[derive(Default)]
pub struct SettingsTab {}

impl SettingsTab {
    #[allow(clippy::unused_self)] // uniform tab API: every tab exposes `ui(&mut self, ...)`
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        shared: &mut SharedState,
        create_tab: &mut crate::tabs::create_tab::CreateTab,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("⚙️ Settings");
            });
            ui.separator();

            Self::defaults_section(ui, shared, create_tab);
            Self::par2_section(ui, shared);
            Self::appearance_section(ui, shared);
            Self::about_section(ui);
            Self::shortcuts_section(ui);
        });
    }

    fn defaults_section(
        ui: &mut egui::Ui,
        shared: &mut SharedState,
        create_tab: &mut crate::tabs::create_tab::CreateTab,
    ) {
        egui::CollapsingHeader::new("🔧 Default Values")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(
                            &mut shared.config.default_redundancy_percent,
                            1..=200,
                        )
                        .text("Default redundancy"),
                    );
                    ui.label(egui::RichText::new("%").strong());
                });
                ui.label(theme::tooltip_text(
                    "Used by the Create tab; click \"Apply to Create tab\" to update it now",
                ));

                ui.separator();

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut shared.config.default_threads)
                            .desired_width(80.0)
                            .hint_text("Auto"),
                    );
                    ui.label("Processing threads")
                        .on_hover_text("Default thread count for creation (empty = auto)");
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut shared.config.default_hash_threads)
                            .desired_width(80.0)
                            .hint_text("2"),
                    );
                    ui.label("Hash threads")
                        .on_hover_text("Default number of files hashed in parallel");
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut shared.config.default_memory_limit)
                            .desired_width(80.0)
                            .hint_text("Auto"),
                    );
                    ui.label("Memory limit (MB)")
                        .on_hover_text("Default memory limit (empty = par2cmdline default)");
                });

                ui.add_space(4.0);
                if ui
                    .button("✅ Apply to Create tab")
                    .on_hover_text("Copy these defaults into the Create tab right now")
                    .clicked()
                {
                    create_tab.apply_defaults(&shared.config);
                    shared.config.save();
                }
            });
    }

    fn par2_section(ui: &mut egui::Ui, shared: &mut SharedState) {
        egui::CollapsingHeader::new("🖥️ PAR2 Command Line")
            .show(ui, |ui| {
                if ui
                    .checkbox(
                        &mut shared.config.auto_detect_par2,
                        "Auto-detect par2cmdline",
                    )
                    .on_hover_text("Find par2 in PATH; turn off to use a custom path")
                    .changed()
                {
                    shared.config.save();
                }

                if !shared.config.auto_detect_par2 {
                    ui.horizontal(|ui| {
                        ui.label("Custom path:");
                        ui.add(
                            egui::TextEdit::singleline(&mut shared.config.par2_path)
                                .desired_width(280.0)
                                .hint_text("/usr/bin/par2"),
                        );
                        if ui.button("📂").clicked() {
                            if let Some(path) = pick_file_dialog("Select par2 binary", None) {
                                shared.config.par2_path = path.to_string_lossy().to_string();
                                shared.config.save();
                            }
                        }
                    });
                }

                // Show the effective detection result.
                let custom = if shared.config.auto_detect_par2 {
                    None
                } else {
                    Some(shared.config.par2_path.as_str())
                };
                if let Some(path) = find_par2cmdline(custom) {
                    ui.horizontal(|ui| {
                        ui.label("Path:");
                        ui.label(
                            egui::RichText::new(path.to_string_lossy().to_string())
                                .family(egui::FontFamily::Monospace)
                                .size(11.0),
                        );
                    });
                    ui.colored_label(egui::Color32::GREEN, "✅ par2cmdline is available");
                } else {
                    ui.colored_label(
                        egui::Color32::RED,
                        "❌ par2cmdline not found — install it to enable Create",
                    );
                    ui.label(theme::tooltip_text(
                        "Install it via: pacman -S par2cmdline / apt install par2 / brew install par2",
                    ));
                }
            });
    }

    fn appearance_section(ui: &mut egui::Ui, shared: &mut SharedState) {
        egui::CollapsingHeader::new("🎨 Appearance")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    let is_dark = shared.dark_mode;
                    if ui.radio(!is_dark, "Light").clicked() && is_dark {
                        shared.set_dark_mode(ui.ctx(), false);
                    }
                    if ui.radio(is_dark, "Dark").clicked() && !is_dark {
                        shared.set_dark_mode(ui.ctx(), true);
                    }
                });
                ui.label(theme::tooltip_text("Shortcut: Ctrl+D toggles the theme"));
            });
    }

    fn about_section(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("ℹ️ About")
            .show(ui, |ui| {
                ui.label(egui::RichText::new(APP_NAME).heading());
                ui.label("A modern cross-platform PAR2 GUI application");
                ui.label("Built with Rust and egui");
                ui.separator();
                ui.label(egui::RichText::new("Features:").strong());
                ui.label("• Create PAR2 recovery files with full parameter control");
                ui.label("• Verify file integrity with detailed status reporting");
                ui.label("• Repair damaged/missing files using Reed-Solomon error correction");
                ui.label("• Batch processing for multiple PAR2 sets");
                ui.label("• Persistent settings, dark and light themes");
                ui.separator();
                ui.label(egui::RichText::new("Backend:").strong());
                ui.label("• rust-par2 for verify/repair (pure Rust, SIMD-accelerated)");
                ui.label("• par2cmdline for creation (spawned as subprocess)");
            });
    }

    fn shortcuts_section(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("⌨️ Keyboard Shortcuts")
            .show(ui, |ui| {
                shortcut_row(ui, "Ctrl+1…5", "Switch between tabs");
                shortcut_row(ui, "Ctrl+D", "Toggle dark / light theme");
            });
    }
}

fn shortcut_row(ui: &mut egui::Ui, keys: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(keys)
                .family(egui::FontFamily::Monospace)
                .strong(),
        );
        ui.label(description);
    });
}
