//! Custom visualizer UI for the state timeline view selection panel.
//!
//! Shows an editable list of state value→label+color+visibility mappings per entity.

use re_component_ui::color_swatch::ColorSwatch;
use re_sdk_types::archetypes::StateConfiguration;
use re_sdk_types::components::{Color, Text, Visible};
use re_sdk_types::datatypes::{Bool, Rgba32};
use re_sdk_types::{ComponentDescriptor, Loggable as _};
use re_ui::UiExt as _;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{DataResultInteractionAddress, Item, MaybeMutRef};

use crate::data::StateValueKind;
use crate::visualizer::current_state_value_kind;

/// One row in the state configuration editor.
struct StateMapping {
    value: String,
    label: String,
    color: Rgba32,
    visible: bool,
}

/// Canonical value strings used to back boolean lanes — kept in sync with
/// `StateLabel for bool` in the visualizer so the editor's lookups match what the renderer
/// produces.
const BOOL_VALUES: [&str; 2] = ["true", "false"];

/// Which parts of the [`StateConfiguration`] the row UI has changed.
///
/// Tracked so we only persist the components the user actually touched — avoids locking in
/// hash-derived default colors or empty labels when an unrelated field is edited.
#[derive(Default, Clone, Copy)]
struct ChangeFlags {
    values: bool,
    labels: bool,
    colors: bool,
    visible: bool,
}

impl ChangeFlags {
    fn merge(&mut self, other: Self) {
        self.values |= other.values;
        self.labels |= other.labels;
        self.colors |= other.colors;
        self.visible |= other.visible;
    }
}

/// Editable state configuration for a single visualizer instruction.
pub fn state_config_editor(
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
            StateConfiguration::descriptor_values().component,
            StateConfiguration::descriptor_labels().component,
            StateConfiguration::descriptor_colors().component,
            StateConfiguration::descriptor_visible().component,
        ],
        Some(instruction),
    );

    let values = extract_texts(&query_result, &StateConfiguration::descriptor_values());
    let labels = extract_texts(&query_result, &StateConfiguration::descriptor_labels());
    let colors = extract_colors(&query_result, &StateConfiguration::descriptor_colors());
    let visible = extract_bools(&query_result, &StateConfiguration::descriptor_visible());

    // Selection/hover item for this entity.
    let item = Item::DataResult(DataResultInteractionAddress {
        view_id: ctx.view_id,
        instance_path: InstancePath::from(entity_path.clone()),
        visualizer: Some(instruction.id),
    });

    let id = ui.make_persistent_id(("state_config", entity_path));

    // For boolean lanes, the only meaningful values are `"true"` and `"false"`, so the editor
    // renders a simplified two-row UI; everything else gets the freeform editor.
    let kind = current_state_value_kind(ctx, data_result, instruction);
    let is_bool_lane = kind == Some(StateValueKind::Bool);

    let (changes, mappings) = if is_bool_lane {
        render_bool_mapping_rows(ui, entity_path, &values, &labels, &colors, &visible)
    } else {
        render_freeform_mapping_rows(ui, entity_path, &values, &labels, &colors, &visible)
    };

    let values_changed = changes.values;
    let labels_changed = changes.labels;
    let colors_changed = changes.colors;
    let visible_changed = changes.visible;

    // Write only the components that actually changed.
    if values_changed {
        let new_values: Vec<Text> = mappings
            .iter()
            .map(|m| Text::from(m.value.as_str()))
            .collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StateConfiguration::descriptor_values(),
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
            &StateConfiguration::descriptor_labels(),
            &new_labels,
        );
    }
    if colors_changed {
        let new_colors: Vec<Color> = mappings.iter().map(|m| Color::from(m.color)).collect();
        instruction.save_override(
            ctx.viewer_ctx,
            &StateConfiguration::descriptor_colors(),
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
            &StateConfiguration::descriptor_visible(),
            &new_visible,
        );
    }

    // Handle hover/click selection.
    let response = ui.interact(egui::Rect::NOTHING, id, egui::Sense::hover());
    if response.hovered() {
        ctx.viewer_ctx.selection_state().set_hovered(item.clone());
    }
}

