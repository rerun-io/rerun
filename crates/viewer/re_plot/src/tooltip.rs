use crate::legend::LegendSwatch;
use egui::{Atom, AtomExt as _, AtomLayout, TextWrapMode};
use re_ui::{DesignTokens, UiExt as _};

/// Shows a plot tooltip at the pointer for the given response.
///
/// The tooltip has a small header label, followed by a value line with a color swatch,
/// series label, and right-aligned value text.
pub fn show_plot_tooltip(
    ui: &egui::Ui,
    response: &egui::Response,
    id_salt: egui::Id,
    header: &str,
    label: &str,
    value: &str,
    color: egui::Color32,
) {
    plot_tooltip(ui, response, id_salt).show(|ui| {
        let tokens = ui.tokens();

        ui.label(
            egui::RichText::new(header)
                .size(DesignTokens::combo_item_small_font_size())
                .color(tokens.text_default),
        );

        ui.add_space(tokens.text_to_icon_padding());

        plot_tooltip_label_value(ui, label, value, color);
    });
}

/// Creates a plot tooltip for the given response with Rerun's plot tooltip settings.
///
/// Temporarily overrides the global egui style to use a shorter tooltip delay (150ms)
/// and keeps the tooltip visible once shown. Restores the original style after creation.
fn plot_tooltip<'a>(
    ui: &egui::Ui,
    response: &'a egui::Response,
    id_salt: egui::Id,
) -> egui::Tooltip<'a> {
    let prev_style = ui.ctx().global_style();
    ui.ctx().global_style_mut(|style| {
        style.interaction.tooltip_delay = 0.15;
        style.interaction.tooltip_grace_time = 3.0;
    });

    let is_open = response.enabled() && egui::Tooltip::should_show_tooltip(response, true);
    let mut tooltip = egui::Tooltip::for_widget(response);
    tooltip.popup = tooltip.popup.open(is_open).id(response.id.with(id_salt));

    ui.ctx().set_global_style(prev_style);

    tooltip
        .at_pointer()
        .gap(f32::from(ui.tokens().view_padding()))
}

/// Renders a single value line in the plot tooltip with truncation.
///
/// Max total width is 500px. The label is marked as `shrink` so it truncates
/// first when space is tight, while the value stays fully visible.
pub fn plot_tooltip_label_value(
    ui: &mut egui::Ui,
    label_text: &str,
    value_text: &str,
    color: egui::Color32,
) {
    const MAX_WIDTH: f32 = 500.0;
    const SWATCH_GAP: f32 = 6.0;
    const LABEL_VALUE_GAP: f32 = 16.0;

    let tokens = ui.tokens();

    let label = egui::RichText::new(label_text)
        .color(tokens.list_item_noninteractive_text)
        .atom_shrink(true);
    let value = egui::RichText::new(value_text).color(tokens.list_item_strong_text);

    let atoms = (
        LegendSwatch::atom(),
        label,
        Atom::default().atom_size(egui::vec2(LABEL_VALUE_GAP - SWATCH_GAP * 2.0, 0.0)),
        value,
    );

    ui.set_max_width(MAX_WIDTH);

    let atom_layout = AtomLayout::new(atoms)
        .gap(SWATCH_GAP)
        .max_width(MAX_WIDTH)
        .sense(egui::Sense::hover())
        .wrap_mode(TextWrapMode::Truncate)
        .allocate(ui);

    let atom_response = atom_layout.paint(ui);

    LegendSwatch {
        color,
        visible: true,
    }
    .paint(ui, &atom_response);
}
