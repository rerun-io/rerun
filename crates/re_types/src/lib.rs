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

// ---

/// Anything that can be serialized to and deserialized from Arrow data.
pub trait Loggable: Sized {
    type Name;
    type Item<'a>;
    type Iter<'a>: Iterator<Item = Self::Item<'a>>;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name() -> Self::Name;

    /// The underlying [`arrow2::datatypes::DataType`].
    fn arrow_datatype() -> arrow2::datatypes::DataType;

    // ---

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_datatype`].
    ///
    /// Panics on failure.
    /// This will _never_ fail for Rerun's built-in [`Loggable`]s.
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
        Self::try_to_arrow_opt(data.into_iter().map(Some), extension_wrapper).detailed_unwrap()
    }

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_datatype`].
    ///
    /// This will _never_ fail for Rerun's built-in [`Loggable`].
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
    /// The Arrow array's datatype will match [`Loggable::arrow_datatype`].
    ///
    /// Panics on failure.
    /// This will _never_ fail for Rerun's built-in [`Loggable`].
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
        Self::try_to_arrow_opt(data, extension_wrapper).detailed_unwrap()
    }

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_datatype`].
    ///
    /// This will _never_ fail for Rerun's built-in [`Loggable`].
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
    fn from_arrow(data: &dyn ::arrow2::array::Array) -> Vec<Self> {
        Self::try_from_arrow(data).detailed_unwrap()
    }

    /// Given an Arrow array, deserializes it into a collection of [`Loggable`]s.
    ///
    /// This will _never_ fail if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_datatype`].
    /// For the non-fallible version, see [`Loggable::from_arrow_opt`].
    #[inline]
    fn try_from_arrow(data: &dyn ::arrow2::array::Array) -> DeserializationResult<Vec<Self>> {
        Ok(Self::try_iter_from_arrow(data)?
            .map(Self::convert_item_to_self)
            .collect())
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_datatype`].
    /// For the fallible version, see [`Loggable::try_from_arrow_opt`].
    #[inline]
    fn from_arrow_opt(data: &dyn ::arrow2::array::Array) -> Vec<Option<Self>> {
        Self::try_from_arrow_opt(data).detailed_unwrap()
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_datatype`].
    /// For the non-fallible version, see [`Loggable::from_arrow_opt`].
    #[inline]
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>> {
        re_tracing::profile_function!();
        Ok(Self::try_iter_from_arrow(data)?
            .map(Self::convert_item_to_opt_self)
            .collect())
    }

    /// Given an Arrow array, deserializes it into a iterator of [`Loggable::Item`]s.
    ///
    /// Note: mostly for reasons related to typing of trait implementations, the implementor
    /// of [`Loggable`] may choose an arbitrary iterable [`Loggable::Item`] that  differs from
    /// the [`Loggable`] itself.
    ///
    /// These items can be be converted to an optional [`Loggable`] using [`Loggable::convert_item_to_self`].
    ///
    /// This is the base deserialization mechanism that all [`Loggable`] implementors must provide. All other
    /// conversions above can be generated from this primitive.
    ///
    /// This will _never_ fail for if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_datatype`].
    fn try_iter_from_arrow(
        data: &dyn ::arrow2::array::Array,
    ) -> DeserializationResult<Self::Iter<'_>>;

    /// Convert a [`Loggable::Item`] into a [`Loggable`]
    ///
    /// This is intended to be used with [`Loggable::try_iter_from_arrow`] when the type
    /// is known to be non-nullible.
    #[inline]
    fn convert_item_to_self(item: Self::Item<'_>) -> Self {
        // TODO(jleibs): This unwrap goes away when we remove the iterator abstraction
        Self::convert_item_to_opt_self(item).unwrap()
    }

    /// Convert a [`Loggable::Item`] into an optional [`Loggable`]
    ///
    /// This is intended to be used with [`Loggable::try_iter_from_arrow`]
    fn convert_item_to_opt_self(item: Self::Item<'_>) -> Option<Self>;
}

/// The fully-qualified name of a [`Datatype`], e.g. `rerun.datatypes.Vec2D`.
pub type DatatypeName = ::std::borrow::Cow<'static, str>;

/// A [`Datatype`] describes plain old data that can be used by any number of [`Component`].
pub trait Datatype: Loggable {}

pub trait Component: Loggable<Name = ComponentName> + Clone {}

// ---

/// The fully-qualified name of an [`Archetype`], e.g. `rerun.archetypes.Points2D`.
pub type ArchetypeName = ::std::borrow::Cow<'static, str>;

