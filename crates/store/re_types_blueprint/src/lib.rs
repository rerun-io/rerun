//! The core types and traits that power Rerun's Blueprint sub-system.

/// Auto-generated blueprint-related types.
///
/// They all implement the [`re_types_core::Component`] trait.
///
/// Unstable. Used for the ongoing blueprint experimentation.
pub mod blueprint;

// TODO(andreas): Workaround for referencing non-blueprint components from blueprint archetypes.
pub(crate) use re_types::datatypes;
pub(crate) mod components {
    pub use re_types::components::Name;
}
