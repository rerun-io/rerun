//! This crate implements various component editors.
//!
//! The only entry point is [`create_component_ui_registry`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod color;
mod datatype_uis;
mod entity_path;
mod geo_line_string;
mod image_format;
mod lat_lon;
mod line_strip;
mod map_provider;
mod marker_shape;
mod pinhole;
mod plane3d;
mod radius;
mod resolution;
mod response_utils;
mod text_log_columns;
mod time_range;
mod timeline_columns;
pub mod transform_frame_id;
mod transforms;
mod variant_uis;
mod video_timestamp;
mod view_coordinates;
mod visible_dnd;
mod visual_bounds2d;
mod zoom_level;

use datatype_uis::{
    edit_bool, edit_f32_min_to_max_float, edit_f32_zero_to_max, edit_f32_zero_to_one,
    edit_f64_min_to_max_float, edit_f64_zero_to_max, edit_multiline_string, edit_or_view_vec2d,
    edit_or_view_vec3d, edit_singleline_string, edit_u64_range, edit_ui_points, edit_view_enum,
    edit_view_enum_with_variant_available, edit_view_range1d, view_timestamp, view_uuid,
    view_view_id,
};
use re_sdk_types::blueprint::components::{
    AngularSpeed, BackgroundKind, Corner2D, Enabled, Eye3DKind, ForceDistance, ForceIterations,
    ForceStrength, GridSpacing, LinkAxis, LockRangeDuringZoom, MapProvider, NearClipPlane,
    RootContainer, ViewFit, ViewMaximized,
};
use re_sdk_types::components::{
    AggregationPolicy, AlbedoFactor, AxisLength, Color, DepthMeter, DrawOrder, FillMode, FillRatio,
    GammaCorrection, GraphType, ImagePlaneDistance, LinearSpeed, MagnificationFilter, MarkerSize,
    Name, Opacity, Position2D, Position3D, Range1D, Scale3D, SeriesVisible, ShowLabels,
    StrokeWidth, Text, Timestamp, TransformRelation, Translation3D, ValueRange, Vector3D,
    VideoCodec, Visible,
};
use re_viewer_context::gpu_bridge::colormap_edit_or_view_ui;

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

// ---

pub const REDAP_URI_BUTTON_VARIANT: &str = "redap_uri";

pub const REDAP_ENTRY_KIND_VARIANT: &str = "redap_entry_kind";

pub const REDAP_THUMBNAIL_VARIANT: &str = "redap_thumbnail";

// ----

