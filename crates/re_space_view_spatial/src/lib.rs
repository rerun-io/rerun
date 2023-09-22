//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

mod contexts;
mod eye;
mod heuristics;
mod instance_hash_conversions;
mod mesh_cache;
mod mesh_loader;
mod parts;
mod picking;
mod space_camera_3d;
mod space_view_2d;
mod space_view_3d;
mod ui;
mod ui_2d;
mod ui_3d;

use re_types::components::ViewCoordinates;
pub use space_view_2d::SpatialSpaceView2D;
pub use space_view_3d::SpatialSpaceView3D;

// ---

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}

/// Utility for querying a pinhole archetype instance.
///
/// TODO(andreas): It should be possible to convert [`re_query::ArchetypeView`] to its corresponding Archetype for situations like this.
/// TODO(andreas): This is duplicated into `re_viewport`
fn query_pinhole(
    store: &re_arrow_store::DataStore,
    query: &re_arrow_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    store
        .query_latest_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(|image_from_camera| re_types::archetypes::Pinhole {
            image_from_camera: image_from_camera.value,
            resolution: store
                .query_latest_component(entity_path, query)
                .map(|c| c.value),
            camera_xyz: store
                .query_latest_component(entity_path, query)
                .map(|c| c.value),
        })
}
