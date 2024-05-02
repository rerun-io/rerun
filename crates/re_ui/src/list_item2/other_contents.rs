use crate::list_item2::{ContentContext, DesiredWidth, ListItemContent};
use crate::ReUi;
use egui::Ui;

/// Empty list item content.
pub struct EmptyContent;

impl ListItemContent for EmptyContent {
    fn ui(
        self: Box<Self>,
        _re_ui: &crate::ReUi,
        _ui: &mut egui::Ui,
        _context: &ContentContext<'_>,
    ) {
    }
}

/// [`ListItemContent`] that delegates to a closure.
#[allow(clippy::type_complexity)]
pub struct CustomContent<'a> {
    ui: Box<dyn FnOnce(&crate::ReUi, &mut egui::Ui, &ContentContext<'_>) + 'a>,
}

impl<'a> CustomContent<'a> {
    pub fn new(ui: impl FnOnce(&crate::ReUi, &mut egui::Ui, &ContentContext<'_>) + 'a) -> Self {
        Self { ui: Box::new(ui) }
    }
}

impl ListItemContent for CustomContent<'_> {
    fn ui(self: Box<Self>, re_ui: &crate::ReUi, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        (self.ui)(re_ui, ui, context);
    }
}

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
    fn ui(self: Box<Self>, _re_ui: &crate::ReUi, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        ui.ctx()
            .debug_painter()
            .debug_rect(context.rect, egui::Color32::DARK_GREEN, self.label);
    }

    fn desired_width(&self, _re_ui: &ReUi, _ui: &Ui) -> DesiredWidth {
        self.desired_width
    }
}
