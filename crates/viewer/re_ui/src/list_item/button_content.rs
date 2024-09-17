use crate::list_item::{ContentContext, ListItemContent};

/// Simple [`ListItemContent`] to easily display a button in a [`crate::list_item::ListItem`]-based UI.
pub struct ButtonContent<'a> {
    label: egui::WidgetText,
    enabled: bool,
    on_click: Option<Box<dyn FnOnce() + 'a>>,
    hover_text: Option<String>,
}

impl<'a> ButtonContent<'a> {
    #[must_use]
    pub fn new(label: impl Into<egui::WidgetText>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            on_click: None,
            hover_text: None,
        }
    }

    /// Sets whether the button is enabled.
    #[inline]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Called when the button is clicked.
    #[inline]
    pub fn on_click(mut self, on_click: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }

    /// Sets the hover text of the button.
    #[inline]
    pub fn hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.hover_text = Some(hover_text.into());
        self
    }
}

impl ListItemContent for ButtonContent<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        let Self {
            label,
            enabled,
            on_click,
            hover_text,
        } = *self;

        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(context.rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        // Compensate for the button padding such that the button text is aligned with other list
        // item contents.
        ui.add_space(-ui.spacing().button_padding.x);

        let response = ui.add_enabled(enabled, egui::Button::new(label));
        if let Some(on_click) = on_click {
            if response.clicked() {
                on_click();
            }
        }

        if let Some(hover_text) = hover_text {
            response.on_hover_text(hover_text);
        }
    }
}
