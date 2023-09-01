use crate::{result::_Backtrace, DeserializationResult, ResultExt as _, SerializationResult};

#[allow(unused_imports)] // used in docstrings
use crate::{Archetype, ComponentList, DatatypeList, LoggableList};

// ---

/// A [`Loggable`] represents a single instance in an array of loggable data.
///
/// Internally, Arrow, and by extension Rerun, only deal with arrays of data.
/// We refer to individual entries in these arrays as instances.
///
/// [`Datatype`] and [`Component`] are specialization of the [`Loggable`] trait that are
/// automatically implemented based on the type used for [`Loggable::Name`].
///
/// Implementing the [`Loggable`] trait (and by extension [`Datatype`]/[`Component`])
/// automatically derives the [`LoggableList`] implementation (and by extension
/// [`DatatypeList`]/[`ComponentList`]), which makes it possible to work with lists' worth of data
/// in a generic fashion.
pub trait Loggable: Clone + Sized {
    type Name: std::fmt::Display;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name() -> Self::Name;

    /// The underlying [`arrow2::datatypes::DataType`], excluding datatype extensions.
    fn arrow_datatype() -> arrow2::datatypes::DataType;

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_field`].
    ///
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    ///
    /// For the non-fallible version, see [`Loggable::to_arrow_opt`].
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: 'a;

    // --- Optional metadata methods ---

    /// The underlying [`arrow2::datatypes::DataType`], including datatype extensions.
    ///
    /// The default implementation will simply wrap [`Self::arrow_datatype`] in an extension called
    /// [`Self::name`], which is what you want in most cases.
    #[inline]
    fn extended_arrow_datatype() -> arrow2::datatypes::DataType {
        arrow2::datatypes::DataType::Extension(
            Self::name().to_string(),
            Box::new(Self::arrow_datatype()),
            None,
        )
    }

    /// The underlying [`arrow2::datatypes::Field`], including datatype extensions.
    ///
    /// The default implementation will simply wrap [`Self::extended_arrow_datatype`] in a
    /// [`arrow2::datatypes::Field`], which is what you want in most cases (e.g. because you want
    /// to declare the field as nullable).
    #[inline]
    fn arrow_field() -> arrow2::datatypes::Field {
        arrow2::datatypes::Field::new(
            Self::name().to_string(),
            Self::extended_arrow_datatype(),
            false,
        )
    }

    // --- Optional serialization methods ---

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_field`].
    ///
    /// Panics on failure.
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    ///
    /// For the fallible version, see [`Loggable::try_to_arrow`].
    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> Box<dyn ::arrow2::array::Array>
    where
        Self: 'a,
    {
        Self::try_to_arrow_opt(data.into_iter().map(Some)).detailed_unwrap()
    }

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_field`].
    ///
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    ///
    /// For the non-fallible version, see [`Loggable::to_arrow`].
    #[inline]
    fn try_to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: 'a,
    {
        Self::try_to_arrow_opt(data.into_iter().map(Some))
    }

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    /// The Arrow array's datatype will match [`Loggable::arrow_field`].
    ///
    /// Panics on failure.
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    ///
    /// For the fallible version, see [`Loggable::try_to_arrow_opt`].
    #[inline]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> Box<dyn ::arrow2::array::Array>
    where
        Self: 'a,
    {
        Self::try_to_arrow_opt(data).detailed_unwrap()
    }

    // --- Optional deserialization methods ---

    /// Given an Arrow array, deserializes it into a collection of [`Loggable`]s.
    ///
    /// This will _never_ fail if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_field`].
    #[inline]
    fn try_from_arrow(data: &dyn ::arrow2::array::Array) -> DeserializationResult<Vec<Self>> {
        Self::try_from_arrow_opt(data)?
            .into_iter()
            .map(|opt| {
                opt.ok_or_else(|| crate::DeserializationError::MissingData {
                    backtrace: _Backtrace::new_unresolved(),
                })
            })
            .collect::<DeserializationResult<Vec<_>>>()
            .with_context(Self::name().to_string())
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    ///
    /// This will _never_ fail if the Arrow array's datatype matches the one returned by
    /// [`Loggable::arrow_field`].
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>> {
        _ = data; // NOTE: do this here to avoid breaking users' autocomplete snippets
        Err(crate::DeserializationError::NotImplemented {
            fqname: Self::name().to_string(),
            backtrace: _Backtrace::new_unresolved(),
        })
    }
}

/// A [`Datatype`] describes plain old data that can be used by any number of [`Component`]s.
///
/// Any [`Loggable`] with a [`Loggable::Name`] set to [`DatatypeName`] automatically implements
/// [`Datatype`].
pub trait Datatype: Loggable<Name = DatatypeName> {}

impl<L: Loggable<Name = DatatypeName>> Datatype for L {}

/// A [`Component`] describes semantic data that can be used by any number of [`Archetype`]s.
///
/// Any [`Loggable`] with a [`Loggable::Name`] set to [`ComponentName`] automatically implements
/// [`Component`].
pub trait Component: Loggable<Name = ComponentName> {}

impl<L: Loggable<Name = ComponentName>> Component for L {}

// ---

re_string_interner::declare_new_type!(
    /// The fully-qualified name of a [`Component`], e.g. `rerun.components.Point2D`.
    pub struct ComponentName;
);

impl ComponentName {
    /// Returns the fully-qualified name, e.g. `rerun.components.Point2D`.
    ///
    /// This is the default `Display` implementation for [`ComponentName`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.0.as_str()
    }

    /// Returns the unqualified name, e.g. `Point2D`.
    ///
    /// Used for most UI elements.
    ///
    /// ```
    /// # use re_types::ComponentName;
    /// assert_eq!(ComponentName::from("rerun.components.Point2D").short_name(), "Point2D");
    /// ```
    #[inline]
    pub fn short_name(&self) -> &'static str {
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.components.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else {
            full_name
        }
    }
}

// ---

impl crate::SizeBytes for ComponentName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

re_string_interner::declare_new_type!(
    /// The fully-qualified name of a [`Datatype`], e.g. `rerun.datatypes.Vec2D`.
    pub struct DatatypeName;
);

impl DatatypeName {
    /// Returns the fully-qualified name, e.g. `rerun.datatypes.Vec2D`.
    ///
    /// This is the default `Display` implementation for [`DatatypeName`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.0.as_str()
    }

    /// Returns the unqualified name, e.g. `Vec2D`.
    ///
    /// Used for most UI elements.
    ///
    /// ```
    /// # use re_types::DatatypeName;
    /// assert_eq!(DatatypeName::from("rerun.datatypes.Vec2D").short_name(), "Vec2D");
    /// ```
    #[inline]
    pub fn short_name(&self) -> &'static str {
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.datatypes.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else {
            full_name
        }
    }
}

impl crate::SizeBytes for DatatypeName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}
