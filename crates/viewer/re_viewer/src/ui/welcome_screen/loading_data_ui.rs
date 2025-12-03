use std::sync::Arc;

use re_log_channel::LogSource;
use re_ui::{DesignTokens, UiExt as _};

/// Show a loading screen for when we are connecting to a data source.
pub fn loading_data_ui(ui: &mut egui::Ui, loading_text: &str) {
    ui.center("loading_data_ui_contents", |ui| {
        ui.vertical_centered(|ui| {
            ui.add(egui::Spinner::new().size(100.0));

            ui.add_space(50.0);

            ui.add(
                egui::Label::new(
                    egui::RichText::new(loading_text)
                        .text_style(DesignTokens::welcome_screen_body()),
                )
                .wrap(),
            );
        });
    });
}

pub fn loading_text_for_data_sources(log_sources: &[Arc<LogSource>]) -> Option<String> {
    // If there's several data sources that should show a loading text, pick the first one.
    for source in log_sources {
        if let Some(loading_name) = source.loading_name() {
            return Some(format!("Loading {loading_name}"));
        }
    }

    None
}
