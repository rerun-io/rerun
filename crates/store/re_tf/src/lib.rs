//! Rerun spatial transform processing

mod component_type_info;
mod transform_forest;
mod transform_frame_id_hash;
mod transform_resolution_cache;

pub use transform_forest::{PinholeTreeRoot, TransformForest, TransformFromToError, TransformInfo};
pub use transform_frame_id_hash::TransformFrameIdHash;
pub use transform_resolution_cache::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformResolutionCache, query_view_coordinates, query_view_coordinates_at_closest_ancestor,
};

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_types::components::ViewCoordinates {
    re_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ
}
