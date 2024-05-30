//! Rerun Space View utilities
//!
//! Types & utilities for defining Space View classes and communicating with the Viewport.

pub mod controls;

mod heuristics;
mod screenshot;
mod view_property_ui;

pub use heuristics::suggest_space_view_for_each_entity;
pub use screenshot::ScreenshotMode;
pub use view_property_ui::view_property_ui;

pub mod external {
    pub use re_entity_db::external::*;
}

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