/// A single row of the value mapping UI.
fn render_mapping_row_contents(
    ui: &mut egui::Ui,
    mapping: &mut StateMapping,
    value_editable: bool,
) -> ChangeFlags {
    let mut changes = ChangeFlags::default();

    let mut value_edit = egui::TextEdit::singleline(&mut mapping.value)
        .desired_width(60.0)
        .interactive(value_editable)
        .hint_text("value");
    if !value_editable {
        // Tone down the locked-in value so it reads as informational rather than editable.
        value_edit = value_edit.text_color(ui.tokens().text_subdued);
    }
    let value_response = ui.add(value_edit);
    if value_editable && (value_response.lost_focus() || value_response.changed()) {
        changes.values = true;
    }

    ui.label("\u{2192}");

    if ui.visibility_toggle_button(&mut mapping.visible).changed() {
        changes.visible = true;
    }

    let mut color_ref = MaybeMutRef::MutRef(&mut mapping.color);
    if ui.add(ColorSwatch::new(&mut color_ref)).changed() {
        changes.colors = true;
    }

    let label_response = ui.add(
        egui::TextEdit::singleline(&mut mapping.label)
            .desired_width(f32::INFINITY)
            .hint_text("label"),
    );
    if label_response.lost_focus() || label_response.changed() {
        changes.labels = true;
    }

    changes
}

/// Render the freeform mapping rows and a bottom "Add mapping" button.
fn render_freeform_mapping_rows(
    ui: &mut egui::Ui,
    entity_path: &re_log_types::EntityPath,
    values: &[String],
    labels: &[String],
    colors: &[Rgba32],
    visible: &[bool],
) -> (ChangeFlags, Vec<StateMapping>) {
    let mut mappings: Vec<StateMapping> = (0..values.len())
        .map(|i| {
            let value = values.get(i).cloned().unwrap_or_default();
            let color = colors
                .get(i)
                .copied()
                .unwrap_or_else(|| default_color(&value));
            StateMapping {
                value,
                label: labels.get(i).cloned().unwrap_or_default(),
                color,
                visible: visible.get(i).copied().unwrap_or(true),
            }
        })
        .collect();

    let mut changes = ChangeFlags::default();
    let mut remove_idx = None;

    for (i, mapping) in mappings.iter_mut().enumerate() {
        let row_id = ui.make_persistent_id(("state_row", entity_path, i));
        ui.push_id(row_id, |ui| {
            let (row_changes, remove_clicked) = egui::Sides::new().shrink_left().show(
                ui,
                |ui| render_mapping_row_contents(ui, mapping, true),
                |ui| {
                    ui.small_icon_button(&re_ui::icons::REMOVE, "Remove mapping")
                        .clicked()
                },
            );
            changes.merge(row_changes);
            if remove_clicked {
                remove_idx = Some(i);
            }
            ui.add_space(6.0);
        });
    }

    if let Some(idx) = remove_idx {
        mappings.remove(idx);
        // Every array shrinks, so every array needs to be rewritten.
        changes.merge(ChangeFlags {
            values: true,
            labels: true,
            colors: true,
            visible: true,
        });
    }

    if ui
        .small_icon_button(&re_ui::icons::ADD, "Add state mapping")
        .clicked()
    {
        mappings.push(StateMapping {
            value: String::new(),
            label: String::new(),
            color: default_color(""),
            visible: true,
        });
        // New row: values (and labels, as an aligned empty) need to grow. Colors/visible fall
        // back to their defaults at render time until the user explicitly sets them.
        changes.values = true;
        changes.labels = true;
    }

    (changes, mappings)
}

/// Render the simplified two-row editor for boolean lanes.
fn render_bool_mapping_rows(
    ui: &mut egui::Ui,
    entity_path: &re_log_types::EntityPath,
    values: &[String],
    labels: &[String],
    colors: &[Rgba32],
    visible: &[bool],
) -> (ChangeFlags, Vec<StateMapping>) {
    let mut mappings: Vec<StateMapping> = BOOL_VALUES
        .iter()
        .map(|v| {
            let existing = values.iter().position(|stored| stored == *v);
            let color = existing
                .and_then(|i| colors.get(i).copied())
                .unwrap_or_else(|| default_color(v));
            StateMapping {
                value: (*v).to_owned(),
                label: existing
                    .and_then(|i| labels.get(i).cloned())
                    .unwrap_or_default(),
                color,
                visible: existing
                    .and_then(|i| visible.get(i).copied())
                    .unwrap_or(true),
            }
        })
        .collect();

    // Force-write the canonical bool values list if the stored values don't already match.
    let bool_values_need_seeding = !(values.len() == BOOL_VALUES.len()
        && std::iter::zip(values, BOOL_VALUES).all(|(stored, canonical)| stored == canonical));

    let mut changes = ChangeFlags::default();

    for (i, mapping) in mappings.iter_mut().enumerate() {
        let row_id = ui.make_persistent_id(("state_row", entity_path, i));
        ui.push_id(row_id, |ui| {
            let (row_changes, ()) = egui::Sides::new().shrink_left().show(
                ui,
                |ui| render_mapping_row_contents(ui, mapping, false),
                |_ui| {},
            );
            changes.merge(row_changes);
            ui.add_space(6.0);
        });
    }

    if bool_values_need_seeding && (changes.labels || changes.colors || changes.visible) {
        changes.values = true;
    }

    (changes, mappings)
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
