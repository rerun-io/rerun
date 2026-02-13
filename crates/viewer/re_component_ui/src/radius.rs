use egui::NumExt as _;
use re_format::format_f32;
use re_sdk_types::components::Radius;
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::response_utils::response_with_changes_of_inner;

pub fn edit_radius_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Radius>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut abs_value = value.0.abs();
        let speed = (abs_value * 0.01).at_least(0.001);

        let drag_response = ui.add(
            egui::DragValue::new(&mut abs_value)
                .range(0.0..=f32::INFINITY)
                .speed(speed),
        );

        let mut is_scene_units = value.scene_units().is_some();
        let selected_label = label_for_unit(is_scene_units);

        let combobox_response = egui::ComboBox::from_id_salt("units")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut is_scene_units, true, label_for_unit(true))
                    | ui.selectable_value(&mut is_scene_units, false, label_for_unit(false))
            });

        if combobox_response
            .inner
            .as_ref()
            .is_some_and(|r| r.changed())
        {
            // When we change the type of units,the value is likely going to be _very wrong_.
            // Unfortunately, we don't have knowledge of a fallback here, so we use hardcoded "reasonable" values.
            //
            // Careful, if these "start values" are too big, they may cause overdraw issues on point clouds too often
            if is_scene_units {
                abs_value = 0.1;
            } else {
                abs_value = 2.5;
            }
        }

        if is_scene_units {
            *value = Radius::new_scene_units(abs_value);
        } else {
            *value = Radius::new_ui_points(abs_value);
        }

        drag_response | response_with_changes_of_inner(combobox_response)
    } else {
        let abs_value = value.0.abs();
        let is_scene_units = value.scene_units().is_some();
        ui.label(format!(
            "{} {}",
            format_f32(abs_value),
            label_for_unit(is_scene_units)
        ))
    }
}

fn label_for_unit(is_scene_units: bool) -> &'static str {
    if is_scene_units {
        "scene units"
    } else {
        "ui points"
    }
}
