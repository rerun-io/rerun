#![allow(clippy::unwrap_used)]
#![allow(clippy::enum_glob_use)] // Nice to have for the color variants

use egui::{Color32, Theme, hex_color};

use crate::{
    DesignTokens,
    color_table::{ColorToken, Scale::*},
};

impl DesignTokens {
    /// Get the [`Color32`] corresponding to the provided [`ColorToken`].
    // TODO: make private
    #[inline]
    pub fn color(&self, token: ColorToken) -> Color32 {
        self.color_table.get(token)
    }

    /// The color we use to mean "loop this selection"
    pub fn loop_selection_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => hex_color!("#012569B2"),
            Theme::Light => hex_color!("#6386C9B2"),
        }
    }

    /// The color we use to mean "loop all the data"
    pub fn loop_everything_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => Color32::from_rgb(2, 80, 45), // from figma 2023-02-09
            Theme::Light => hex_color!("#06A35C"),
        }
    }

    /// Used by the "add view or container" modal.
    pub fn thumbnail_background_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S250)),
            Theme::Light => self.color(ColorToken::gray(S800)),
        }
    }

    pub fn example_card_background_color(&self) -> Color32 {
        //TODO(ab): as per figma, use design tokens instead
        match self.theme {
            Theme::Dark => Color32::WHITE.gamma_multiply(0.04),
            Theme::Light => self.color(ColorToken::gray(S850)),
        }
    }

    pub fn breadcrumb_text_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => egui::hex_color!("#6A8CD0"),
            Theme::Light => self.color(ColorToken::blue(S300)),
        }
    }

    pub fn breadcrumb_separator_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::blue(S500)),
            Theme::Light => self.color(ColorToken::blue(S400)),
        }
    }

    pub fn blueprint_time_panel_bg_fill(&self) -> Color32 {
        match self.theme {
            Theme::Dark => egui::hex_color!("#141326"),
            Theme::Light => self.color(ColorToken::blue(S900)),
        }
    }

    /// Color for notification panel background
    pub fn notification_panel_background_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S150)),
            Theme::Light => self.color(ColorToken::gray(S850)),
        }
    }

    /// Color for notification background
    pub fn notification_background_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S200)),
            Theme::Light => self.color(ColorToken::gray(S800)),
        }
    }

    pub fn table_header_bg_fill(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S150)),
            Theme::Light => self.color(ColorToken::gray(S850)),
        }
    }

    pub fn table_header_stroke_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S300)),
            Theme::Light => self.color(ColorToken::gray(S700)),
        }
    }

    pub fn table_interaction_hovered_bg_stroke(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color_table.gray(S300),
            Theme::Light => self.color_table.gray(S700),
        }
    }

    pub fn table_interaction_active_bg_stroke(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color_table.gray(S350),
            Theme::Light => self.color_table.gray(S650),
        }
    }

    pub fn table_interaction_noninteractive_bg_stroke(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color_table.gray(S200),
            Theme::Light => self.color_table.gray(S800),
        }
    }
}
