use egui::{Color32, RichText};
use re_types::components::TextLogLevel;
use re_ui::UiExt as _;

pub fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
    let error_color = ui.visuals().error_fg_color;
    let warn_color = ui.visuals().warn_fg_color;
    let text_color = ui.visuals().text_color();
    let (info_color, debug_color, trace_color);
    match ui.theme() {
        egui::Theme::Dark => {
            info_color = Color32::LIGHT_GREEN;
            debug_color = Color32::LIGHT_BLUE;
            trace_color = Color32::LIGHT_GRAY;
        }
        egui::Theme::Light => {
            info_color = Color32::DARK_GREEN;
            debug_color = Color32::DARK_BLUE;
            trace_color = Color32::DARK_GRAY;
        }
    }

    match lvl {
        TextLogLevel::CRITICAL => RichText::new(lvl)
            .color(Color32::WHITE)
            .background_color(error_color),
        TextLogLevel::ERROR => RichText::new(lvl).color(error_color),
        TextLogLevel::WARN => RichText::new(lvl).color(warn_color),
        TextLogLevel::INFO => RichText::new(lvl).color(info_color),
        TextLogLevel::DEBUG => RichText::new(lvl).color(debug_color),
        TextLogLevel::TRACE => RichText::new(lvl).color(trace_color),
        _ => RichText::new(lvl).color(text_color),
    }
}
