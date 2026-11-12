use std::path::PathBuf;

use eframe::egui;

use crate::app_config::AppConfig;
use crate::tabs::batch_tab::BatchTab;
use crate::tabs::create_tab::CreateTab;
use crate::tabs::repair_tab::RepairTab;
use crate::tabs::settings_tab::SettingsTab;
use crate::tabs::verify_tab::VerifyTab;
use crate::theme;

/// State shared by every tab: theme plus persistent configuration.
pub struct SharedState {
    pub dark_mode: bool,
    pub config: AppConfig,
}

impl SharedState {
    /// Toggle the theme and persist the choice.
    pub fn set_dark_mode(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark_mode = dark;
        self.config.dark_mode = dark;
        self.config.save();
        theme::apply(ctx, dark);
    }

    /// Custom par2 binary to pass to the create backend, if configured.
    pub fn par2_binary_override(&self) -> Option<String> {
        if self.config.auto_detect_par2 || self.config.par2_path.trim().is_empty() {
            None
        } else {
            Some(self.config.par2_path.clone())
        }
    }
}

#[derive(Default, PartialEq, Copy, Clone)]
enum ActiveTab {
    #[default]
    Create,
    Verify,
    Repair,
    Batch,
    Settings,
}

impl ActiveTab {
    fn from_key(key: egui::Key) -> Option<Self> {
        match key {
            egui::Key::Num1 => Some(Self::Create),
            egui::Key::Num2 => Some(Self::Verify),
            egui::Key::Num3 => Some(Self::Repair),
            egui::Key::Num4 => Some(Self::Batch),
            egui::Key::Num5 => Some(Self::Settings),
            _ => None,
        }
    }
}

/// Signal from the verify tab to switch to Repair with pre-filled paths.
struct RepairRequest {
    par2_path: PathBuf,
    data_dir: PathBuf,
    extra_files: Vec<PathBuf>,
}

pub struct Par2DialogApp {
    active_tab: ActiveTab,
    shared: SharedState,
    create_tab: CreateTab,
    verify_tab: VerifyTab,
    repair_tab: RepairTab,
    batch_tab: BatchTab,
    settings_tab: SettingsTab,
    repair_request: Option<RepairRequest>,
}

impl Par2DialogApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        theme::apply(&cc.egui_ctx, config.dark_mode);

        let mut create_tab = CreateTab::default();
        create_tab.apply_defaults(&config);

        Self {
            active_tab: ActiveTab::Create,
            shared: SharedState {
                dark_mode: config.dark_mode,
                config,
            },
            create_tab,
            verify_tab: VerifyTab::default(),
            repair_tab: RepairTab::default(),
            batch_tab: BatchTab::default(),
            settings_tab: SettingsTab::default(),
            repair_request: None,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            if !input.modifiers.ctrl {
                return;
            }
            for key in [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
            ] {
                if input.key_pressed(key) {
                    if let Some(tab) = ActiveTab::from_key(key) {
                        self.active_tab = tab;
                    }
                }
            }
            if input.key_pressed(egui::Key::D) {
                let dark = !self.shared.dark_mode;
                self.shared.set_dark_mode(ctx, dark);
            }
        });
    }
}

impl eframe::App for Par2DialogApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🛡️ Par2Dialog");
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new("PAR2 GUI")
                        .size(12.0)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let theme_icon = if self.shared.dark_mode { "☀️" } else { "🌙" };
                    let theme_tip = if self.shared.dark_mode {
                        "Switch to light theme (Ctrl+D)"
                    } else {
                        "Switch to dark theme (Ctrl+D)"
                    };
                    if ui
                        .small_button(theme_icon)
                        .on_hover_text(theme_tip)
                        .clicked()
                    {
                        let dark = !self.shared.dark_mode;
                        self.shared.set_dark_mode(ctx, dark);
                    }
                });
            });

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Create, "📦 Create");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Verify, "✅ Verify");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Repair, "🔧 Repair");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Batch, "📋 Batch");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Settings, "⚙️ Settings");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                ActiveTab::Create => self.create_tab.ui(ui, &mut self.shared),
                ActiveTab::Verify => {
                    if let Some(request) = self.verify_tab.ui(ui, &mut self.shared) {
                        self.repair_request = Some(RepairRequest {
                            par2_path: request.par2_path,
                            data_dir: request.data_dir,
                            extra_files: request.extra_files,
                        });
                        self.active_tab = ActiveTab::Repair;
                    }
                }
                ActiveTab::Repair => {
                    // Coming from verify: pre-fill with the verified set.
                    if let Some(request) = self.repair_request.take() {
                        self.repair_tab.par2_path = request.par2_path;
                        self.repair_tab.data_dir = request.data_dir;
                        self.repair_tab.extra_files = request.extra_files;
                    }
                    self.repair_tab.ui(ui, &mut self.shared);
                }
                ActiveTab::Batch => self.batch_tab.ui(ui, &mut self.shared),
                ActiveTab::Settings => {
                    self.settings_tab
                        .ui(ui, &mut self.shared, &mut self.create_tab);
                }
            }
        });
    }
}