//! Custom visualizer UI for the Status view selection panel.
//!
//! Shows an editable list of status value→label+color+visibility mappings per entity.

use re_component_ui::color_swatch::ColorSwatch;
use re_sdk_types::archetypes::StatusConfiguration;
use re_sdk_types::components::{Color, Text, Visible};
use re_sdk_types::datatypes::{Bool, Rgba32};
use re_sdk_types::{ComponentDescriptor, Loggable as _};
use re_ui::UiExt as _;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{DataResultInteractionAddress, Item, MaybeMutRef};

/// One row in the status configuration editor.
struct StatusMapping {
    value: String,
    label: String,
    color: Rgba32,
    visible: bool,
}

/// Editable status configuration for a single visualizer instruction.
pub fn status_config_editor(
    ui: &mut egui::Ui,
    ctx: &re_viewer_context::ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
) {
    let entity_path = &data_result.entity_path;

    // Query current state configuration.
    let query_result = re_view::latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &ctx.current_query(),
        data_result,
        [
            StatusConfiguration::descriptor_values().component,
            StatusConfiguration::descriptor_labels().component,
            StatusConfiguration::descriptor_colors().component,
            StatusConfiguration::descriptor_visible().component,
        ],
        Some(instruction),
    );

    let values = extract_texts(&query_result, &StatusConfiguration::descriptor_values());
    let labels = extract_texts(&query_result, &StatusConfiguration::descriptor_labels());
    let colors = extract_colors(&query_result, &StatusConfiguration::descriptor_colors());
    let visible = extract_bools(&query_result, &StatusConfiguration::descriptor_visible());

    // Build the editable mapping list from whatever is already configured.
    let mut mappings: Vec<StatusMapping> = (0..values.len())
        .map(|i| {
            let value = values.get(i).cloned().unwrap_or_default();
            let color = colors
                .get(i)
                .copied()
                .unwrap_or_else(|| default_color(&value));
            StatusMapping {
                value,
                label: labels.get(i).cloned().unwrap_or_default(),
                color,
                visible: visible.get(i).copied().unwrap_or(true),
            }
        })
        .collect();

    // Selection/hover item for this entity.
    let item = Item::DataResult(DataResultInteractionAddress {
        view_id: ctx.view_id,
        instance_path: InstancePath::from(entity_path.clone()),
        visualizer: Some(instruction.id),
    });

    let id = ui.make_persistent_id(("status_config", entity_path));

    // Track which components actually changed so we only persist what the user
    // touched. That avoids locking in hash-derived default colors when the user
    // edits an unrelated field.
    let mut values_changed = false;
    let mut labels_changed = false;
    let mut colors_changed = false;
    let mut visible_changed = false;

    {
        // Mapping rows.
        let mut remove_idx = None;
        for (i, mapping) in mappings.iter_mut().enumerate() {
            let row_id = ui.make_persistent_id(("status_row", entity_path, i));
            ui.push_id(row_id, |ui| {
                ui.horizontal(|ui| {
                    // Visibility toggle.
                    if ui.visibility_toggle_button(&mut mapping.visible).changed() {
                        visible_changed = true;
                    }

                    // Color swatch (editable).
                    let mut color_ref = MaybeMutRef::MutRef(&mut mapping.color);
                    if ui.add(ColorSwatch::new(&mut color_ref)).changed() {
                        colors_changed = true;
                    }

                    // Value field.
                    ui.add_space(4.0);
                    let value_response = ui.add(
                        egui::TextEdit::singleline(&mut mapping.value)
                            .desired_width(60.0)
                            .hint_text("value"),
                    );
                    if value_response.lost_focus() || value_response.changed() {
                        values_changed = true;
                    }

                    // Arrow separator.
                    ui.label("\u{2192}");

                    // Label field.
                    let label_response = ui.add(
                        egui::TextEdit::singleline(&mut mapping.label)
                            .desired_width(80.0)
                            .hint_text("label"),
                    );
                    if label_response.lost_focus() || label_response.changed() {
                        labels_changed = true;
                    }

                    // Remove button.
                    if ui
                        .small_icon_button(&re_ui::icons::REMOVE, "Remove mapping")
                        .clicked()
                    {
                        remove_idx = Some(i);
                    }
                });
                ui.add_space(6.0);
            });
        }

        if let Some(idx) = remove_idx {
            mappings.remove(idx);
            // Every array shrinks, so every array needs to be rewritten.
            values_changed = true;
            labels_changed = true;
            colors_changed = true;
            visible_changed = true;
        }

        // Add button.
        if ui
            .small_icon_button(&re_ui::icons::ADD, "Add status mapping")
            .clicked()
        {
            mappings.push(StatusMapping {
                value: String::new(),
                label: String::new(),
                color: default_color(""),
                visible: true,
            });
            // New row: values (and labels, as an aligned empty) need to grow.
            // Colors/visible fall back to their defaults at render time until
            // the user explicitly sets them.
            values_changed = true;
            labels_changed = true;
        }
    }

    // Write only the components that actually changed.
    if values_changed {
        let new_values: Vec<Text> = mappings
            .iter()
            .map(|m| Text::from(m.value.as_str()))
            .collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StatusConfiguration::descriptor_values(),
            &new_values,
        );
    }
    if labels_changed {
        let new_labels: Vec<Text> = mappings
            .iter()
            .map(|m| Text::from(m.label.as_str()))
            .collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StatusConfiguration::descriptor_labels(),
            &new_labels,
        );
    }
    if colors_changed {
        let new_colors: Vec<Color> = mappings.iter().map(|m| Color::from(m.color)).collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StatusConfiguration::descriptor_colors(),
            &new_colors,
        );
    }
    if visible_changed {
        let new_visible: Vec<Visible> = mappings
            .iter()
            .map(|m| Visible::from(Bool(m.visible)))
            .collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StatusConfiguration::descriptor_visible(),
            &new_visible,
        );
    }

    // Handle hover/click selection.
    let response = ui.interact(egui::Rect::NOTHING, id, egui::Sense::hover());
    if response.hovered() {
        ctx.viewer_ctx.selection_state().set_hovered(item.clone());
    }
}

