//! This crate implements various component editors.
//!
//! The only entry point is [`register_editors`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

mod datatype_editors;
mod marker_shape;
mod range1d;
mod response_utils;

use datatype_editors::edit_enum;
use re_types::{
    blueprint::components::{BackgroundKind, Corner2D, LockRangeDuringZoom, Visible},
    components::{Color, MarkerSize, Name, Radius, Range1D, StrokeWidth, Text},
};
use re_viewer_context::ViewerContext;

// ----

fn edit_color_ui(_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, value: &mut Color) -> egui::Response {
    let mut edit_color = (*value).into();
    let response = egui::color_picker::color_edit_button_srgba(
        ui,
        &mut edit_color,
        // TODO(#1611): No transparency supported right now.
        // Once we do we probably need to be more explicit about the component semantics.
        egui::color_picker::Alpha::Opaque,
    );
    *value = edit_color.into();
    response
}

// ----

/// Registers all editors of this crate in the component UI registry.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_editor(edit_color_ui);

    registry.add_editor(range1d::edit_range1d);
    registry.add_editor(marker_shape::edit_marker_shape_ui);

    registry.add_editor::<LockRangeDuringZoom>(datatype_editors::edit_bool);
    registry.add_editor::<Visible>(datatype_editors::edit_bool_raw);

    registry.add_editor::<Name>(datatype_editors::edit_singleline_string);
    registry.add_editor::<Text>(datatype_editors::edit_singleline_string);

    registry.add_editor::<MarkerSize>(datatype_editors::edit_f32_zero_to_inf);
    registry.add_editor::<Radius>(datatype_editors::edit_f32_zero_to_inf);
    registry.add_editor::<StrokeWidth>(datatype_editors::edit_f32_zero_to_inf);

    registry.add_editor(|_ctx, ui, value| edit_enum(ui, "corner2d", value, &Corner2D::ALL));
    registry
        .add_editor(|_ctx, ui, value| edit_enum(ui, "backgroundkind", value, &BackgroundKind::ALL));
}
