//! Rerun Space View utilities
//!
//! Types & utilities for defining Space View classes and communicating with the Viewport.

pub mod controls;

mod heuristics;
mod query;
mod results_ext;
mod screenshot;
mod view_property_ui;

pub use heuristics::suggest_space_view_for_each_entity;
pub use query::{
    latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data, DataResultQuery,
};
pub use results_ext::{
    HybridLatestAtResults, HybridResults, HybridResultsChunkIter, RangeResultsExt,
};
pub use screenshot::ScreenshotMode;
pub use view_property_ui::view_property_ui;

pub mod external {
    pub use re_entity_db::external::*;
}

// -----------

/// Utility for implementing [`re_viewer_context::VisualizerAdditionalApplicabilityFilter`] using on the properties of a concrete component.
#[inline]
pub fn diff_component_filter<T: re_types_core::Component>(
    event: &re_chunk_store::ChunkStoreEvent,
    filter: impl Fn(&T) -> bool,
) -> bool {
    let filter = &filter;
    event
        .diff
        .chunk
        .components()
        .get(&T::name())
        .map_or(false, |list_array| {
            list_array
                .iter()
                .filter_map(|array| array.and_then(|array| T::from_arrow(&*array).ok()))
                .any(|instances| instances.iter().any(filter))
        })
}