fn extract_texts(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    descr: &ComponentDescriptor,
) -> Vec<String> {
    let Some(raw) = query_result.get_raw_cell(descr.component) else {
        return Vec::new();
    };
    Text::from_arrow(&raw)
        .map(|texts| texts.iter().map(|t| t.to_string()).collect())
        .unwrap_or_default()
}

fn extract_colors(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    descr: &ComponentDescriptor,
) -> Vec<Rgba32> {
    let Some(raw) = query_result.get_raw_cell(descr.component) else {
        return Vec::new();
    };
    Color::from_arrow(&raw)
        .map(|colors| colors.iter().map(|c| c.0).collect())
        .unwrap_or_default()
}

fn extract_bools(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    descr: &ComponentDescriptor,
) -> Vec<bool> {
    let Some(raw) = query_result.get_raw_cell(descr.component) else {
        return Vec::new();
    };
    Visible::from_arrow(&raw)
        .map(|rows| rows.iter().map(|v| v.0.0).collect())
        .unwrap_or_default()
}

/// Stable default color for a value, matching the renderer's fallback.
///
/// Using a hash of the value keeps the color fixed as the user adds or
/// reorders rows in the editor.
#[expect(clippy::disallowed_methods)] // Data-driven visualization color, not a UI theme color.
fn default_color(value: &str) -> Rgba32 {
    const PALETTE: &[egui::Color32] = &[
        egui::Color32::from_rgb(76, 175, 80),
        egui::Color32::from_rgb(255, 183, 77),
        egui::Color32::from_rgb(66, 165, 245),
        egui::Color32::from_rgb(239, 83, 80),
        egui::Color32::from_rgb(171, 71, 188),
        egui::Color32::from_rgb(38, 198, 218),
        egui::Color32::from_rgb(255, 241, 118),
        egui::Color32::from_rgb(141, 110, 99),
    ];
    let hash = re_log_types::hash::Hash64::hash(value).hash64();
    Rgba32::from(PALETTE[(hash as usize) % PALETTE.len()])
}
