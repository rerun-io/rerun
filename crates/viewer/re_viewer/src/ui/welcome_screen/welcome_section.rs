use egui::{Color32, Ui};

use re_ui::UiExt as _;

pub(super) const DOCS_URL: &str = "https://www.rerun.io/docs";
pub(super) const WELCOME_SCREEN_TITLE: &str = "Visualize multimodal data";
pub(super) const WELCOME_SCREEN_BULLET_TEXT: &[&str] = &[
    "Log data with the Rerun SDK in C++, Python, or Rust",
    "Visualize and explore live or recorded data",
    "Configure the viewer interactively or through code",
];

// XXX TODO MOVE MACRO TO UTILITY MODULE OR UTILITY CRATE
macro_rules! color_from_rgb_hex {
    ($hex:literal) => {
        match color_hex::color_from_hex!($hex).as_slice() {
            [a, b, c] => Color32::from_rgb(*a, *b, *c),
            _ => panic!("XXX"),
        }
    };
}

const DOC_BTN_TEXT: &str = "Go to documentation →";
const DOC_BTN_COLOR: Color32 = color_from_rgb_hex!("#60A0FF");

/// Show the welcome section.
pub(super) fn welcome_section_ui(ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        let (style, line_height) = if ui.available_width() > 400.0 {
            (re_ui::DesignTokens::welcome_screen_h1(), 50.0)
        } else {
            (re_ui::DesignTokens::welcome_screen_h2(), 36.0)
        };

        ui.add(
            egui::Label::new(
                egui::RichText::new(WELCOME_SCREEN_TITLE)
                    .strong()
                    .line_height(Some(line_height))
                    .text_style(style),
            )
            .wrap(),
        );

        ui.add_space(18.0);

        let bullet_text = |ui: &mut Ui, text: &str| {
            ui.horizontal(|ui| {
                ui.add_space(1.0);
                ui.bullet(ui.visuals().strong_text_color());
                ui.add_space(5.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .color(ui.visuals().widgets.active.text_color())
                            .text_style(re_ui::DesignTokens::welcome_screen_body()),
                    )
                    .wrap(),
                );
            });
            ui.add_space(4.0);
        };

        for text in WELCOME_SCREEN_BULLET_TEXT {
            bullet_text(ui, text);
        }

        ui.add_space(9.0);
        if ui
            .button(
                egui::RichText::new(DOC_BTN_TEXT)
                    .color(DOC_BTN_COLOR)
                    .text_style(re_ui::DesignTokens::welcome_screen_body()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            ui.ctx().open_url(egui::output::OpenUrl {
                url: DOCS_URL.to_owned(),
                new_tab: true,
            });
        }
    });
}
