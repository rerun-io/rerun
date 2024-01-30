//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

mod contexts;
mod eye;
mod heuristics;
mod instance_hash_conversions;
mod mesh_cache;
mod mesh_loader;
mod picking;
mod scene_bounding_boxes;
mod space_camera_3d;
mod space_view_2d;
mod space_view_3d;
mod spatial_topology;
mod ui;
mod ui_2d;
mod ui_3d;
mod visualizers;

use re_types::components::{Resolution, TensorData};
pub use space_view_2d::SpatialSpaceView2D;
pub use space_view_3d::SpatialSpaceView3D;

#[doc(hidden)] // Public for benchmarks
pub use visualizers::{LoadedPoints, Points3DComponentData};

// ---

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}

fn resolution_from_tensor(
    store: &re_data_store::DataStore,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<Resolution> {
    store
        .query_latest_component::<TensorData>(entity_path, query)
        .and_then(|tensor| {
            tensor
                .image_height_width_channels()
                .map(|hwc| Resolution([hwc[1] as f32, hwc[0] as f32].into()))
        })
}

/// Utility for querying a pinhole archetype instance.
///
/// TODO(andreas): It should be possible to convert [`re_query::ArchetypeView`] to its corresponding Archetype for situations like this.
/// TODO(andreas): This is duplicated into `re_viewport`
fn query_pinhole(
    store: &re_data_store::DataStore,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    store
        .query_latest_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(|image_from_camera| re_types::archetypes::Pinhole {
            image_from_camera: image_from_camera.value,
            resolution: store
                .query_latest_component(entity_path, query)
                .map(|c| c.value)
                .or_else(|| resolution_from_tensor(store, query, entity_path)),
            camera_xyz: store
                .query_latest_component(entity_path, query)
                .map(|c| c.value),
        })
}
