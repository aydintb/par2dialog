use eframe::egui;

/// Apply the dark/light theme plus app-wide spacing.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    ctx.set_style(style);
}

/// Small italic helper text.
#[must_use]
pub fn tooltip_text(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(11.0).italics()
}