/// Crates a component ui registry and registers all editors of this crate to it.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn create_component_ui_registry() -> re_viewer_context::ComponentUiRegistry {
    re_tracing::profile_function!();

    let mut registry = re_viewer_context::ComponentUiRegistry::new();

    // Color components:
    registry.add_singleline_edit_or_view::<Color>(color::edit_rgba32);
    registry.add_singleline_edit_or_view::<AlbedoFactor>(color::edit_rgba32);

    // 0-inf float components:
    registry.add_singleline_edit_or_view::<AxisLength>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<DepthMeter>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<FillRatio>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<ForceDistance>(edit_f64_zero_to_max);
    registry.add_singleline_edit_or_view::<GammaCorrection>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<GridSpacing>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<ImagePlaneDistance>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<LinearSpeed>(edit_f64_zero_to_max);
    registry.add_singleline_edit_or_view::<AngularSpeed>(edit_f64_min_to_max_float);
    registry.add_singleline_edit_or_view::<MarkerSize>(edit_ui_points);
    registry.add_singleline_edit_or_view::<NearClipPlane>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<StrokeWidth>(edit_ui_points);

    // float min-max components:
    registry.add_singleline_edit_or_view::<DrawOrder>(edit_f32_min_to_max_float);
    registry.add_singleline_edit_or_view::<ForceStrength>(edit_f64_min_to_max_float);

    // float 0-1 components:
    registry.add_singleline_edit_or_view::<Opacity>(edit_f32_zero_to_one);

    // integer range components:
    registry.add_singleline_edit_or_view::<ForceIterations>(|ctx, ui, value| {
        edit_u64_range(ctx, ui, value, 1..=5)
    });

    // Bool components:
    registry.add_singleline_edit_or_view::<Enabled>(edit_bool);
    registry.add_singleline_edit_or_view::<LockRangeDuringZoom>(edit_bool);
    registry.add_singleline_edit_or_view::<ShowLabels>(edit_bool);
    registry.add_singleline_edit_or_view::<Visible>(edit_bool);
    registry.add_singleline_edit_or_view::<SeriesVisible>(edit_bool);

    // Date components:
    registry.add_singleline_edit_or_view::<Timestamp>(view_timestamp);

    // Text components:
    registry.add_singleline_edit_or_view::<Text>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Text>(edit_multiline_string);
    registry.add_singleline_edit_or_view::<Name>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Name>(edit_multiline_string);

    // Enums:
    // TODO(#6974): Enums editors trivial and always the same, provide them automatically!
    registry.add_singleline_edit_or_view::<AggregationPolicy>(edit_view_enum);
    registry.add_singleline_edit_or_view::<BackgroundKind>(edit_view_enum);
    registry.add_singleline_edit_or_view::<Corner2D>(edit_view_enum);
    registry.add_singleline_edit_or_view::<Eye3DKind>(edit_view_enum);
    registry.add_singleline_edit_or_view::<FillMode>(edit_view_enum);
    registry.add_singleline_edit_or_view::<GraphType>(edit_view_enum);
    registry.add_singleline_edit_or_view::<LinkAxis>(edit_view_enum);
    registry.add_singleline_edit_or_view::<MapProvider>(
        edit_view_enum_with_variant_available::<
            MapProvider,
            crate::map_provider::MapProviderVariantAvailable,
        >,
    );
    registry.add_singleline_edit_or_view::<MagnificationFilter>(edit_view_enum);
    registry.add_singleline_edit_or_view::<TransformRelation>(edit_view_enum);
    registry.add_singleline_edit_or_view::<VideoCodec>(|ctx, ui, value| {
        // Hack to make this field never editable.
        // Editing the codec rarely makes sense and isn't supported by the visualizer.
        // (to change this we'd have to do a blueprint query, but `VideoStreamCache` needs more context for that
        // and the result is almost certainly just decoding failure)
        edit_view_enum(
            ctx,
            ui,
            &mut re_viewer_context::MaybeMutRef::Ref(value.as_ref()),
        )
    });
    registry.add_singleline_edit_or_view::<ViewFit>(edit_view_enum);

    // Vec2 components:
    registry.add_singleline_edit_or_view::<Position2D>(edit_or_view_vec2d);

    // Vec3 components:
    registry.add_singleline_edit_or_view::<Translation3D>(edit_or_view_vec3d);
    registry.add_singleline_edit_or_view::<Scale3D>(edit_or_view_vec3d);
    registry.add_singleline_edit_or_view::<Position3D>(edit_or_view_vec3d);
    registry.add_singleline_edit_or_view::<Vector3D>(edit_or_view_vec3d);

    // Components that refer to views:
    registry.add_singleline_edit_or_view::<ViewMaximized>(view_view_id);

    registry.add_singleline_edit_or_view::<RootContainer>(view_uuid);

    // Range1D components:
    registry.add_singleline_edit_or_view::<Range1D>(edit_view_range1d);
    registry.add_singleline_edit_or_view::<ValueRange>(edit_view_range1d);

    // --------------------------------------------------------------------------------
    // All other special components:
    // --------------------------------------------------------------------------------

    registry.add_multiline_edit_or_view(time_range::time_range_multiline_edit_or_view_ui);
    registry.add_singleline_edit_or_view(time_range::time_range_singleline_view_ui);

    // `Colormap` _is_ an enum, but its custom editor is far better.
    registry.add_singleline_edit_or_view(colormap_edit_or_view_ui);

    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_edit_or_view(visual_bounds2d::singleline_edit_visual_bounds2d);

    registry.add_multiline_edit_or_view(transforms::multiline_view_transform_mat3x3);
    registry.add_singleline_edit_or_view(transforms::singleline_view_transform_mat3x3);

    registry.add_singleline_edit_or_view(image_format::edit_or_view_image_format);
    registry.add_singleline_edit_or_view(resolution::edit_or_view_resolution);
    registry.add_singleline_edit_or_view(view_coordinates::edit_or_view_view_coordinates);

    registry.add_singleline_edit_or_view(radius::edit_radius_ui);
    registry.add_singleline_edit_or_view(marker_shape::edit_marker_shape_ui);

    registry.add_singleline_edit_or_view(pinhole::singleline_view_pinhole);
    registry.add_multiline_edit_or_view(pinhole::multiline_view_pinhole);

    line_strip::register_linestrip_component_ui(&mut registry);
    geo_line_string::register_geo_line_string_component_ui(&mut registry);

    registry.add_singleline_edit_or_view(entity_path::edit_or_view_entity_path);

    registry.add_singleline_edit_or_view(video_timestamp::edit_or_view_timestamp);

    registry.add_singleline_edit_or_view(lat_lon::singleline_view_lat_lon);

    registry.add_singleline_edit_or_view(zoom_level::edit_zoom_level);

    registry.add_singleline_edit_or_view(plane3d::edit_or_view_plane3d);
    registry.add_multiline_edit_or_view(plane3d::multiline_edit_or_view_plane3d);

    registry.add_singleline_array_edit_or_view(timeline_columns::edit_or_view_columns_singleline);
    registry.add_multiline_array_edit_or_view(timeline_columns::edit_or_view_columns_multiline);

    registry.add_singleline_array_edit_or_view(text_log_columns::edit_or_view_columns_singleline);
    registry.add_multiline_array_edit_or_view(text_log_columns::edit_or_view_columns_multiline);

    registry.add_singleline_edit_or_view(transform_frame_id::edit_or_view_transform_frame_id);

    // --------------------------------------------------------------------------------
    // All variant UIs:
    // --------------------------------------------------------------------------------

    registry.add_variant_ui(REDAP_URI_BUTTON_VARIANT, variant_uis::redap_uri_button);
    registry.add_variant_ui(REDAP_ENTRY_KIND_VARIANT, variant_uis::redap_entry_kind);
    registry.add_variant_ui(REDAP_THUMBNAIL_VARIANT, variant_uis::redap_thumbnail);

    registry
}
