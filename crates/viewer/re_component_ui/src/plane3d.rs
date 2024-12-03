use re_types::{components, external::glam};
use re_ui::UiExt as _;
use re_viewer_context::MaybeMutRef;

use crate::{
    datatype_uis::{edit_f32_float_raw, edit_or_view_vec3d_raw},
    response_utils::response_with_changes_of_inner,
};

#[derive(PartialEq, Eq, Copy, Clone)]
enum AxisDirection {
    PosX,
    PosY,
    PosZ,
    NegX,
    NegY,
    NegZ,
}

impl AxisDirection {
    const VARIANTS: [Self; 6] = [
        Self::PosX,
        Self::PosY,
        Self::PosZ,
        Self::NegX,
        Self::NegY,
        Self::NegZ,
    ];
}

impl std::fmt::Display for AxisDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PosX => write!(f, "+X"),
            Self::PosY => write!(f, "+Y"),
            Self::PosZ => write!(f, "+Z"),
            Self::NegX => write!(f, "-X"),
            Self::NegY => write!(f, "-Y"),
            Self::NegZ => write!(f, "-Z"),
        }
    }
}

impl TryFrom<glam::Vec3> for AxisDirection {
    type Error = ();

    fn try_from(value: glam::Vec3) -> Result<Self, Self::Error> {
        match value {
            glam::Vec3::X => Ok(Self::PosX),
            glam::Vec3::Y => Ok(Self::PosY),
            glam::Vec3::Z => Ok(Self::PosZ),
            glam::Vec3::NEG_X => Ok(Self::NegX),
            glam::Vec3::NEG_Y => Ok(Self::NegY),
            glam::Vec3::NEG_Z => Ok(Self::NegZ),
            _ => Err(()),
        }
    }
}

impl From<AxisDirection> for glam::Vec3 {
    fn from(value: AxisDirection) -> Self {
        match value {
            AxisDirection::PosX => Self::X,
            AxisDirection::PosY => Self::Y,
            AxisDirection::PosZ => Self::Z,
            AxisDirection::NegX => Self::NEG_X,
            AxisDirection::NegY => Self::NEG_Y,
            AxisDirection::NegZ => Self::NEG_Z,
        }
    }
}

pub fn edit_or_view_plane3d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, components::Plane3D>,
) -> egui::Response {
    let distance = value.distance();

    ui.label("n");
    // Show simplified combobox if this is axis aligned.
    let normal_response = if let Ok(mut axis_dir) =
        AxisDirection::try_from(glam::Vec3::from(value.normal()))
    {
        response_with_changes_of_inner(
            egui::ComboBox::from_id_salt("plane_normal")
                .selected_text(format!("{axis_dir}"))
                .height(250.0)
                .show_ui(ui, |ui| {
                    let mut variants = AxisDirection::VARIANTS.iter();
                    #[allow(clippy::unwrap_used)] // We know there's more than zero variants.
                    let variant = variants.next().unwrap();

                    let mut response =
                        ui.selectable_value(&mut axis_dir, *variant, variant.to_string());
                    for variant in variants {
                        response |=
                            ui.selectable_value(&mut axis_dir, *variant, variant.to_string());
                    }

                    if let MaybeMutRef::MutRef(value) = value {
                        **value = components::Plane3D::new(glam::Vec3::from(axis_dir), distance);
                    }
                    response
                }),
        )
    } else {
        // Editing for arbitrary normals takes too much space here.
        edit_or_view_vec3d_raw(ui, &mut MaybeMutRef::Ref(&value.normal()))
    };

    ui.label("d");
    let mut maybe_mut_distance = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(&value.0 .0[3]),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.0 .0[3]),
    };
    let distance_response =
        edit_f32_float_raw(ui, &mut maybe_mut_distance, f32::MIN..=f32::MAX, "");

    normal_response | distance_response
}

pub fn multiline_edit_or_view_plane3d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, components::Plane3D>,
) -> egui::Response {
    let mut any_edit = false;

    let response_normal = ui.list_item_flat_noninteractive(
        re_ui::list_item::PropertyContent::new("Normal").value_fn(|ui, _| {
            let mut normal = value.normal();
            let mut maybe_mut_normal = match value {
                MaybeMutRef::Ref(_) => MaybeMutRef::Ref(&normal),
                MaybeMutRef::MutRef(_) => MaybeMutRef::MutRef(&mut normal),
            };

            any_edit |= edit_or_view_vec3d_raw(ui, &mut maybe_mut_normal).changed();

            if let MaybeMutRef::MutRef(value) = value {
                **value = components::Plane3D::new(normal, value.distance());
            }
        }),
    );

    let response_distance = ui.list_item_flat_noninteractive(
        re_ui::list_item::PropertyContent::new("Distance").value_fn(|ui, _| {
            let mut maybe_mut_distance = match value {
                MaybeMutRef::Ref(value) => MaybeMutRef::Ref(&value.0 .0[3]),
                MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.0 .0[3]),
            };

            any_edit |=
                edit_f32_float_raw(ui, &mut maybe_mut_distance, f32::MIN..=f32::MAX, "").changed();
        }),
    );

    let mut response = response_normal | response_distance;
    if any_edit {
        response.mark_changed();
    }
    response
}
