use eframe::egui;

pub fn setup(cc: &eframe::CreationContext<'_>) {
    set_theme(&cc.egui_ctx, true);
}

pub fn set_theme(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();

    if dark {
        style.visuals = egui::Visuals::dark();
    } else {
        style.visuals = egui::Visuals::light();
    }

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.interact_size.y = 28.0;

    ctx.set_style(style);
}

pub fn tooltip_text(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(11.0).italics()
}

pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
