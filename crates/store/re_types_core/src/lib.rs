//! The core types and traits that power Rerun's data model.
//!
//! The [`Archetype`] trait is the core of this crate and is a good starting point to get familiar
//! with the code.
//! An archetype is a logical collection of batches of [`Component`]s that play well with each other.
//!
//! Rerun (and the underlying Arrow data framework) is designed to work with large arrays of
//! [`Component`]s, as opposed to single instances.
//! When multiple instances of a [`Component`] are put together in an array, they yield a
//! [`ComponentBatch`]: the atomic unit of (de)serialization.
//!
//! Internally, [`Component`]s are implemented using many different [`Loggable`]s.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

// ---

/// Number of decimals shown for all float display methods.
pub const DEFAULT_DISPLAY_DECIMALS: usize = 3;

mod archetype;
mod arrow_buffer;
pub mod arrow_helpers;
mod arrow_string;
pub mod arrow_zip_validity;
mod as_components;
mod component_descriptor;
mod loggable;
mod loggable_batch;
pub mod reflection;
mod result;
mod tuid;
mod view;

pub use self::{
    archetype::{
        Archetype, ArchetypeFieldName, ArchetypeName, ArchetypeReflectionMarker,
        GenericIndicatorComponent, NamedIndicatorComponent,
    },
    arrow_buffer::ArrowBuffer,
    arrow_string::ArrowString,
    as_components::AsComponents,
    component_descriptor::ComponentDescriptor,
    loggable::{
        Component, ComponentName, ComponentNameSet, DatatypeName, Loggable,
        UnorderedComponentNameSet,
    },
    loggable_batch::{
        ComponentBatch, ComponentBatchCow, ComponentBatchCowWithDescriptor, LoggableBatch,
    },
    result::{
        DeserializationError, DeserializationResult, ResultExt, SerializationError,
        SerializationResult, _Backtrace,
    },
    view::{View, ViewClassIdentifier},
};

/// Fundamental [`Archetype`]s that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_types`.
pub mod archetypes;

/// Fundamental [`Component`]s that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_types`.
pub mod components;

/// Fundamental datatypes that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_types`.
pub mod datatypes;

// ---

#[path = "macros.rs"]
mod _macros; // just for the side-effect of exporting the macros

pub mod macros {
    pub use super::impl_into_cow;
}

pub mod external {
    pub use anyhow;
    pub use arrow;
    pub use arrow2;
    pub use re_tuid;
}

/// Useful macro for statically asserting that a `struct` contains some specific fields.
///
/// In particular, this is useful to statcially check that an archetype
/// has a specific component.
///
///  ```
/// # #[macro_use] extern crate re_types_core;
/// struct Data {
///     x: f32,
///     y: String,
///     z: u32,
/// }
///
/// static_assert_struct_has_fields!(Data, x: f32, y: String);
/// ```
///
/// This will fail to compile because the type is wrong:
///
/// ```compile_fail
/// # #[macro_use] extern crate re_types_core;
/// struct Data {
///     x: f32,
/// }
///
/// static_assert_struct_has_fields!(Data, x: u32);
/// ```
///
/// This will fail to compile because the field is missing:
///
/// ```compile_fail
/// # #[macro_use] extern crate re_types_core;
/// struct Data {
///     x: f32,
/// }
///
/// static_assert_struct_has_fields!(Data, nosuch: f32);
/// ```
///
#[macro_export]
macro_rules! static_assert_struct_has_fields {
    ($strct:ty, $($field:ident: $field_typ:ty),+ $(,)?) => {
        const _: fn(&$strct) = |s: &$strct| {
            $(let _: &$field_typ = &s.$field;)+
        };
    }
}
