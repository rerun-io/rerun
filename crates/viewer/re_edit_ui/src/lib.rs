//! This crate implements various component editors.
//!
//! The only entry point is [`register_editors`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

mod color;
mod datatype_editors;
mod marker_shape;
mod radius;
mod range1d;
mod response_utils;
mod transforms;
mod visual_bounds2d;

use datatype_editors::{
    display_name_ui, display_text_ui, edit_bool, edit_f32_min_to_max_float, edit_f32_zero_to_max,
    edit_f32_zero_to_one, edit_multiline_string, edit_or_view_vec3d, edit_singleline_string,
    edit_view_enum,
};
use re_types::{
    blueprint::components::{
        BackgroundKind, Corner2D, DataframeViewMode, LockRangeDuringZoom, SortKey, SortOrder,
        ViewFit, Visible,
    },
    components::{
        AggregationPolicy, AlbedoFactor, AxisLength, ChannelDatatype, Color, ColorModel, Colormap,
        DepthMeter, DrawOrder, FillMode, FillRatio, GammaCorrection, ImagePlaneDistance,
        MagnificationFilter, MarkerSize, Name, Opacity, PixelFormat, Scale3D, StrokeWidth, Text,
        TransformRelation, Translation3D,
    },
    Loggable as _,
};

// ----

/// Registers all editors of this crate in the component UI registry.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_singleline_edit_or_view::<Color>(color::edit_rgba32);

    registry.add_singleline_edit_or_view(radius::edit_radius_ui);

    registry.add_singleline_edit_or_view(marker_shape::edit_marker_shape_ui);
    registry.add_singleline_edit_or_view::<AlbedoFactor>(color::edit_rgba32);
    registry.add_singleline_edit_or_view(range1d::edit_range1d);

    registry.add_singleline_edit_or_view::<AxisLength>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<DepthMeter>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<FillRatio>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<GammaCorrection>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<ImagePlaneDistance>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<MarkerSize>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<StrokeWidth>(edit_f32_zero_to_max);

    registry.add_singleline_edit_or_view::<DrawOrder>(edit_f32_min_to_max_float);

    registry.add_singleline_edit_or_view::<Opacity>(edit_f32_zero_to_one);

    registry.add_singleline_edit_or_view::<Visible>(edit_bool);
    registry.add_singleline_edit_or_view::<LockRangeDuringZoom>(edit_bool);

    registry.add_display_ui(Text::name(), Box::new(display_text_ui));
    registry.add_singleline_edit_or_view::<Text>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Text>(edit_multiline_string);
    registry.add_display_ui(Name::name(), Box::new(display_name_ui));
    registry.add_singleline_edit_or_view::<Name>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Name>(edit_multiline_string);

    // TODO(#6974): Enums editors trivial and always the same, provide them automatically!
    registry.add_singleline_edit_or_view::<AggregationPolicy>(edit_view_enum);
    registry.add_singleline_edit_or_view::<BackgroundKind>(edit_view_enum);
    registry.add_singleline_edit_or_view::<ChannelDatatype>(edit_view_enum);
    registry.add_singleline_edit_or_view::<Colormap>(edit_view_enum);
    registry.add_singleline_edit_or_view::<ColorModel>(edit_view_enum);
    registry.add_singleline_edit_or_view::<Corner2D>(edit_view_enum);
    registry.add_singleline_edit_or_view::<DataframeViewMode>(edit_view_enum);
    registry.add_singleline_edit_or_view::<FillMode>(edit_view_enum);
    registry.add_singleline_edit_or_view::<MagnificationFilter>(edit_view_enum);
    registry.add_singleline_edit_or_view::<PixelFormat>(edit_view_enum);
    registry.add_singleline_edit_or_view::<SortKey>(edit_view_enum);
    registry.add_singleline_edit_or_view::<SortOrder>(edit_view_enum);
    registry.add_singleline_edit_or_view::<TransformRelation>(edit_view_enum);
    registry.add_singleline_edit_or_view::<ViewFit>(edit_view_enum);

    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_edit_or_view(visual_bounds2d::singleline_edit_visual_bounds2d);

    registry.add_multiline_edit_or_view(transforms::multiline_view_transform_mat3x3);
    registry.add_singleline_edit_or_view(transforms::singleline_view_transform_mat3x3);

    registry.add_singleline_edit_or_view::<Translation3D>(edit_or_view_vec3d);
    registry.add_singleline_edit_or_view::<Scale3D>(edit_or_view_vec3d);
}
