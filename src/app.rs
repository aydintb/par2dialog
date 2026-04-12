use eframe::egui;

use crate::tabs::batch_tab::BatchTab;
use crate::tabs::create_tab::CreateTab;
use crate::tabs::repair_tab::RepairTab;
use crate::tabs::settings_tab::SettingsTab;
use crate::tabs::verify_tab::VerifyTab;
use crate::theme;

#[derive(Default, PartialEq)]
enum ActiveTab {
    #[default]
    Create,
    Verify,
    Repair,
    Batch,
    Settings,
}

pub struct ParParApp {
    active_tab: ActiveTab,
    dark_mode: bool,
    create_tab: CreateTab,
    verify_tab: VerifyTab,
    repair_tab: RepairTab,
    batch_tab: BatchTab,
    settings_tab: SettingsTab,
}

impl ParParApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::setup(cc);

        let dark_mode = true;
        Self {
            active_tab: ActiveTab::Create,
            dark_mode,
            create_tab: CreateTab::default(),
            verify_tab: VerifyTab::default(),
            repair_tab: RepairTab::default(),
            batch_tab: BatchTab::default(),
            settings_tab: SettingsTab::default(),
        }
    }
}

impl eframe::App for ParParApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🛡️ Par2Dialog");
                ui.separator();
                ui.label(egui::RichText::new("PAR2 GUI").size(12.0).color(ui.visuals().weak_text_color()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("🌙").clicked() {
                        self.dark_mode = !self.dark_mode;
                        theme::set_theme(ctx, self.dark_mode);
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
                ActiveTab::Create => self.create_tab.ui(ui, ctx),
                ActiveTab::Verify => self.verify_tab.ui(ui, ctx),
                ActiveTab::Repair => self.repair_tab.ui(ui, ctx),
                ActiveTab::Batch => self.batch_tab.ui(ui, ctx),
                ActiveTab::Settings => self.settings_tab.ui(ui, ctx),
            }
        });
    }
}
