use crate::{UiExt as _, list_item};

/// A collapsible section header, with support for optional help tooltip and button.
///
/// It toggles on click.
#[allow(clippy::type_complexity)]
pub struct SectionCollapsingHeader<'a> {
    label: egui::WidgetText,
    default_open: bool,
    button: Option<Box<dyn list_item::ItemButton + 'a>>,
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
    pub fn button(mut self, button: impl list_item::ItemButton + 'a) -> Self {
        self.button = Some(Box::new(button));
        self
    }

    /// Set the help text tooltip to be shown in the header.
    //TODO(#6191): the help button should be just another `impl ItemButton`.
    #[inline]
    pub fn help_text(mut self, help: impl Into<egui::WidgetText>) -> Self {
        let help = help.into();
        self.help = Some(Box::new(move |ui| {
            ui.label(help);
        }));
        self
    }

    /// Set the help markdown tooltip to be shown in the header.
    //TODO(#6191): the help button should be just another `impl ItemButton`.
    #[inline]
    pub fn help_markdown(mut self, help: &'a str) -> Self {
        self.help = Some(Box::new(move |ui| {
            ui.markdown_ui(help);
        }));
        self
    }

    /// Set the help UI closure to be shown in the header.
    //TODO(#6191): the help button should be just another `impl ItemButton`.
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
                    let button_response = button.map(|button| button.ui(ui));
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
            .force_background(ui.design_tokens().section_collapsing_header_color)
            .show_hierarchical_with_children_unindented(ui, id, default_open, content, |ui| {
                //TODO(ab): this space is not desirable when the content actually is list items
                ui.add_space(4.0); // Add space only if there is a body to make minimized headers stick together.
                add_body(ui);
                ui.add_space(4.0); // Same here
            });

        if resp.item_response.clicked() {
            // `show_hierarchical_with_children_unindented` already toggles on double-click,
            // but we are _only_ a collapsing header, so we should also toggle on normal click:
            if let Some(mut state) = egui::collapsing_header::CollapsingState::load(ui.ctx(), id) {
                state.toggle(ui);
                state.store(ui.ctx());
            }
        }

        egui::CollapsingResponse {
            header_response: resp.item_response,
            body_response: resp.body_response.map(|r| r.response),
            body_returned: None,
            openness: resp.openness,
        }
    }
}
