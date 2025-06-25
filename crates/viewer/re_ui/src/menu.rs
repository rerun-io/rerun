use crate::DesignTokens;
use egui::style::StyleModifier;
use egui::{Frame, InnerResponse, Ui};

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
