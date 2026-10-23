use egui::{InnerResponse, NumExt as _, Response};
use re_format::format_f32;
use re_sdk_types::components::Radius;
use re_viewer_context::{AppContext, MaybeMutRef};

use crate::response_utils::response_with_changes_of_inner;

fn edit_radius_magnitude_ui(abs_value: &mut f32, ui: &mut egui::Ui ) -> Response
{
    let speed = (*abs_value * 0.01).at_least(0.001);
    let drag_value_id = ui.next_auto_id();
    let is_dragging_since_last_frame = ui.ctx().is_being_dragged(drag_value_id);
    let aim_rad = ui.input(|i| i.aim_radius());
    let mut drag_response = ui.add(
        egui::DragValue::new(abs_value)
            .range(0.0..=f32::INFINITY)
            .speed(if is_dragging_since_last_frame {0.0} else {speed}).max_decimals((aim_rad / speed).log10().ceil() as usize),
    );

    if drag_response.drag_started() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CursorGrab(egui::CursorGrab::Locked));
        if let Some(press_origin) = ui.input(|i| i.pointer.press_origin()) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CursorPosition(press_origin));
        }
    }

    if is_dragging_since_last_frame && drag_response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        let motion = ui.input(|i| i.pointer.motion()).unwrap_or_else(||drag_response.drag_delta());
        let is_slow = ui.input(|i| i.modifiers.shift_only());
        let drag_speed = if is_slow { speed / 10.0 } else { speed };
        let delta_value = (motion.x - motion.y) * drag_speed; // right/up increases, as in DragValue
        if delta_value != 0.0 {
            *abs_value = (*abs_value + delta_value).at_least(0.0);
            drag_response.mark_changed();
        }
    }
    if drag_response.drag_stopped() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CursorGrab(egui::CursorGrab::None));
    }
    drag_response
}

fn edit_radius_unit_ui(value: &mut Radius, abs_value: &mut f32, ui: &mut egui::Ui) -> InnerResponse<Option<Response>> {
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
            *abs_value = 0.1;
        } else {
            *abs_value = 2.5;
        }
    }

    if is_scene_units {
        *value = Radius::new_scene_units(*abs_value);
    } else {
        *value = Radius::new_ui_points(*abs_value);
    }
    combobox_response
}

pub fn edit_radius_ui(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Radius>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut abs_value = value.0.abs();

        let drag_response = edit_radius_magnitude_ui(&mut abs_value, ui);
        let combobox_response = edit_radius_unit_ui(value, &mut abs_value, ui);

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
