//! Rerun Space View class definition
//!
//! Defines a framework & utilities for defining classes of space views in the Rerun viewer.
//! Does not implement any concrete space view.

// TODO(andreas): Can we move some of these to the `re_view` crate?
mod highlights;
mod named_system;
mod space_view_class;
mod space_view_class_placeholder;
mod space_view_class_registry;
mod spawn_heuristics;
mod system_execution_output;
mod view_context;
mod view_context_system;
mod view_query;
mod view_states;
mod visualizer_entity_subscriber;
mod visualizer_system;

pub use highlights::{
    OptionalSpaceViewEntityHighlight, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks,
};
pub use named_system::{IdentifiedViewSystem, PerSystemEntities, ViewSystemIdentifier};
pub use space_view_class::{
    SpaceViewClass, SpaceViewClassExt, SpaceViewClassLayoutPriority, SpaceViewState,
    SpaceViewStateExt, VisualizableFilterContext,
};
pub use space_view_class_registry::{
    SpaceViewClassRegistry, SpaceViewClassRegistryError, SpaceViewSystemRegistrator,
};
pub use spawn_heuristics::{RecommendedSpaceView, SpaceViewSpawnHeuristics};
pub use system_execution_output::SystemExecutionOutput;
pub use view_context::ViewContext;
pub use view_context_system::{ViewContextCollection, ViewContextSystem};
pub use view_query::{
    DataResult, OverridePath, PerSystemDataResults, PropertyOverrides, SmallVisualizerSet,
    ViewQuery,
};
pub use view_states::ViewStates;
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
    QueryError2(#[from] re_query::QueryError),

    #[error(transparent)]
    DeserializationError(#[from] re_types::DeserializationError),

    #[error("Failed to create draw data: {0}")]
    DrawDataCreationError(Box<dyn std::error::Error>),

    #[error("Error accessing map view tiles.")]
    MapTilesError,

    #[error(transparent)]
    GpuTransferError(#[from] re_renderer::CpuWriteGpuReadError),

    #[error("Failed to downcast space view's to the {0}.")]
    StateCastError(&'static str),

    #[error("No render context error.")]
    NoRenderContextError,

    #[error(transparent)]
    ComponentFallbackError(#[from] crate::ComponentFallbackError),

    #[error(transparent)]
    ViewBuilderError(#[from] re_renderer::view_builder::ViewBuilderError),
}

// Convenience conversions for some re_renderer error types since these are so frequent.

impl From<re_renderer::renderer::LineDrawDataError> for SpaceViewSystemExecutionError {
    fn from(val: re_renderer::renderer::LineDrawDataError) -> Self {
        Self::DrawDataCreationError(Box::new(val))
    }
}

impl From<re_renderer::renderer::PointCloudDrawDataError> for SpaceViewSystemExecutionError {
    fn from(val: re_renderer::renderer::PointCloudDrawDataError) -> Self {
        Self::DrawDataCreationError(Box::new(val))
    }
}
