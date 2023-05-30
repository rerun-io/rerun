//! Rerun Space View class definition
//!
//! Defines a framework & utilities for defining classes of space views in the Rerun viewer.
//! Does not implement any concrete space view.

mod space_view_class;
mod space_view_class_impl;
mod space_view_class_registry;

pub use space_view_class::{
    ArchetypeDefinition, Scene, SceneElement, SpaceViewClass, SpaceViewClassName, SpaceViewState,
};
pub use space_view_class_impl::{EmptySpaceViewState, SpaceViewClassImpl};
pub use space_view_class_registry::{SpaceViewClassRegistry, SpaceViewTypeRegistryError};
