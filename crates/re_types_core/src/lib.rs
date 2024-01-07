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
/// Have a look at our [Custom Data] example to learn more about handwritten bundles.
///
/// [IDL definitions]: https://github.com/rerun-io/rerun/tree/latest/crates/re_types/definitions/rerun
/// [Custom Data]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data/src/main.rs
pub trait AsComponents {
    /// Exposes the object's contents as a set of [`ComponentBatch`]s.
    ///
    /// This is the main mechanism for easily extending builtin archetypes or even writing
    /// fully custom ones.
    /// Have a look at our [Custom Data] example to learn more about extending archetypes.
    ///
    /// [Custom Data]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data/src/main.rs
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>>;

    /// The number of instances in each batch.
    ///
    /// If not implemented, the number of instances will be determined by the longest
    /// batch in the bundle.
    ///
    /// Each batch returned by `as_component_batches` should have this number of elements,
    /// or 1 in the case it is a splat, or 0 in the case that component is being cleared.
    #[inline]
    fn num_instances(&self) -> usize {
        self.as_component_batches()
            .into_iter()
            .map(|comp_batch| comp_batch.as_ref().num_instances())
            .max()
            .unwrap_or(0)
    }

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
mod loggable;
mod loggable_batch;
mod result;
mod size_bytes;
mod tuid;
mod tuple;

pub use self::archetype::{
    Archetype, ArchetypeName, GenericIndicatorComponent, NamedIndicatorComponent,
};
pub use self::loggable::{
    Component, ComponentName, ComponentNameSet, Datatype, DatatypeName, Loggable,
};
pub use self::loggable_batch::{
    ComponentBatch, DatatypeBatch, LoggableBatch, MaybeOwnedComponentBatch,
};
pub use self::result::{
    DeserializationError, DeserializationResult, ResultExt, SerializationError,
    SerializationResult, _Backtrace,
};
pub use self::size_bytes::SizeBytes;

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

mod arrow_buffer;
mod arrow_string;
pub use self::arrow_buffer::ArrowBuffer;
pub use self::arrow_string::ArrowString;

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
