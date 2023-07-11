//! Rerun Space View class definition
//!
//! Defines a framework & utilities for defining classes of space views in the Rerun viewer.
//! Does not implement any concrete space view.

// TODO(andreas): Can we move some of these to the `re_space_view` crate?
mod dyn_space_view_class;
mod highlights;
mod scene;
mod space_view_class;
mod space_view_class_placeholder;
mod space_view_class_registry;
mod view_context_system;
mod view_part_system;
mod view_query;

pub use dyn_space_view_class::{
    ArchetypeDefinition, DynSpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassName,
    SpaceViewState,
};
pub use highlights::{SpaceViewEntityHighlight, SpaceViewHighlights, SpaceViewOutlineMasks};
pub use scene::{Scene, TypedScene};
pub use space_view_class::SpaceViewClass;
pub use space_view_class_registry::{
    SpaceViewClassRegistry, SpaceViewClassRegistryEntry, SpaceViewClassRegistryError,
};
pub use view_context_system::{ViewContext, ViewContextSystem};
pub use view_part_system::{ViewPartSystem, ViewPartSystemCollection};
pub use view_query::ViewQuery;
