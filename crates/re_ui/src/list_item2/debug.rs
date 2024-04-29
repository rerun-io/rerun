use crate::list_item2::{ContentContext, DesiredWidth, ListItemContent};
use crate::ReUi;
use egui::Ui;

pub struct EmptyContent;

impl ListItemContent for EmptyContent {
    fn ui(&mut self, _re_ui: &crate::ReUi, _ui: &egui::Ui, _context: &ContentContext<'_>) {}
}

#[derive(Debug, Clone, Default)]
pub struct DebugContent {
    desired_width: DesiredWidth,
}

impl ListItemContent for DebugContent {
    fn ui(&mut self, _re_ui: &crate::ReUi, ui: &egui::Ui, context: &ContentContext<'_>) {
        ui.ctx()
            .debug_painter()
            .debug_rect(context.rect, egui::Color32::DARK_GREEN, "")
    }

    fn desired_width(&self, _re_ui: &ReUi, _ui: &Ui) -> DesiredWidth {
        self.desired_width
    }
}
