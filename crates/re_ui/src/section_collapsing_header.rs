use crate::{list_item, DesignTokens, Icon, UiExt as _};

/// Icon button to be used in the header of a panel.
pub struct HeaderMenuButton<'a> {
    pub icon: &'static Icon,
    pub add_contents: Box<dyn FnOnce(&mut egui::Ui) + 'a>,
    pub enabled: bool,
    pub hover_text: Option<String>,
    pub disabled_hover_text: Option<String>,
}

impl<'a> HeaderMenuButton<'a> {
    pub fn new(icon: &'static Icon, add_contents: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        Self {
            icon,
            add_contents: Box::new(add_contents),
            enabled: true,
            hover_text: None,
            disabled_hover_text: None,
        }
    }

    /// Sets enable/disable state of the button.
    #[inline]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets text shown when the button hovered.
    #[inline]
    pub fn hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.hover_text = Some(hover_text.into());
        self
    }

    /// Sets text shown when the button is disabled and hovered.
    #[inline]
    pub fn disabled_hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.disabled_hover_text = Some(hover_text.into());
        self
    }

    fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let mut response = egui::menu::menu_image_button(
                ui,
                ui.small_icon_button_widget(self.icon),
                self.add_contents,
            )
            .response;
            if let Some(hover_text) = self.hover_text {
                response = response.on_hover_text(hover_text);
            }
            if let Some(disabled_hover_text) = self.disabled_hover_text {
                response = response.on_disabled_hover_text(disabled_hover_text);
            }

            response
        })
        .inner
    }
}

/// A collapsible section header, with support for optional help tooltip and button.
#[allow(clippy::type_complexity)]
pub struct SectionCollapsingHeader<'a> {
    label: egui::WidgetText,
    default_open: bool,
    button: Option<HeaderMenuButton<'a>>,
    help: Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>,
}

impl<'a> SectionCollapsingHeader<'a> {
    /// Create a new [`Self`].
    ///
    /// See also [`crate::UiExt::section_collapsing_header`]
    pub fn new(label: impl Into<egui::WidgetText>) -> Self {
        Self {
            label: label.into(),
            default_open: true,
            button: None,
            help: None,
        }
    }

    /// Set the default open state of the section header.
    ///
    /// Defaults to `true`.
    #[inline]
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Set the button to be shown in the header.
    #[inline]
    pub fn button(mut self, button: HeaderMenuButton<'a>) -> Self {
        self.button = Some(button);
        self
    }

    /// Set the help text tooltip to be shown in the header.
    #[inline]
    pub fn help_text(mut self, help: impl Into<egui::WidgetText>) -> Self {
        let help = help.into();
        self.help = Some(Box::new(move |ui| {
            ui.label(help);
        }));
        self
    }

    /// Set the help UI closure to be shown in the header.
    #[inline]
    pub fn help_ui(mut self, help: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        self.help = Some(Box::new(help));
        self
    }

    /// Display the header.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        add_body: impl FnOnce(&mut egui::Ui),
    ) -> egui::CollapsingResponse<()> {
        let Self {
            label,
            default_open,
            button,
            help,
        } = self;

        let id = ui.make_persistent_id(label.text());

        let mut content = list_item::LabelContent::new(label);
        if button.is_some() || help.is_some() {
            content = content
                .with_buttons(|ui| {
                    let button_response = button.map(|button| button.show(ui));
                    let help_response = help.map(|help| ui.help_hover_button().on_hover_ui(help));

                    match (button_response, help_response) {
                        (Some(button_response), Some(help_response)) => {
                            button_response | help_response
                        }
                        (Some(response), None) | (None, Some(response)) => response,
                        (None, None) => unreachable!("at least one of button or help is set"),
                    }
                })
                .always_show_buttons(true);
        }

        let resp = list_item::ListItem::new()
            .interactive(true)
            .force_background(DesignTokens::large_collapsing_header_color())
            .show_hierarchical_with_children_unindented(ui, id, default_open, content, |ui| {
                //TODO(ab): this space is not desirable when the content actually is list items
                ui.add_space(4.0); // Add space only if there is a body to make minimized headers stick together.
                add_body(ui);
                ui.add_space(4.0); // Same here
            });

        egui::CollapsingResponse {
            header_response: resp.item_response,
            body_response: resp.body_response.map(|r| r.response),
            body_returned: None,
            openness: resp.openness,
        }
    }
}
