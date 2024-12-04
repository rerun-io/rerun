//! This crate implements various component editors.
//!
//! The only entry point is [`create_component_ui_registry`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

mod color;
mod datatype_uis;
mod entity_path;
mod fallback_ui;
mod geo_line_string;
mod image_format;
mod lat_lon;
mod line_strip;
mod map_provider;
mod marker_shape;
mod pinhole;
mod plane3d;
mod radius;
mod recording_uri;
mod resolution;
mod response_utils;
mod timeline;
mod transforms;
mod video_timestamp;
mod view_coordinates;
mod visual_bounds2d;
mod zoom_level;

use datatype_uis::{
    display_name_ui, display_text_ui, edit_bool, edit_f32_min_to_max_float, edit_f32_zero_to_max,
    edit_f32_zero_to_one, edit_multiline_string, edit_or_view_vec3d, edit_singleline_string,
    edit_ui_points, edit_view_enum, edit_view_enum_with_variant_available, edit_view_range1d,
    view_uuid, view_view_id,
};

use re_types::{
    blueprint::components::{
        BackgroundKind, Corner2D, GridSpacing, LockRangeDuringZoom, MapProvider, ViewFit, Visible,
    },
    components::{
        AggregationPolicy, AlbedoFactor, AxisLength, Color, DepthMeter, DrawOrder, FillMode,
        FillRatio, GammaCorrection, GraphType, ImagePlaneDistance, MagnificationFilter, MarkerSize,
        Name, Opacity, Range1D, Scale3D, ShowLabels, StrokeWidth, Text, TransformRelation,
        Translation3D, ValueRange,
    },
    Component as _,
};
use re_types_blueprint::blueprint::components::{RootContainer, SpaceViewMaximized};
use re_viewer_context::gpu_bridge::colormap_edit_or_view_ui;

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

// ----

