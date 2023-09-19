//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

mod auto_layout;
mod space_info;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod space_view_highlights;
mod viewport;
mod viewport_blueprint;
mod viewport_blueprint_ui;

pub mod blueprint_components;

pub use space_info::SpaceInfoCollection;
pub use space_view::SpaceViewBlueprint;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;

pub mod external {
    pub use re_space_view;
}

/// Utility for querying a pinhole archetype instance.
///
/// TODO(andreas): It should be possible to convert [`re_query::ArchetypeView`] to its corresponding Archetype for situations like this.
/// TODO(andreas): This is duplicated into `re_space_view_spatial`
fn query_pinhole(
    store: &re_arrow_store::DataStore,
    query: &re_arrow_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    store
        .query_latest_component(entity_path, query)
        .map(|image_from_cam| re_types::archetypes::Pinhole {
            image_from_cam: image_from_cam.value,
            resolution: store
                .query_latest_component(entity_path, query)
                .map(|c| c.value),
        })
}
