use crate::list_item::{ItemButtons, ListItemContentButtonsExt};
use crate::{UiExt as _, list_item};

/// A collapsible section header, with support for optional help tooltip and button.
///
/// It toggles on click.
#[allow(clippy::type_complexity)]
pub struct SectionCollapsingHeader<'a> {
    label: egui::WidgetText,
    default_open: bool,
    buttons: ItemButtons<'a>,
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
            buttons: ItemButtons::default(),
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
    pub fn button(mut self, button: impl egui::Widget + 'a) -> Self {
        self.buttons.add(button);
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
            buttons,
            help,
        } = self;

        let id = ui.make_persistent_id(label.text());

        let mut content = list_item::LabelContent::new(label);
        *content.buttons_mut() = buttons;

        let resp = list_item::ListItem::new()
            .interactive(true)
            .force_background(ui.tokens().section_header_color)
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
