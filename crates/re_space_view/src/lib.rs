//! Rerun Space View utilities
//!
//! Types & utilities for defining Space View classes and communicating with the Viewport.

pub mod controls;
mod data_query;
mod heuristics;
mod screenshot;
mod space_view;
mod space_view_contents;
mod sub_archetypes; // TODO(andreas): better name before `sub_archetype` sticks around?
mod visual_time_range;
mod visualizable;

pub use data_query::{DataQuery, EntityOverrideContext, PropertyResolver};
pub use heuristics::suggest_space_view_for_each_entity;
pub use screenshot::ScreenshotMode;
pub use space_view::{SpaceViewBlueprint, SpaceViewName};
pub use space_view_contents::SpaceViewContents;
pub use sub_archetypes::{
    entity_path_for_space_view_sub_archetype, query_space_view_sub_archetype,
    query_space_view_sub_archetype_or_default,
};
pub use visual_time_range::{
    default_time_range, query_visual_history, time_range_boundary_to_visible_history_boundary,
    visible_history_boundary_to_time_range_boundary, visible_time_range_to_time_range,
};
pub use visualizable::determine_visualizable_entities;

// -----------

use re_entity_db::external::re_data_store;

/// Utility for implementing [`re_viewer_context::VisualizerAdditionalApplicabilityFilter`] using on the properties of a concrete component.
#[inline]
pub fn diff_component_filter<T: re_types_core::Component>(
    event: &re_data_store::StoreEvent,
    filter: impl Fn(&T) -> bool,
) -> bool {
    let filter = &filter;
    event.diff.cells.iter().any(|(component_name, cell)| {
        component_name == &T::name()
            && T::from_arrow(cell.as_arrow_ref())
                .map(|components| components.iter().any(filter))
                .unwrap_or(false)
    })
}
