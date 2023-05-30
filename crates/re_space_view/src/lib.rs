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

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
