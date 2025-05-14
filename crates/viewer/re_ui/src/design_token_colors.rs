#![allow(clippy::unwrap_used)]
#![allow(clippy::enum_glob_use)] // Nice to have for the color variants

use egui::{Color32, Theme};

use crate::{
    DesignTokens,
    color_table::{ColorToken, Scale::*},
};

impl DesignTokens {
    /// Get the [`Color32`] corresponding to the provided [`ColorToken`].
    #[inline]
    pub fn color(&self, token: ColorToken) -> Color32 {
        self.color_table.get(token)
    }

    pub fn strong_fg_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => Color32::WHITE,
            Theme::Light => Color32::BLACK,
        }
    }

    pub fn info_log_text_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => Color32::LIGHT_GREEN,
            Theme::Light => Color32::DARK_GREEN,
        }
    }

    pub fn debug_log_text_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => Color32::LIGHT_BLUE,
            Theme::Light => Color32::DARK_BLUE,
        }
    }

    pub fn trace_log_text_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => Color32::LIGHT_GRAY,
            Theme::Light => Color32::DARK_GRAY,
        }
    }

    /// Color of an icon next to a label
    pub fn label_button_icon_color(&self) -> Color32 {
        match self.theme {
            Theme::Dark => self.color_table.gray(S500),
            Theme::Light => self.color_table.gray(S550),
        }
    }

    /// The color for the background of [`crate::SectionCollapsingHeader`].
    pub fn section_collapsing_header_color(&self) -> Color32 {
        // same as visuals.widgets.inactive.bg_fill
        match self.theme {
            Theme::Dark => self.color(ColorToken::gray(S200)),
            Theme::Light => self.color(ColorToken::gray(S900)),
        }
    }

    /// The color we use to mean "loop this selection"
    pub fn loop_selection_color() -> Color32 {
        Color32::from_rgb(1, 37, 105) // from figma 2023-02-09
    }

    /// The color we use to mean "loop all the data"
    pub fn loop_everything_color() -> Color32 {
        Color32::from_rgb(2, 80, 45) // from figma 2023-02-09
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
            Theme::Light => Color32::BLACK.gamma_multiply(0.05),
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
}
