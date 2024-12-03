use re_types::datatypes;
use re_viewer_context::MaybeMutRef;

use crate::datatype_uis::{edit_f32_float_raw, edit_or_view_vec3d_raw};

pub fn edit_or_view_plane3d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::Plane3D>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, datatypes::Plane3D> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_or_view_plane3d_impl(ui, &mut value)
}

fn edit_or_view_plane3d_impl(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, datatypes::Plane3D>,
) -> egui::Response {
    let mut normal = value.normal();
    let mut distance = value.distance();
    let (mut maybe_mut_normal, mut maybe_mutdistance) = match value {
        MaybeMutRef::Ref(value) => (MaybeMutRef::Ref(&normal), MaybeMutRef::Ref(&distance)),
        MaybeMutRef::MutRef(value) => (
            MaybeMutRef::MutRef(&mut normal),
            MaybeMutRef::MutRef(&mut distance),
        ),
    };

    ui.label("n");
    let normal_response = edit_or_view_vec3d_raw(ui, &mut maybe_mut_normal);
    ui.label("d");
    let distance_response = edit_f32_float_raw(ui, &mut maybe_mutdistance, f32::MIN..=f32::MAX);

    if let MaybeMutRef::MutRef(value) = value {
        **value = datatypes::Plane3D::new(normal, distance);
    }

    normal_response.union(distance_response)
}
