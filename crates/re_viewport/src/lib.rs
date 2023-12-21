//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

pub const VIEWPORT_PATH: &str = "viewport";

mod auto_layout;
mod container;
mod space_info;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod space_view_highlights;
mod system_execution;
mod viewport;
mod viewport_blueprint;
mod viewport_blueprint_ui;

/// Auto-generated blueprint-related types.
///
/// They all implement the [`re_types_core::Component`] trait.
///
/// Unstable. Used for the ongoing blueprint experimentations.
pub mod blueprint;

// Transitive re-imports of blueprint dependencies.
use re_types::datatypes;

pub use space_info::SpaceInfoCollection;
pub use space_view::SpaceViewBlueprint;
pub use space_view_heuristics::identify_entities_per_system_per_class;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;

pub mod external {
    pub use re_space_view;
}

/// Utility for querying a pinhole archetype instance.
///
/// TODO(andreas): It should be possible to convert `re_query::ArchetypeView` to its corresponding Archetype for situations like this.
/// TODO(andreas): This is duplicated into `re_space_view_spatial`
fn query_pinhole(
    store: &re_arrow_store::DataStore,
    query: &re_arrow_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    store
        .query_latest_component(entity_path, query)
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
