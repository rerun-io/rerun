//! Rerun Time Panel
//!
//! Defines a framework & utilities for space views in the Rerun viewer.
//! Does not implement any concrete space view.

mod space_view_type;
mod space_view_type_impl;
mod space_view_type_registry;

pub use space_view_type::{
    ArchetypeDefinition, Scene, SceneElement, SpaceViewState, SpaceViewType, SpaceViewTypeName,
};
pub use space_view_type_impl::{EmptySpaceViewState, SpaceViewTypeImpl};
pub use space_view_type_registry::{SpaceViewTypeRegistry, SpaceViewTypeRegistryError};
