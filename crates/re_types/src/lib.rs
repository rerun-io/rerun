//! The standard Rerun data types, component types, and archetypes.
//!
//! This crate contains both the IDL definitions for Rerun types (flatbuffers) as well as the code
//! generated from those using `re_types_builder`.
//!
//! The [`Archetype`] trait is the core of this crate and is a good starting point to get familiar
//! with the code.
//! An archetype is a logical collection of [`Component`]s that play well with each other.
//!
//! Rerun (and the underlying Arrow data framework) is designed to work with large arrays of
//! [`Component`]s, as opposed to single instances.
//! When multiple instances of a [`Component`] are put together in an array, they yield a
//! [`ComponentBatch`]: the atomic unit of (de)serialization.
//!
//! Internally, [`Component`]s are implemented using many different [`Datatype`]s.
//!
//! All builtin archetypes, components and datatypes can be found in their respective top-level
//! modules.
//!
//! ## Contributing
//!
//! ### Organization
//!
//! - `definitions/` contains IDL definitions for all Rerun types (data, components, archetypes).
//! - `src/` contains the code generated for Rust.
//! - `rerun_py/rerun/rerun2/` (at the root of this workspace) contains the code generated for Python.
//!
//! While most of the code in this crate is auto-generated, some manual extensions are littered
//! throughout: look for files ending in `_ext.rs`, `_ext.py`, or `_ext.cpp` (also see the "Extensions"
//! section of this document).
//!
//!
//! ### Build cache
//!
//! Updating either the source code of the code generator itself (`re_types_builder`) or any of the
//! .fbs files should re-trigger the code generation process the next time `re_types` is built.
//! Manual extension files will be left untouched.
//!
//! Caching is controlled by a versioning hash that is stored in `store_hash.txt`.
//! If you suspect something is wrong with the caching mechanism and that your changes aren't taken
//! into account when they should, try and remove `source_hash.txt`.
//! If that fixes the issue, you've found a bug.
//!
//!
//! ### How-to: add a new datatype/component/archetype
//!
//! Create the appropriate .fbs file in the appropriate place, and make sure it gets included in
//! some way (most likely indirectly) by `archetypes.fbs`, which is the main entrypoint for
//! codegen.
//! Generally, the easiest thing to do is to add your new type to one of the centralized manifests,
//! e.g. for a new component, include it into `components.fbs`.
//!
//! Your file should get picked up automatically by the code generator.
//! Once the code for your new component has been generated, implement whatever extensions you need
//! and make sure to tests any custom constructors you add.
//!
//!
//! ### How-to: remove an existing datatype/component/archetype
//!
//! Simply get rid of the type in question and rebuild `re_types` to trigger codegen.
//!
//! Beware though: if you remove a whole definition file re-running codegen will not remove the
//! associated generated files, you'll have to do that yourself.
//!
//!
//! ### Extensions
//!
//!
//! #### Rust
//!
//! Generated Rust code can be manually extended by adding sibling files with the `_ext.rs`
//! prefix. E.g. to extend `vec2d.rs`, create a `vec2d_ext.rs`.
//!
//! Trigger the codegen (e.g. by removing `source_hash.txt`) to generate the right `mod` clauses
//! automatically.
//!
//! The simplest way to get started is to look at any of the existing examples.
//!
//!
//! #### Python
//!
//! Generated Python code can be manually extended by adding a sibling file with the `_ext.py`
//! prefix. E.g. to extend `vec2d.py`, create a `vec2d_ext.py`.
//!
//! This sibling file needs to implement an extension class that is mixed in with the
//! auto-generated class.
//! The simplest way to get started is to look at any of the existing examples.
//!
//!
//! #### C++
//!
//! Generated C++ code can be manually extended by adding a sibling file with the `_ext.cpp` suffix.
//! E.g. to extend `vec2d.cpp`, create a `vec2d_ext.cpp`.
//!
//! The sibling file is compiled as-is as part of the `rerun_cpp` crate.
//!
//! Any include directive used in the extension is automatically added to the generated header,
//! except to the generated header itself.
//!
//! In order to extend the generated type declaration in the header,
//! you can specify a single code-block that you want to be injected into the type declaration by
//! starting it with `[CODEGEN COPY TO HEADER START]` and ending it with `[CODEGEN COPY TO HEADER END]`.
//! Note that it is your responsibility to make sure that the cpp file is valid C++ code -
//! the code generator & build will not adjust the extension file for you!
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

/// Number of decimals shown for all vector display methods.
pub const DISPLAY_PRECISION: usize = 3;

pub mod archetypes;
pub mod components;
pub mod datatypes;

mod archetype;
mod loggable;
mod loggable_batch;
mod result;
mod size_bytes;

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

#[cfg(feature = "datagen")]
pub mod datagen;

// ---

mod arrow_buffer;
mod arrow_string;
pub use self::arrow_buffer::ArrowBuffer;
pub use self::arrow_string::ArrowString;

pub mod external {
    pub use anyhow;
    pub use arrow2;
    pub use uuid;

    #[cfg(feature = "ecolor")]
    pub use ecolor;

    #[cfg(feature = "glam")]
    pub use glam;

    #[cfg(feature = "image")]
    pub use image;
}

// TODO(jleibs): Should all of this go into `tensor_data_ext`? Don't have a good way to export
// additional helpers yet.
pub mod image;
pub mod tensor_data;
pub mod view_coordinates;

#[cfg(feature = "testing")]
pub mod testing;
