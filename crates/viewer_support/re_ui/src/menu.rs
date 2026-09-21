use egui::style::StyleModifier;
use egui::{Frame, InnerResponse, Ui};

use crate::DesignTokens;

pub fn menu_style() -> StyleModifier {
    StyleModifier::new(|style| {
        egui::containers::menu::menu_style(style);
        style.spacing.interact_size.y = 24.0;
        style.spacing.menu_margin = 4.0.into();
        style.spacing.icon_spacing = 6.0;
        style.spacing.button_padding.x = DesignTokens::menu_button_padding();
        style.spacing.item_spacing.y = 0.0;

        let widgets = &mut style.visuals.widgets;
        for visual in [
            &mut widgets.inactive,
            &mut widgets.active,
            &mut widgets.hovered,
            &mut widgets.open,
            &mut widgets.noninteractive,
        ] {
            visual.expansion = 0.0;
            visual.corner_radius = 4.0.into();
        }
    })
}

/// Since the menu buttons have a transparent background, we have to manually align
/// non-button widgets to visually align them.
pub fn align_non_button_menu_items<T>(
    ui: &mut Ui,
    content: impl FnOnce(&mut Ui) -> T,
) -> InnerResponse<T> {
    Frame::new()
        .inner_margin(DesignTokens::menu_button_padding())
        .show(ui, content)
}

/// Show a context menu on right clicks anywhere within this widget, even if covered by a click
/// sensing widget on the same layer.
///
/// Same as [`egui::Response::container_context_menu`], but styled with [`menu_style`].
// TODO(lucasmerlin): Remove this once menus can be styled via `StyleProvider`
pub fn container_context_menu<R>(
    response: &egui::Response,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    egui::Popup::menu(response)
        .open_memory(if response.container_secondary_clicked() {
            Some(egui::SetOpenCommand::Bool(true))
        } else if response.container_clicked() {
            // Explicitly close the menu if the container was clicked,
            // otherwise the context menu would stay open when clicking elsewhere in the container.
            Some(egui::SetOpenCommand::Bool(false))
        } else {
            None
        })
        .style(menu_style())
        .at_pointer_fixed()
        .show(add_contents)
}
