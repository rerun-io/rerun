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

    /// Not along any axis.
    Other,
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
            Self::Other => write!(f, "-"),
        }
    }
}

impl From<glam::Vec3> for AxisDirection {
    fn from(value: glam::Vec3) -> Self {
        match value {
            glam::Vec3::X => Self::PosX,
            glam::Vec3::Y => Self::PosY,
            glam::Vec3::Z => Self::PosZ,
            glam::Vec3::NEG_X => Self::NegX,
            glam::Vec3::NEG_Y => Self::NegY,
            glam::Vec3::NEG_Z => Self::NegZ,
            _ => Self::Other,
        }
    }
}

impl TryFrom<AxisDirection> for glam::Vec3 {
    type Error = ();

    fn try_from(value: AxisDirection) -> Result<Self, Self::Error> {
        match value {
            AxisDirection::PosX => Ok(Self::X),
            AxisDirection::PosY => Ok(Self::Y),
            AxisDirection::PosZ => Ok(Self::Z),
            AxisDirection::NegX => Ok(Self::NEG_X),
            AxisDirection::NegY => Ok(Self::NEG_Y),
            AxisDirection::NegZ => Ok(Self::NEG_Z),
            AxisDirection::Other => Err(()),
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

    let normal_response = if let Some(value_mut) = value.as_mut() {
        // Show simplified combobox if this is axis aligned.
        let mut axis_dir = AxisDirection::from(glam::Vec3::from(value_mut.normal()));
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
                    if let Ok(new_dir) = glam::Vec3::try_from(axis_dir) {
                        *value_mut = components::Plane3D::new(new_dir, distance);
                    }
                    response
                }),
        )
        .on_hover_text(format!(
            "{} {} {}",
            re_format::format_f32(value.normal().x()),
            re_format::format_f32(value.normal().y()),
            re_format::format_f32(value.normal().z()),
        ))
    } else {
        let normal = value.normal();
        edit_or_view_vec3d_raw(ui, &mut MaybeMutRef::Ref(&normal))
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

    let response_normal = ui.list_item().interactive(false).show_hierarchical(
        ui,
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

    let response_distance = ui.list_item().interactive(false).show_hierarchical(
        ui,
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
