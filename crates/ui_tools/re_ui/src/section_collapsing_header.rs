use egui::emath::GuiRounding as _;

use crate::list_item::{CollapseButtonPlacement, ItemButtons, ListItemContentButtonsExt};
use crate::{UiExt as _, list_item};

/// A collapsible section header, with support for optional help tooltip and button.
///
/// It toggles on click.
pub struct SectionCollapsingHeader<'a> {
    label: egui::WidgetText,
    default_open: bool,
    buttons: ItemButtons<'a>,
}

impl SectionCollapsingHeader<'_> {
    /// Create a new [`Self`].
    ///
    /// See also [`crate::UiExt::section_collapsing_header`]
    pub fn new(label: impl Into<egui::WidgetText>) -> Self {
        Self {
            label: label.into(),
            default_open: true,
            buttons: ItemButtons::default()
                // Section headers should always show buttons
                .with_always_show_buttons(true),
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
        } = self;

        let id = ui.make_persistent_id(label.text());
        let label = ui.tokens().eyebrow_text(ui.style(), label.text());

        let mut content = list_item::LabelContent::new(label);
        *content.buttons_mut() = buttons;

        let resp = list_item::ListItem::new()
            .interactive(true)
            .force_background(ui.tokens().section_header_color)
            .with_density(crate::ListItemDensity::Spacious)
            .with_top_padding(ui.tokens().section_header_top_padding())
            .with_collapse_button_placement(CollapseButtonPlacement::AfterLabel)
            .show_hierarchical_with_children_unindented(ui, id, default_open, content, |ui| {
                //TODO(ab): this space is not desirable when the content actually is list items
                ui.add_space(4.0); // Add space only if there is a body to make minimized headers stick together.
                add_body(ui);
                ui.add_space(4.0); // Same here
            });

        if ui.is_rect_visible(resp.item_response.rect) {
            let separator_y = resp
                .item_response
                .rect
                .top()
                .round_to_pixels(ui.pixels_per_point());
            ui.painter()
                .hline(ui.full_span(), separator_y, ui.tokens().divider_stroke);
        }

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

impl<'a> ListItemContentButtonsExt<'a> for SectionCollapsingHeader<'a> {
    fn buttons(&self) -> &ItemButtons<'a> {
        &self.buttons
    }

    fn buttons_mut(&mut self) -> &mut ItemButtons<'a> {
        &mut self.buttons
    }
}
