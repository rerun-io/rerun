use crate::UiExt as _;
use egui::{InnerResponse, Rect, Shape};

/// Static title bar that separates a panel into sections.
///
/// It draws a full-span background, a label, and up to two groups of buttons: one directly
/// after the label, and one aligned to the right edge of the panel.
///
/// This title bar is meant to be used in a panel with proper inner margin and clip rectangle
/// set.
pub struct PanelTitleBar {
    label: egui::RichText,
    hover_text: Option<egui::WidgetText>,
}

impl PanelTitleBar {
    /// Create a title bar with the given label.
    pub fn new(label: impl Into<egui::RichText>) -> Self {
        Self {
            label: label.into(),
            hover_text: None,
        }
    }

    /// Tooltip shown when hovering the label.
    #[inline]
    pub fn hover_text(mut self, hover_text: impl Into<egui::WidgetText>) -> Self {
        self.hover_text = Some(hover_text.into());
        self
    }

    /// Show the bar with no buttons.
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        self.show_impl(ui, |_ui| {}, |_ui| {}).0
    }

    /// Show the bar with buttons directly after the label.
    pub fn show_with_left_buttons<L>(
        self,
        ui: &mut egui::Ui,
        add_left_buttons: impl FnOnce(&mut egui::Ui) -> L,
    ) -> L {
        self.show_impl(ui, add_left_buttons, |_ui| {}).1
    }

    /// Show the bar with buttons aligned to its right edge.
    pub fn show_with_right_buttons<R>(
        self,
        ui: &mut egui::Ui,
        add_right_buttons: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        self.show_impl(ui, |_ui| {}, add_right_buttons).2
    }

    /// The left buttons are laid out left-to-right, the right buttons right-to-left.
    ///
    /// Returns the response of the whole bar, plus whatever the two closures returned.
    fn show_impl<L, R>(
        self,
        ui: &mut egui::Ui,
        add_left_buttons: impl FnOnce(&mut egui::Ui) -> L,
        add_right_buttons: impl FnOnce(&mut egui::Ui) -> R,
    ) -> (egui::Response, L, R) {
        let Self { label, hover_text } = self;

        let tokens = ui.tokens();
        let height = tokens.title_bar_height();
        let background = tokens.section_header_color;

        // egui inserts this after every widget, including after the label.

        let shape_idx = ui.painter().add(Shape::Noop);

        let InnerResponse {
            response,
            inner: (left, right),
        } = ui.scope(|ui| {
            egui::Sides::new().height(height).show(
                ui,
                |ui| {
                    let label_response = ui.strong(label);
                    if let Some(hover_text) = hover_text {
                        label_response.on_hover_text(hover_text);
                    }
                    add_left_buttons(ui)
                },
                add_right_buttons,
            )
        });

        ui.painter().set(
            shape_idx,
            Shape::rect_filled(
                Rect::from_x_y_ranges(ui.full_span(), response.rect.y_range()),
                0,
                background,
            ),
        );

        (response, left, right)
    }
}