pub trait Archetype {
    /// The fully-qualified name of this archetype, e.g. `rerun.archetypes.Points2D`.
    fn name() -> ArchetypeName;

    // ---

    /// The fully-qualified component names of every component that _must_ be provided by the user
    /// when constructing this archetype.
    fn required_components() -> &'static [ComponentName];

    /// The fully-qualified component names of every component that _should_ be provided by the user
    /// when constructing this archetype.
    fn recommended_components() -> &'static [ComponentName];

    /// The fully-qualified component names of every component that _could_ be provided by the user
    /// when constructing this archetype.
    fn optional_components() -> &'static [ComponentName];

    /// All components including required, recommended, and optional.
    fn all_components() -> &'static [ComponentName];

    /// Returns the name of the associated indicator component, whose presence indicates that the
    /// high-level archetype-based APIs where used to log the data.
    ///
    /// Indicator components open new opportunities in terms of API design, better heuristics and
    /// performance improvements on the query side.
    ///
    /// Indicator components are non-splatted null arrays.
    /// Their names follow the pattern `rerun.components.{ArchetypeName}Indicator`, e.g.
    /// `rerun.components.Points3DIndicator`.
    ///
    /// The reason for not using splats is so that indicator components don't require dedicated rows.
    /// This is not an issue because of the way null arrays are stored: storing 1 null value or 1M null
    /// values takes the same size.
    fn indicator_component() -> ComponentName;

    /// Returns the number of instances of the archetype, i.e. the number of instances currently
    /// present in its required component(s).
    fn num_instances(&self) -> usize;

    // ---

    /// Serializes all non-null [`Component`]s of this [`Archetype`] into Arrow arrays.
    ///
    /// Panics on failure.
    /// This can _never_ fail for Rerun's built-in archetypes.
    ///
    /// For the fallible version, see [`Archetype::try_to_arrow`].
    #[inline]
    fn to_arrow(&self) -> Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)> {
        self.try_to_arrow().detailed_unwrap()
    }

    /// Serializes all non-null [`Component`]s of this [`Archetype`] into Arrow arrays.
    ///
    /// This can _never_ fail for Rerun's built-in archetypes.
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
        Self::try_from_arrow(data).detailed_unwrap()
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

// NOTE: We have to make an alias, otherwise we'll trigger `thiserror`'s magic codepath which will
// attempt to use nightly features.
pub type _Backtrace = backtrace::Backtrace;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SerializationError {
    #[error("Failed to serialize {location:?}")]
    Context {
        location: String,
        source: Box<SerializationError>,
    },

    #[error("arrow2-convert serialization Failed: {0}")]
    ArrowConvertFailure(String),
}

impl SerializationError {
    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    pub fn backtrace(&self) -> Option<_Backtrace> {
        match self {
            SerializationError::Context { .. } | SerializationError::ArrowConvertFailure(_) => None,
        }
    }
}

pub type SerializationResult<T> = ::std::result::Result<T, SerializationError>;

#[derive(thiserror::Error, Debug, Clone)]
pub enum DeserializationError {
    #[error("Failed to deserialize {location:?}")]
    Context {
        location: String,
        #[source]
        source: Box<DeserializationError>,
    },

    #[error("Expected non-nullable data but didn't find any")]
    MissingData { backtrace: _Backtrace },

    #[error("Expected field {field_name:?} to be present in {datatype:#?}")]
    MissingStructField {
        datatype: ::arrow2::datatypes::DataType,
        field_name: String,
        backtrace: _Backtrace,
    },

    #[error("Expected union arm {arm_name:?} (#{arm_index}) to be present in {datatype:#?}")]
    MissingUnionArm {
        datatype: ::arrow2::datatypes::DataType,
        arm_name: String,
        arm_index: usize,
        backtrace: _Backtrace,
    },

    #[error("Expected {expected:#?} but found {got:#?} instead")]
    DatatypeMismatch {
        expected: ::arrow2::datatypes::DataType,
        got: ::arrow2::datatypes::DataType,
        backtrace: _Backtrace,
    },

    #[error("Offset ouf of bounds: trying to read at offset #{offset} in an array of size {len}")]
    OffsetOutOfBounds {
        offset: usize,
        len: usize,
        backtrace: _Backtrace,
    },

    #[error(
        "Offset slice ouf of bounds: trying to read offset slice at [#{from}..#{to}] in an array of size {len}"
    )]
    OffsetSliceOutOfBounds {
        from: usize,
        to: usize,
        len: usize,
        backtrace: _Backtrace,
    },

    #[error("arrow2-convert deserialization Failed: {0}")]
    ArrowConvertFailure(String),

    #[error("Datacell deserialization Failed: {0}")]
    DataCellError(String),

    #[error("Validation Error: {0}")]
    ValidationError(String),
}

