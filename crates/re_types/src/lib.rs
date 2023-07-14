//! The standard Rerun data types, component types, and archetypes.
//!
//! This crate contains both the IDL definitions for Rerun types (flatbuffers) as well as the code
//! generated from those using `re_types_builder`.
//!
//!
//! ### Organization
//!
//! - `definitions/` contains IDL definitions for all Rerun types (data, components, archetypes).
//! - `src/` contains the code generated for Rust.
//! - `rerun_py/rerun/rerun2/` (at the root of this workspace) contains the code generated for Python.
//!
//! While most of the code in this crate is auto-generated, some manual extensions are littered
//! throughout: look for files ending in `_ext.rs` or `_ext.py` (also see the "Extensions" section
//! of this document).
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

// TODO(cmc): `Datatype` & `Component` being full-blown copies of each other is a bit dumb... but
// things are bound to evolve very soon anyway (see e.g. Jeremy's paper).

// ---

/// Anything that can be serialized to and deserialized from Arrow data.
pub trait Loggable {
    type Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name() -> Self::Name;

    /// The underlying [`arrow2::datatypes::DataType`].
    fn to_arrow_datatype() -> arrow2::datatypes::DataType;

    // ---

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::to_arrow_datatype`].
    ///
    /// Panics on failure.
    /// This will _never_ fail for Rerun's builtin [`Loggable`]s.
    ///
    /// For the fallible version, see [`Loggable::try_to_arrow`].
    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, Self>>>,
        extension_wrapper: Option<&str>,
    ) -> Box<dyn ::arrow2::array::Array>
    where
        Self: Clone + 'a,
    {
        Self::try_to_arrow_opt(data.into_iter().map(Some), extension_wrapper).unwrap()
    }

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::to_arrow_datatype`].
    ///
    /// This will _never_ fail for Rerun's builtin [`Loggable`].
    /// For the non-fallible version, see [`Loggable::to_arrow`].
    #[inline]
    fn try_to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, Self>>>,
        extension_wrapper: Option<&str>,
    ) -> SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        Self::try_to_arrow_opt(data.into_iter().map(Some), extension_wrapper)
    }

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::to_arrow_datatype`].
    ///
    /// Panics on failure.
    /// This will _never_ fail for Rerun's builtin [`Loggable`].
    ///
    /// For the fallible version, see [`Loggable::try_to_arrow_opt`].
    #[inline]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> Box<dyn ::arrow2::array::Array>
    where
        Self: Clone + 'a,
    {
        Self::try_to_arrow_opt(data, extension_wrapper).unwrap()
    }

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::to_arrow_datatype`].
    ///
    /// This will _never_ fail for Rerun's builtin [`Loggable`].
    /// For the non-fallible version, see [`Loggable::to_arrow_opt`].
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a;

    // ---

    /// Given an Arrow array, deserializes it into a collection of [`Loggable`]s.
    ///
    /// Panics if the data schema doesn't match, or if optional entries were missing at runtime.
    /// For the non-fallible version, see [`Loggable::try_from_arrow`].
    #[inline]
    fn from_arrow(data: &dyn ::arrow2::array::Array) -> Vec<Self>
    where
        Self: Sized,
    {
        Self::from_arrow_opt(data)
            .into_iter()
            .map(Option::unwrap)
            .collect()
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail for if the Arrow array's datatype matches the one returned by
    /// [`Loggable::to_arrow_datatype`].
    /// For the non-fallible version, see [`Loggable::from_arrow_opt`].
    #[inline]
    fn try_from_arrow(data: &dyn ::arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        Self::try_from_arrow_opt(data)?
            .into_iter()
            .map(|v| {
                v.ok_or_else(|| DeserializationError::MissingData {
                    datatype: data.data_type().clone(),
                })
            })
            .collect()
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail for if the Arrow array's datatype matches the one returned by
    /// [`Loggable::to_arrow_datatype`].
    /// For the fallible version, see [`Loggable::try_from_arrow_opt`].
    #[inline]
    fn from_arrow_opt(data: &dyn ::arrow2::array::Array) -> Vec<Option<Self>>
    where
        Self: Sized,
    {
        Self::try_from_arrow_opt(data).unwrap()
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail for if the Arrow array's datatype matches the one returned by
    /// [`Loggable::to_arrow_datatype`].
    /// For the non-fallible version, see [`Loggable::from_arrow_opt`].
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized;
}

/// The fully-qualified name of a [`Datatype`], e.g. `rerun.datatypes.Vec2D`.
pub type DatatypeName = ::std::borrow::Cow<'static, str>;

/// A [`Datatype`] describes plain old data that can be used by any number of [`Component`].
pub trait Datatype: Loggable {}

/// The fully-qualified name of a [`Component`], e.g. `rerun.components.Point2D`.
pub type ComponentName = ::std::borrow::Cow<'static, str>;

pub trait Component: Loggable<Name = ComponentName> {}

// ---

/// The fully-qualified name of an [`Archetype`], e.g. `rerun.archetypes.Points2D`.
pub type ArchetypeName = ::std::borrow::Cow<'static, str>;

pub trait Archetype {
    /// The fully-qualified name of this archetype, e.g. `rerun.archetypes.Points2D`.
    fn name() -> ArchetypeName;

    // ---

    /// The fully-qualified component names of every component that _must_ be provided by the user
    /// when constructing this archetype.
    fn required_components() -> Vec<ComponentName>;

    /// The fully-qualified component names of every component that _should_ be provided by the user
    /// when constructing this archetype.
    fn recommended_components() -> Vec<ComponentName>;

    /// The fully-qualified component names of every component that _could_ be provided by the user
    /// when constructing this archetype.
    fn optional_components() -> Vec<ComponentName>;

    // ---

    /// Serializes all non-null [`Component`]s of this [`Archetype`] into Arrow arrays.
    ///
    /// Panics on failure.
    /// This can _never_ fail for Rerun's builtin archetypes.
    ///
    /// For the fallible version, see [`Archetype::try_to_arrow`].
    #[inline]
    fn to_arrow(&self) -> Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)> {
        self.try_to_arrow().unwrap()
    }

    /// Serializes all non-null [`Component`]s of this [`Archetype`] into Arrow arrays.
    ///
    /// This can _never_ fail for Rerun's builtin archetypes.
    /// For the non-fallible version, see [`Archetype::to_arrow`].
    fn try_to_arrow(
        &self,
    ) -> SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>>;

    // ---

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Panics on failure.
    /// For the fallible version, see [`Archetype::try_from_arrow`].
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow(
        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    ) -> Self
    where
        Self: Sized,
    {
        Self::try_from_arrow(data).unwrap()
    }

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    ///
    /// For the non-fallible version, see [`Archetype::from_arrow`].
    fn try_from_arrow(
        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized;
}

// ---

#[derive(thiserror::Error, Debug)]
pub enum SerializationError {
    #[error(
        "Trying to serialize field {obj_field_fqname:?} with unsupported datatype: {datatype:#?}:"
    )]
    UnsupportedDatatype {
        obj_field_fqname: String,
        datatype: ::arrow2::datatypes::DataType,
    },
}

