//! Rerun spatial transform processing

mod transform_cache;
mod transform_tree;

pub use transform_cache::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformCacheStoreSubscriber, query_view_coordinates,
    query_view_coordinates_at_closest_ancestor,
};
pub use transform_tree::{TransformInfo, TransformTree, TwoDInThreeDTransformInfo};

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_types::components::ViewCoordinates {
    re_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ
}
