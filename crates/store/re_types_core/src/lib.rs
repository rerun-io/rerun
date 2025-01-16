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
        SerializedComponentBatch,
    },
    result::{
        DeserializationError, DeserializationResult, ResultExt, SerializationError,
        SerializationResult, _Backtrace,
    },
    tuid::tuid_arrow_fields,
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
/// For asserting that an archetype has a specific component use `re_log_types::debug_assert_archetype_has_components`
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

// ---

/// Internal serialization helper for code-generated archetypes.
///
/// # Fallibility
///
/// There are very few ways in which serialization can fail, all of which are very rare to hit
/// in practice.
/// One such example is trying to serialize data with more than 2^31 elements into a `ListArray`.
///
/// For that reason, this method favors a nice user experience over error handling: errors will
/// merely be logged, not returned (except in debug builds, where all errors panic).
#[doc(hidden)] // public so we can access it from re_types too
#[allow(clippy::unnecessary_wraps)] // clippy gets confused in debug builds
pub fn try_serialize_field<C: crate::Component>(
    descriptor: ComponentDescriptor,
    instances: impl IntoIterator<Item = impl Into<C>>,
) -> Option<SerializedComponentBatch> {
    let res = C::to_arrow(
        instances
            .into_iter()
            .map(|v| std::borrow::Cow::Owned(v.into())),
    );

    match res {
        Ok(array) => Some(SerializedComponentBatch::new(array, descriptor)),

        #[cfg(debug_assertions)]
        Err(err) => {
            panic!(
                "failed to serialize data for {descriptor}: {}",
                re_error::format_ref(&err)
            )
        }

        #[cfg(not(debug_assertions))]
        Err(err) => {
            re_log::error!(
                %descriptor,
                "failed to serialize data: {}",
                re_error::format_ref(&err)
            );
            None
        }
    }
}
