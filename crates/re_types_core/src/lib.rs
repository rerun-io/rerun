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
//! Internally, [`Component`]s are implemented using many different [`Datatype`]s.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

// ---

/// Describes the interface for interpreting an object as a bundle of [`Component`]s.
///
/// ## Custom bundles
///
/// While, in most cases, component bundles are code generated from our [IDL definitions],
/// it is possible to manually extend existing bundles, or even implement fully custom ones.
///
/// All [`AsComponents`] methods are optional to implement, with the exception of
/// [`AsComponents::as_component_batches`], which describes how the bundle can be interpreted
/// as a set of [`ComponentBatch`]es: arrays of components that are ready to be serialized.
///
/// Have a look at our [Custom Data Loader] example to learn more about handwritten bundles.
///
/// [IDL definitions]: https://github.com/rerun-io/rerun/tree/latest/crates/re_types/definitions/rerun
/// [Custom Data Loader]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data_loader
pub trait AsComponents {
    /// Exposes the object's contents as a set of [`ComponentBatch`]s.
    ///
    /// This is the main mechanism for easily extending builtin archetypes or even writing
    /// fully custom ones.
    /// Have a look at our [Custom Data Loader] example to learn more about extending archetypes.
    ///
    /// [Custom Data Loader]: https://github.com/rerun-io/rerun/tree/latest/examples/rust/custom_data_loader
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>>;

    // ---

    /// Serializes all non-null [`Component`]s of this bundle into Arrow arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_batches`]
    /// as-is, which is what you want in 99.9% of cases.
    #[inline]
    fn to_arrow(
        &self,
    ) -> SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>>
    {
        self.as_component_batches()
            .into_iter()
            .map(|comp_batch| {
                comp_batch
                    .as_ref()
                    .to_arrow()
                    .map(|array| (comp_batch.as_ref().arrow_field(), array))
                    .with_context(comp_batch.as_ref().name())
            })
            .collect()
    }
}

// ---

mod archetype;
mod arrow_buffer;
mod arrow_string;
mod loggable;
mod loggable_batch;
pub mod reflection;
mod result;
mod size_bytes;
mod tuid;
mod view;

pub use self::{
    archetype::{
        Archetype, ArchetypeFieldInfo, ArchetypeInfo, ArchetypeName, GenericIndicatorComponent,
        NamedIndicatorComponent,
    },
    arrow_buffer::ArrowBuffer,
    arrow_string::ArrowString,
    loggable::{Component, ComponentName, ComponentNameSet, Datatype, DatatypeName, Loggable},
    loggable_batch::{ComponentBatch, DatatypeBatch, LoggableBatch, MaybeOwnedComponentBatch},
    result::{
        DeserializationError, DeserializationResult, ResultExt, SerializationError,
        SerializationResult, _Backtrace,
    },
    size_bytes::SizeBytes,
    view::{SpaceViewClassIdentifier, View},
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

/// Fundamental [`Datatype`]s that are implemented in `re_types_core` directly for convenience and
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
    pub use arrow2;
    pub use re_tuid;
}
