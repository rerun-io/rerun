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

// ---

/// Number of decimals shown for all float display methods.
pub const DEFAULT_DISPLAY_DECIMALS: usize = 3;

mod archetype;
pub mod arrow_helpers;
mod arrow_string;
pub mod arrow_zip_validity;
mod as_components;
mod chunk_id;
mod component_batch;
mod component_descriptor;
mod dynamic_archetype;
mod loggable;
pub mod reflection;
mod result;
mod row_id;
mod timeline_name;
mod tuid;
mod view;
mod wrapper_component;

pub use self::archetype::{
    Archetype, ArchetypeName, ArchetypeReflectionMarker, ComponentIdentifier,
};
pub use self::arrow_string::ArrowString;
pub use self::as_components::AsComponents;
pub use self::chunk_id::ChunkId;
pub use self::component_batch::{
    ComponentBatch, SerializedComponentBatch, SerializedComponentColumn,
};
pub use self::component_descriptor::{
    ComponentDescriptor, FIELD_METADATA_KEY_ARCHETYPE, FIELD_METADATA_KEY_COMPONENT,
    FIELD_METADATA_KEY_COMPONENT_TYPE,
};
pub use self::dynamic_archetype::DynamicArchetype;
pub use self::loggable::{
    Component, ComponentSet, ComponentType, DatatypeName, Loggable, UnorderedComponentSet,
};
pub use self::result::{
    _Backtrace, DeserializationError, DeserializationResult, ResultExt, SerializationError,
    SerializationResult,
};
pub use self::row_id::RowId;
pub use self::tuid::tuids_to_arrow;
pub use self::view::{View, ViewClassIdentifier};
pub use self::wrapper_component::WrapperComponent;
pub use timeline_name::TimelineName;

/// Fundamental [`Archetype`]s that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_sdk_types`.
pub mod archetypes;

/// Fundamental [`Component`]s that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_sdk_types`.
pub mod components;

/// Fundamental datatypes that are implemented in `re_types_core` directly for convenience and
/// dependency optimization.
///
/// There are also re-exported by `re_sdk_types`.
pub mod datatypes;

// ---

#[path = "macros.rs"]
mod _macros; // just for the side-effect of exporting the macros

pub mod macros {
    pub use super::impl_into_cow;
}

pub mod external {
    pub use {anyhow, arrow, re_tuid};
}

/// Useful macro for statically asserting that a `struct` contains some specific fields.
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
#[doc(hidden)] // public so we can access it from re_sdk_types too
#[expect(clippy::unnecessary_wraps)] // clippy gets confused in debug builds
pub fn try_serialize_field<L: Loggable>(
    descriptor: ComponentDescriptor,
    instances: impl IntoIterator<Item = impl Into<L>>,
) -> Option<SerializedComponentBatch> {
    let res = L::to_arrow(
        instances
            .into_iter()
            .map(|v| std::borrow::Cow::Owned(v.into())),
    );

    match res {
        Ok(array) => Some(SerializedComponentBatch::new(array, descriptor)),

        #[cfg(debug_assertions)]
        #[expect(clippy::panic)]
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
