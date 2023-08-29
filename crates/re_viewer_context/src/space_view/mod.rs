//! Rerun Space View class definition
//!
//! Defines a framework & utilities for defining classes of space views in the Rerun viewer.
//! Does not implement any concrete space view.

// TODO(andreas): Can we move some of these to the `re_space_view` crate?
mod auto_spawn_heuristic;
mod dyn_space_view_class;
mod highlights;
mod named_system;
mod space_view_class;
mod space_view_class_placeholder;
mod space_view_class_registry;
mod view_context_system;
mod view_part_system;
mod view_query;

pub use auto_spawn_heuristic::AutoSpawnHeuristic;
pub use dyn_space_view_class::{
    ArchetypeDefinition, DynSpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassName,
    SpaceViewState,
};
pub use highlights::{SpaceViewEntityHighlight, SpaceViewHighlights, SpaceViewOutlineMasks};
pub use named_system::{NamedViewSystem, PerSystemEntities, ViewSystemName};
pub use space_view_class::SpaceViewClass;
pub use space_view_class_registry::{
    SpaceViewClassRegistry, SpaceViewClassRegistryError, SpaceViewSystemRegistry,
};
pub use view_context_system::{ViewContextCollection, ViewContextSystem};
pub use view_part_system::{ViewPartCollection, ViewPartSystem};
pub use view_query::ViewQuery;

// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewSystemExecutionError {
    #[error("View context system {0} not found")]
    ContextSystemNotFound(&'static str),

    #[error("View part system {0} not found")]
    PartSystemNotFound(&'static str),
}
