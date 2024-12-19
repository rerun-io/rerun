use std::ops::RangeInclusive;

use egui::NumExt as _;
use re_types::datatypes;
use re_viewer_context::{MaybeMutRef, UiLayout};

pub fn edit_or_view_vec2d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::Vec2D>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, datatypes::Vec2D> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_or_view_vec2d_raw(ui, &mut value)
}

pub fn edit_or_view_vec3d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::Vec3D>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, datatypes::Vec3D> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_or_view_vec3d_raw(ui, &mut value)
}

fn drag<'a>(value: &'a mut f32, range: RangeInclusive<f32>, suffix: &str) -> egui::DragValue<'a> {
    let speed = (value.abs() * 0.01).at_least(0.001);
    egui::DragValue::new(value)
        .clamp_existing_to_range(false)
        .range(range)
        .speed(speed)
        .suffix(suffix)
}

// TODO(#6743): Since overrides are not yet taken into account, editing this value has no effect.
pub fn edit_or_view_vec2d_raw(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec2D>,
) -> egui::Response {
    let x = value.0[0];
    let y = value.0[1];

    if let Some(value) = value.as_mut() {
        let mut x_edit = x;
        let mut y_edit = y;

        let response_x = ui.add(drag(&mut x_edit, f32::MIN..=f32::MAX, ""));
        let response_y = ui.add(drag(&mut y_edit, f32::MIN..=f32::MAX, ""));

        let response = response_y | response_x;

        if response.changed() {
            *value = datatypes::Vec2D([x_edit, y_edit]);
        }

        response
    } else {
        UiLayout::List.data_label(
            ui,
            format!(
                "[{}, {}]",
                re_format::format_f32(x),
                re_format::format_f32(y),
            ),
        )
    }
}

// TODO(#6743): Since overrides are not yet taken into account, editing this value has no effect.
pub fn edit_or_view_vec3d_raw(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec3D>,
) -> egui::Response {
    let x = value.0[0];
    let y = value.0[1];
    let z = value.0[2];

    if let Some(value) = value.as_mut() {
        let mut x_edit = x;
        let mut y_edit = y;
        let mut z_edit = z;

        let response_x = ui.add(drag(&mut x_edit, f32::MIN..=f32::MAX, ""));
        let response_y = ui.add(drag(&mut y_edit, f32::MIN..=f32::MAX, ""));
        let response_z = ui.add(drag(&mut z_edit, f32::MIN..=f32::MAX, ""));

        let response = response_y | response_x | response_z;

        if response.changed() {
            *value = datatypes::Vec3D([x_edit, y_edit, z_edit]);
        }

        response
    } else {
        UiLayout::List.data_label(
            ui,
            format!(
                "[{}, {}, {}]",
                re_format::format_f32(x),
                re_format::format_f32(y),
                re_format::format_f32(z),
            ),
        )
    }
}
