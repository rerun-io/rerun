use re_types::datatypes;
use re_viewer_context::MaybeMutRef;

use super::float_drag::edit_f32_float_raw;

pub fn edit_or_view_vec3d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::Vec3D>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, datatypes::Vec3D> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_or_view_vec3d_raw_immutable(ui, &mut value)
}

// TODO(#6743): Since overrides are not yet taken into account, editing this value has no effect.
//MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value[i]),
fn edit_or_view_vec3d_raw_immutable(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec3D>,
) -> egui::Response {
    edit_or_view_vector_component_immutable(ui, value, 0)
        | edit_or_view_vector_component_immutable(ui, value, 1)
        | edit_or_view_vector_component_immutable(ui, value, 2)
}

fn edit_or_view_vector_component_immutable(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec3D>,
    i: usize,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, f32> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(&value[i]),

        MaybeMutRef::MutRef(value) => MaybeMutRef::Ref(&value[i]),
    };
    edit_f32_float_raw(ui, &mut value, f32::MIN..=f32::MAX)
}

pub fn edit_or_view_vec3d_raw(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec3D>,
) -> egui::Response {
    edit_or_view_vector_component(ui, value, 0)
        | edit_or_view_vector_component(ui, value, 1)
        | edit_or_view_vector_component(ui, value, 2)
}

fn edit_or_view_vector_component(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Vec3D>,
    i: usize,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, f32> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(&value[i]),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value[i]),
    };
    edit_f32_float_raw(ui, &mut value, f32::MIN..=f32::MAX)
}
