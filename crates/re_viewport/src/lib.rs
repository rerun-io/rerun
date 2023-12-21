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

use nohash_hasher::{IntMap, IntSet};
pub use space_info::SpaceInfoCollection;
pub use space_view::SpaceViewBlueprint;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;

pub mod external {
    pub use re_space_view;
}

use re_data_store::StoreDb;
use re_log_types::{EntityPath, EntityPathHash};
use re_types::datatypes;

use re_viewer_context::{
    ActiveEntitiesPerVisualizer, ApplicableEntitiesPerVisualizer, DynSpaceViewClass,
    ViewSystemIdentifier, VisualizableEntities, VisualizableEntitiesPerVisualizer,
};

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

// TODO(andreas): Untangle this:
// * Heuristics need to be applied as part of the query
// * Visibility set is needed on several places, therefore store as part of the space view state (?)
pub fn determine_heuristically_active_entities_per_system(
    applicable_entities_per_visualizer: &ApplicableEntitiesPerVisualizer,
    indicator_matching_entities_per_visualizer: &IntMap<
        ViewSystemIdentifier,
        IntSet<EntityPathHash>,
    >,
    store_db: &StoreDb,
    visualizers: &re_viewer_context::ViewPartCollection,
    class: &dyn DynSpaceViewClass,
    space_origin: &EntityPath,
) -> ActiveEntitiesPerVisualizer {
    re_tracing::profile_function!();

    let filter_ctx = class.visualizable_filter_context(space_origin, store_db);

    let visualizable_entities = VisualizableEntitiesPerVisualizer(
        visualizers
            .iter_with_identifiers()
            .map(|(visualizer_identifier, visualizer_system)| {
                let entities = if let Some(applicable_entities) =
                    applicable_entities_per_visualizer.get(&visualizer_identifier)
                {
                    visualizer_system.filter_visualizable_entities(
                        applicable_entities,
                        store_db.store(),
                        &filter_ctx,
                    )
                } else {
                    VisualizableEntities::default()
                };

                (visualizer_identifier, entities)
            })
            .collect(),
    );

    class.filter_heuristic_entities_per_visualizer(
        indicator_matching_entities_per_visualizer,
        space_origin,
        &visualizable_entities,
    )
}