pub type SerializationResult<T> = ::std::result::Result<T, SerializationError>;

#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error("Not implemented")]
    NotImplemented,

    #[error("Missing data for {datatype:#?}")]
    MissingData {
        datatype: ::arrow2::datatypes::DataType,
    },

    #[error("Expected {expected:#?} but found {got:#?} instead")]
    SchemaMismatch {
        expected: ::arrow2::datatypes::DataType,
        got: ::arrow2::datatypes::DataType,
    },

    #[error(
        "Offsets were ouf of bounds, trying to read from {bounds:?} in an array of size {len}"
    )]
    OffsetsMismatch {
        bounds: (usize, usize),
        len: usize,
        datatype: ::arrow2::datatypes::DataType,
    },

    #[error("Expected array of length {expected} but found a length of {got:#?} instead")]
    ArrayLengthMismatch {
        expected: usize,
        got: usize,
        datatype: ::arrow2::datatypes::DataType,
    },

    #[error("Expected single-instanced component but found {got} instances instead")]
    MonoMismatch {
        got: usize,
        datatype: ::arrow2::datatypes::DataType,
    },
}

pub type DeserializationResult<T> = ::std::result::Result<T, DeserializationError>;

// ---

/// Number of decimals shown for all vector display methods.
pub const DISPLAY_PRECISION: usize = 3;

pub mod archetypes;
pub mod components;
pub mod datatypes;