/// Crates a component ui registry and registers all editors of this crate to it.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn create_component_ui_registry() -> re_viewer_context::ComponentUiRegistry {
    re_tracing::profile_function!();

    let mut registry =
        re_viewer_context::ComponentUiRegistry::new(Box::new(&fallback_ui::fallback_component_ui));

    // Color components:
    registry.add_singleline_edit_or_view::<Color>(color::edit_rgba32);
    registry.add_singleline_edit_or_view::<AlbedoFactor>(color::edit_rgba32);

    // 0-inf float components:
    registry.add_singleline_edit_or_view::<AxisLength>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<DepthMeter>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<FillRatio>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<GammaCorrection>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<GridSpacing>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<ImagePlaneDistance>(edit_f32_zero_to_max);
    registry.add_singleline_edit_or_view::<MarkerSize>(edit_ui_points);
    registry.add_singleline_edit_or_view::<StrokeWidth>(edit_ui_points);

    // float min-max components:
    registry.add_singleline_edit_or_view::<DrawOrder>(edit_f32_min_to_max_float);

    // float 0-1 components:
    registry.add_singleline_edit_or_view::<Opacity>(edit_f32_zero_to_one);

    // Bool components:
    registry.add_singleline_edit_or_view::<Visible>(edit_bool);
    registry.add_singleline_edit_or_view::<LockRangeDuringZoom>(edit_bool);
    registry.add_singleline_edit_or_view::<ShowLabels>(edit_bool);
    registry.add_singleline_edit_or_view::<Visible>(edit_bool);

    // Text components:
    registry.add_legacy_display_ui(Text::name(), Box::new(display_text_ui)); // TODO(andreas): Why is there a display ui?
    registry.add_singleline_edit_or_view::<Text>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Text>(edit_multiline_string);
    registry.add_legacy_display_ui(Name::name(), Box::new(display_name_ui)); // TODO(andreas): Why is there a display ui?
    registry.add_singleline_edit_or_view::<Name>(edit_singleline_string);
    registry.add_multiline_edit_or_view::<Name>(edit_multiline_string);

    // Enums:
    // TODO(#6974): Enums editors trivial and always the same, provide them automatically!
    registry.add_singleline_edit_or_view::<AggregationPolicy>(edit_view_enum);
    registry.add_singleline_edit_or_view::<BackgroundKind>(edit_view_enum);
    registry.add_singleline_edit_or_view::<Corner2D>(edit_view_enum);
    registry.add_singleline_edit_or_view::<FillMode>(edit_view_enum);
    registry.add_singleline_edit_or_view::<GraphType>(edit_view_enum);
    registry.add_singleline_edit_or_view::<MapProvider>(
        edit_view_enum_with_variant_available::<
            MapProvider,
            crate::map_provider::MapProviderVariantAvailable,
        >,
    );
    registry.add_singleline_edit_or_view::<MagnificationFilter>(edit_view_enum);
    registry.add_singleline_edit_or_view::<TransformRelation>(edit_view_enum);
    registry.add_singleline_edit_or_view::<ViewFit>(edit_view_enum);

    // Vec3 components:
    registry.add_singleline_edit_or_view::<Translation3D>(edit_or_view_vec3d);
    registry.add_singleline_edit_or_view::<Scale3D>(edit_or_view_vec3d);

    // Components that refer to views:
    registry.add_singleline_edit_or_view::<SpaceViewMaximized>(view_view_id);

    registry.add_singleline_edit_or_view::<RootContainer>(view_uuid);

    // Range1D components:
    registry.add_singleline_edit_or_view::<Range1D>(edit_view_range1d);
    registry.add_singleline_edit_or_view::<ValueRange>(edit_view_range1d);

    // --------------------------------------------------------------------------------
    // All other special components:
    // --------------------------------------------------------------------------------

    // `Colormap` _is_ an enum, but its custom editor is far better.
    registry.add_singleline_edit_or_view(colormap_edit_or_view_ui);

    registry.add_singleline_edit_or_view(timeline::edit_timeline_name);

    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_edit_or_view(visual_bounds2d::singleline_edit_visual_bounds2d);

    registry.add_multiline_edit_or_view(transforms::multiline_view_transform_mat3x3);
    registry.add_singleline_edit_or_view(transforms::singleline_view_transform_mat3x3);

    registry.add_singleline_edit_or_view(image_format::edit_or_view_image_format);
    registry.add_singleline_edit_or_view(resolution::edit_or_view_resolution);
    registry.add_singleline_edit_or_view(view_coordinates::edit_or_view_view_coordinates);

    registry.add_singleline_edit_or_view(radius::edit_radius_ui);
    registry.add_singleline_edit_or_view(marker_shape::edit_marker_shape_ui);

    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_edit_or_view(visual_bounds2d::singleline_edit_visual_bounds2d);
    registry.add_multiline_edit_or_view(visual_bounds2d::multiline_edit_visual_bounds2d);
    registry.add_singleline_edit_or_view(visual_bounds2d::singleline_edit_visual_bounds2d);

    registry.add_singleline_edit_or_view(pinhole::singleline_view_pinhole);
    registry.add_multiline_edit_or_view(pinhole::multiline_view_pinhole);

    registry.add_singleline_edit_or_view(recording_uri::singleline_view_recording_uri);

    line_strip::register_linestrip_component_ui(&mut registry);
    geo_line_string::register_geo_line_string_component_ui(&mut registry);

    registry.add_singleline_edit_or_view(entity_path::edit_or_view_entity_path);

    registry.add_singleline_edit_or_view(video_timestamp::edit_or_view_timestamp);

    registry.add_singleline_edit_or_view(lat_lon::singleline_view_lat_lon);

    registry.add_singleline_edit_or_view(zoom_level::edit_zoom_level);

    registry.add_singleline_edit_or_view(plane3d::edit_or_view_plane3d);
    registry.add_multiline_edit_or_view(plane3d::multiline_edit_or_view_plane3d);

    registry
}
