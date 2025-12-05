use egui::RichText;
use re_sdk_types::components::TextLogLevel;
use re_ui::UiExt as _;

pub fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
    let design_tokens = ui.tokens();

    let error_color = ui.visuals().error_fg_color;
    let warn_color = ui.visuals().warn_fg_color;
    let info_color = design_tokens.info_log_text_color;
    let debug_color = design_tokens.debug_log_text_color;
    let trace_color = design_tokens.trace_log_text_color;
    let text_color = ui.visuals().text_color();

    match lvl {
        TextLogLevel::CRITICAL => RichText::new(lvl)
            .color(design_tokens.strong_fg_color)
            .background_color(error_color),
        TextLogLevel::ERROR => RichText::new(lvl).color(error_color),
        TextLogLevel::WARN => RichText::new(lvl).color(warn_color),
        TextLogLevel::INFO => RichText::new(lvl).color(info_color),
        TextLogLevel::DEBUG => RichText::new(lvl).color(debug_color),
        TextLogLevel::TRACE => RichText::new(lvl).color(trace_color),
        _ => RichText::new(lvl).color(text_color),
    }
}