impl DeserializationError {
    #[inline]
    pub fn missing_data() -> Self {
        Self::MissingData {
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn missing_struct_field(
        datatype: arrow2::datatypes::DataType,
        field_name: impl AsRef<str>,
    ) -> Self {
        Self::MissingStructField {
            datatype,
            field_name: field_name.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn missing_union_arm(
        datatype: arrow2::datatypes::DataType,
        arm_name: impl AsRef<str>,
        arm_index: usize,
    ) -> Self {
        Self::MissingUnionArm {
            datatype,
            arm_name: arm_name.as_ref().into(),
            arm_index,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn datatype_mismatch(
        expected: arrow2::datatypes::DataType,
        got: arrow2::datatypes::DataType,
    ) -> Self {
        Self::DatatypeMismatch {
            expected,
            got,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn offset_oob(offset: usize, len: usize) -> Self {
        Self::OffsetOutOfBounds {
            offset,
            len,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn offset_slice_oob((from, to): (usize, usize), len: usize) -> Self {
        Self::OffsetSliceOutOfBounds {
            from,
            to,
            len,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    #[inline]
    pub fn backtrace(&self) -> Option<_Backtrace> {
        match self {
            DeserializationError::Context {
                location: _,
                source,
            } => source.backtrace(),
            DeserializationError::MissingStructField { backtrace, .. }
            | DeserializationError::MissingUnionArm { backtrace, .. }
            | DeserializationError::MissingData { backtrace }
            | DeserializationError::DatatypeMismatch { backtrace, .. }
            | DeserializationError::OffsetOutOfBounds { backtrace, .. }
            | DeserializationError::OffsetSliceOutOfBounds { backtrace, .. } => {
                Some(backtrace.clone())
            }
            DeserializationError::ArrowConvertFailure(_)
            | DeserializationError::DataCellError(_)
            | DeserializationError::ValidationError(_) => None,
        }
    }
}

pub type DeserializationResult<T> = ::std::result::Result<T, DeserializationError>;

trait ResultExt<T> {
    fn with_context(self, location: impl AsRef<str>) -> Self;
    fn detailed_unwrap(self) -> T;
}

impl<T> ResultExt<T> for SerializationResult<T> {
    #[inline]
    fn with_context(self, location: impl AsRef<str>) -> Self {
        self.map_err(|err| SerializationError::Context {
            location: location.as_ref().into(),
            source: Box::new(err),
        })
    }

    #[track_caller]
    fn detailed_unwrap(self) -> T {
        match self {
            Ok(v) => v,
            Err(err) => {
                let bt = err.backtrace().map(|mut bt| {
                    bt.resolve();
                    bt
                });

                let err = Box::new(err) as Box<dyn std::error::Error>;
                if let Some(bt) = bt {
                    panic!("{}:\n{:#?}", re_error::format(&err), bt)
                } else {
                    panic!("{}", re_error::format(&err))
                }
            }
        }
    }
}

impl<T> ResultExt<T> for DeserializationResult<T> {
    #[inline]
    fn with_context(self, location: impl AsRef<str>) -> Self {
        self.map_err(|err| DeserializationError::Context {
            location: location.as_ref().into(),
            source: Box::new(err),
        })
    }

    #[track_caller]
    fn detailed_unwrap(self) -> T {
        match self {
            Ok(v) => v,
            Err(err) => {
                let bt = err.backtrace().map(|mut bt| {
                    bt.resolve();
                    bt
                });

                let err = Box::new(err) as Box<dyn std::error::Error>;
                if let Some(bt) = bt {
                    panic!("{}:\n{:#?}", re_error::format(&err), bt)
                } else {
                    panic!("{}", re_error::format(&err))
                }
            }
        }
    }
}

// ---

/// Number of decimals shown for all vector display methods.
pub const DISPLAY_PRECISION: usize = 3;

pub mod archetypes;
pub mod components;
pub mod datatypes;

mod component_name;
mod size_bytes;

pub use component_name::ComponentName;
pub use size_bytes::SizeBytes;

mod arrow_buffer;
mod arrow_string;
mod tensor_data;
pub use arrow_buffer::ArrowBuffer;
pub use arrow_string::ArrowString;
pub use tensor_data::{TensorDataType, TensorDataTypeTrait, TensorElement};

#[cfg(feature = "testing")]
pub mod testing;
