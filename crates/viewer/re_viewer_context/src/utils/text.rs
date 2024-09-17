use egui::{Color32, RichText};
use re_types::components::TextLogLevel;

pub fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
    match lvl {
        TextLogLevel::CRITICAL => RichText::new(lvl)
            .color(Color32::WHITE)
            .background_color(ui.visuals().error_fg_color),
        TextLogLevel::ERROR => RichText::new(lvl).color(ui.visuals().error_fg_color),
        TextLogLevel::WARN => RichText::new(lvl).color(ui.visuals().warn_fg_color),
        TextLogLevel::INFO => RichText::new(lvl).color(Color32::LIGHT_GREEN),
        TextLogLevel::DEBUG => RichText::new(lvl).color(Color32::LIGHT_BLUE),
        TextLogLevel::TRACE => RichText::new(lvl).color(Color32::LIGHT_GRAY),
        _ => RichText::new(lvl).color(ui.visuals().text_color()),
    }
}
