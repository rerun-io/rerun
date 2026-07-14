use egui::Ui;

use crate::list_item::{ContentContext, DesiredWidth, ListItemContent};

/// [`ListItemContent`] that displays the content rect.
#[derive(Debug, Clone, Default)]
pub struct DebugContent {
    label: String,
    desired_width: DesiredWidth,
}

impl DebugContent {
    #[inline]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    #[inline]
    pub fn with_desired_width(mut self, desired_width: DesiredWidth) -> Self {
        self.desired_width = desired_width;
        self
    }
}

impl ListItemContent for DebugContent {
    fn ui(self: Box<Self>, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        ui.debug_painter()
            .debug_rect(context.rect, egui::Color32::DARK_GREEN, self.label);
    }

    fn desired_width(&self, _ui: &Ui) -> DesiredWidth {
        self.desired_width
    }
}
