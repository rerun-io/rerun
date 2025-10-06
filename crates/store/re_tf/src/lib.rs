//! Rerun spatial transform processing

mod component_type_info;
mod transform_resolution_cache;
mod transform_tree;

pub use transform_resolution_cache::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformResolutionCache, query_view_coordinates, query_view_coordinates_at_closest_ancestor,
};
pub use transform_tree::{
    TransformFromToError, TransformInfo, TransformTree, TwoDInThreeDTransformInfo,
};

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_types::components::ViewCoordinates {
    re_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ
}
