use re_ui::{DesignTokens, UiExt as _};

use super::entrance_animation::{EntranceAnimation, EntranceAnimationItem};

pub(super) const DOCS_URL: &str = "https://www.rerun.io/docs";
pub(super) const WELCOME_SCREEN_TITLE: &str = "The data layer for physical AI";
pub(super) const WELCOME_SCREEN_BULLET_TEXT: &[&str] = &[
    "Log multi-rate, multimodal data with the Rerun SDK in C++, Python, or Rust",
    "Visualize and explore live or recorded data across the pipeline",
    "Query with dataframes or SQL, and stream directly to training",
];

const TITLE_ANIMATION: EntranceAnimationItem =
    EntranceAnimationItem::new(0.38, 0.07).translated_y(30.0);

/// Show the welcome section.
pub fn welcome_section_ui(ui: &mut egui::Ui, animation: &mut EntranceAnimation) {
    re_tracing::profile_function!();

    ui.vertical(|ui| {
        let style = if ui.available_width() > 400.0 {
            DesignTokens::heading_1()
        } else {
            DesignTokens::heading_2()
        };

        animation.show(
            ui,
            ui.scope_id().with("welcome_title"),
            TITLE_ANIMATION,
            |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(WELCOME_SCREEN_TITLE)
                            .strong()
                            .text_style(style)
                            .color(ui.tokens().text_strong),
                    )
                    .wrap(),
                );
            },
        );
    });
}
