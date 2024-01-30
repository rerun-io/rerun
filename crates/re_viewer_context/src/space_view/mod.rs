//! Rerun Space View class definition
//!
//! Defines a framework & utilities for defining classes of space views in the Rerun viewer.
//! Does not implement any concrete space view.

// TODO(andreas): Can we move some of these to the `re_space_view` crate?
mod dyn_space_view_class;
mod highlights;
mod named_system;
mod space_view_class;
mod space_view_class_placeholder;
mod space_view_class_registry;
mod spawn_heuristics;
mod system_execution_output;
mod view_context_system;
mod view_query;
mod visualizer_entity_subscriber;
mod visualizer_system;

pub use dyn_space_view_class::{
    DynSpaceViewClass, SpaceViewClassIdentifier, SpaceViewClassLayoutPriority, SpaceViewState,
    VisualizableFilterContext,
};
pub use highlights::{SpaceViewEntityHighlight, SpaceViewHighlights, SpaceViewOutlineMasks};
pub use named_system::{IdentifiedViewSystem, PerSystemEntities, ViewSystemIdentifier};
pub use space_view_class::SpaceViewClass;
pub use space_view_class_registry::{
    SpaceViewClassRegistry, SpaceViewClassRegistryError, SpaceViewSystemRegistrator,
};
pub use spawn_heuristics::{RecommendedSpaceView, SpaceViewSpawnHeuristics};
pub use system_execution_output::SystemExecutionOutput;
pub use view_context_system::{ViewContextCollection, ViewContextSystem};
pub use view_query::{DataResult, PerSystemDataResults, PropertyOverrides, ViewQuery};
pub use visualizer_entity_subscriber::VisualizerAdditionalApplicabilityFilter;
pub use visualizer_system::{VisualizerCollection, VisualizerQueryInfo, VisualizerSystem};

// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewSystemExecutionError {
    #[error("View context system {0} not found")]
    ContextSystemNotFound(&'static str),

    #[error("View part system {0} not found")]
    VisualizerSystemNotFound(&'static str),

    #[error(transparent)]
    QueryError(#[from] re_query::QueryError),

    #[error(transparent)]
    DeserializationError(#[from] re_types::DeserializationError),
}
