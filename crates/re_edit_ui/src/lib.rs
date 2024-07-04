//! This crate implements various component editors.
//!
//! The only entry point is [`register_editors`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

mod color;
mod datatype_editors;
mod marker_shape;
mod material;
mod radius;
mod range1d;
mod response_utils;
mod visual_bounds2d;

use datatype_editors::{
    display_name_ui, display_text_ui, edit_bool, edit_bool_raw, edit_enum,
    edit_f32_min_to_max_float_raw, edit_f32_zero_to_max, edit_f32_zero_to_max_float_raw,
    edit_f32_zero_to_one, edit_singleline_string,
};
use re_types::{
    blueprint::components::{BackgroundKind, Corner2D, LockRangeDuringZoom, ViewFit, Visible},
    components::{
        AggregationPolicy, AxisLength, Colormap, DepthMeter, DrawOrder, FillRatio, GammaCorrection,
        ImagePlaneDistance, MagnificationFilter, MarkerSize, Name, Opacity, StrokeWidth, Text,
    },
    Loggable as _,
};

// ----

/// Registers all editors of this crate in the component UI registry.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_singleline_edit_or_view(color::edit_color_ui);

    registry.add_singleline_edit_or_view(radius::edit_radius_ui);

    registry.add_singleline_editor_ui(marker_shape::edit_marker_shape_ui);
    registry.add_singleline_edit_or_view(material::edit_material_ui);
    registry.add_singleline_editor_ui(range1d::edit_range1d);

    registry.add_singleline_editor_ui::<AxisLength>(edit_f32_zero_to_max);
    registry.add_singleline_editor_ui::<FillRatio>(edit_f32_zero_to_max);
    registry.add_singleline_editor_ui::<ImagePlaneDistance>(edit_f32_zero_to_max);
    registry.add_singleline_editor_ui::<GammaCorrection>(edit_f32_zero_to_max);

    registry.add_singleline_editor_ui::<DrawOrder>(edit_f32_min_to_max_float_raw);

    registry.add_singleline_editor_ui::<DepthMeter>(edit_f32_zero_to_max_float_raw);
    registry.add_singleline_editor_ui::<MarkerSize>(edit_f32_zero_to_max_float_raw);
    registry.add_singleline_editor_ui::<StrokeWidth>(edit_f32_zero_to_max_float_raw);

    registry.add_singleline_editor_ui::<Opacity>(edit_f32_zero_to_one);

    registry.add_singleline_editor_ui::<Visible>(edit_bool_raw);
    registry.add_singleline_editor_ui::<LockRangeDuringZoom>(edit_bool);

    registry.add_singleline_editor_ui::<Text>(edit_singleline_string);
    registry.add_display_ui(Text::name(), Box::new(display_text_ui));
    registry.add_singleline_editor_ui::<Name>(edit_singleline_string);
    registry.add_display_ui(Name::name(), Box::new(display_name_ui));

    registry.add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<BackgroundKind>(ui, value));
    registry.add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<Colormap>(ui, value));
    registry.add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<Corner2D>(ui, value));
    registry
        .add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<MagnificationFilter>(ui, value));
    registry.add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<AggregationPolicy>(ui, value));
    registry.add_singleline_editor_ui(|_ctx, ui, value| edit_enum::<ViewFit>(ui, value));

    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_editor_ui(visual_bounds2d::singleline_edit_visual_bounds2d);
}
