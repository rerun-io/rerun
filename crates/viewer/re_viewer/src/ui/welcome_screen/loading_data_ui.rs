use std::sync::Arc;

use egui::Widget as _;
use re_smart_channel::SmartChannelSource;
use re_ui::{DesignTokens, UiExt as _};

/// Show a loading screen for when we are connecting to a data source.
pub fn loading_data_ui(ui: &mut egui::Ui, loading_text: &str) {
    ui.center("loading_data_ui_contents", |ui| {
        ui.vertical_centered(|ui| {
            egui::Spinner::new().size(100.0).ui(ui);

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

pub fn loading_text_for_data_sources(log_sources: &[Arc<SmartChannelSource>]) -> Option<String> {
    // If there's several data sources that should show a loading text, pick the first one.
    for source in log_sources {
        match source.as_ref() {
            SmartChannelSource::File(path) => {
                if let Some(path_str) = path.to_str() {
                    return Some(format!("Loading {path_str} …",));
                }
            }

            SmartChannelSource::RrdHttpStream { url, .. } => {
                return Some(format!("Connecting to {url} …"));
            }

            SmartChannelSource::RrdWebEventListener | SmartChannelSource::JsChannel { .. } => {
                return Some("Loading…".to_owned());
            }

            SmartChannelSource::Sdk
            | SmartChannelSource::Stdin
            | SmartChannelSource::MessageProxy(..) => {
                // These sources may or may not send data, so stick with regular welcome screen until we know better.
            }

            SmartChannelSource::RedapGrpcStream { uri, .. } => {
                if uri.origin != *re_redap_browser::EXAMPLES_ORIGIN {
                    return Some(format!("Connecting to {} …", uri.origin));
                }
            }
        }
    }

    None
}
