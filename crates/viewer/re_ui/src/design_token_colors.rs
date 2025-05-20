#![allow(clippy::unwrap_used)]
#![allow(clippy::enum_glob_use)] // Nice to have for the color variants

use egui::Color32;

use crate::{color_table::ColorToken, DesignTokens};

impl DesignTokens {
    /// Get the [`Color32`] corresponding to the provided [`ColorToken`].
    // TODO: make private
    #[inline]
    pub fn color(&self, token: ColorToken) -> Color32 {
        self.color_table.get(token)
    }
}
