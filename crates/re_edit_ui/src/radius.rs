use egui::NumExt as _;
use re_types::components::Radius;
use re_viewer_context::ViewerContext;

use crate::response_utils::response_with_changes_of_inner;

pub fn edit_radius_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Radius,
) -> egui::Response {
    let mut abs_value = value.0.abs();
    let speed = (abs_value * 0.01).at_least(0.001);

    let drag_response = ui.add(
        egui::DragValue::new(&mut abs_value)
            .clamp_to_range(false)
            .range(0.0..=f32::INFINITY)
            .speed(speed),
    );

    let mut is_scene_units = value.scene_units().is_some();
    let selected_label = label_for_unit(is_scene_units);

    if ui.is_enabled() {
        let combobox_response = egui::ComboBox::from_id_source("units")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut is_scene_units, true, label_for_unit(true))
                    | ui.selectable_value(&mut is_scene_units, false, label_for_unit(false))
            });

        if combobox_response
            .inner
            .as_ref()
            .map_or(false, |r| r.changed())
        {
            // When we change the type of units,the value is likely going to be _very wrong_.
            // Unfortunately, we don't have knowledge of a fallback here, so we to hardcoded "reasonable" values.
            if is_scene_units {
                abs_value = 0.5;
            } else {
                abs_value = 2.5;
            };
        }

        if is_scene_units {
            *value = Radius::new_scene_units(abs_value);
        } else {
            *value = Radius::new_ui_points(abs_value);
        }

        drag_response | response_with_changes_of_inner(combobox_response)
    } else {
        // Don't show the combo box drop down if this is disabled ui.
        // TODO(#6661): This shouldn't happen on disabled ui, but rather when this is simply not editable.
        ui.selectable_label(false, selected_label)
    }
}

fn label_for_unit(is_scene_units: bool) -> &'static str {
    if is_scene_units {
        "scene units"
    } else {
        "ui points"
    }
}
