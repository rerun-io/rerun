use nohash_hasher::IntSet;
use re_byte_size::SizeBytes;

#[expect(unused_imports, clippy::unused_trait_names)] // used in docstrings
use crate::{Archetype, ComponentBatch};
use crate::{ComponentIdentifier, DeserializationResult, SerializationResult};

// ---

/// Describes the Arrow datatype that a type is (de)serialized to and from.
///
/// This is the base trait of the four (de)serialization traits ([`ToArrow`], [`ToArrowOpt`],
/// [`FromArrow`], [`FromArrowOpt`]): a type may implement any subset of those, but it must always
/// agree on a single datatype.
pub trait ArrowDatatype: Sized {
    /// The underlying [`arrow::datatypes::DataType`], excluding datatype extensions.
    fn arrow_datatype() -> arrow::datatypes::DataType;

    /// Returns an empty Arrow array that matches this type's underlying datatype.
    #[inline]
    fn arrow_empty() -> arrow::array::ArrayRef {
        arrow::array::new_empty_array(&Self::arrow_datatype())
    }
}

/// Serializes an iterator of values into an Arrow array.
///
/// Most types implement this in terms of [`ToArrowOpt`]; see [`crate::macros::impl_to_arrow_via_to_arrow_opt`].
/// Types whose Arrow encoding cannot express null values (e.g. `Tuid`) implement this one only.
pub trait ToArrow: ArrowDatatype + Clone {
    /// Given an iterator of owned or reference values, serializes them into an Arrow array.
    ///
    /// When using Rerun's builtin components & encodings, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a;
}

/// Serializes an iterator of optional values into a nullable Arrow array.
pub trait ToArrowOpt: ArrowDatatype + Clone {
    /// Given an iterator of options of owned or reference values, serializes them into an Arrow
    /// array.
    ///
    /// When using Rerun's builtin components & encodings, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a;
}

/// Deserializes an Arrow array into a collection of values, failing on nulls.
pub trait FromArrow: ArrowDatatype {
    /// Given an Arrow array, deserializes it into a collection of values.
    fn from_arrow(data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>;

    /// Verifies that the given Arrow array can be deserialized into a collection of [`Self`]s.
    ///
    /// Calls [`Self::from_arrow`] and returns an error if it fails.
    fn verify_arrow_array(data: &dyn arrow::array::Array) -> DeserializationResult<()> {
        Self::from_arrow(data).map(|_| ())
    }
}

/// Deserializes a nullable Arrow array into a collection of optional values.
pub trait FromArrowOpt: ArrowDatatype {
    /// Given an Arrow array, deserializes it into a collection of optional values.
    fn from_arrow_opt(data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Option<Self>>>;
}

// --- Bridges between the serialization traits ---

/// Implements [`ToArrow::to_arrow`] in terms of [`ToArrowOpt`].
///
/// See [`crate::macros::impl_to_arrow_via_to_arrow_opt`].
#[inline]
pub fn to_arrow_via_to_arrow_opt<'a, T: ToArrowOpt + 'a>(
    data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, T>>>,
) -> SerializationResult<arrow::array::ArrayRef> {
    T::to_arrow_opt(data.into_iter().map(Some))
}

/// Implements [`FromArrow::from_arrow`] in terms of [`FromArrowOpt`], failing on nulls.
///
/// See [`crate::macros::impl_from_arrow_via_from_arrow_opt`].
#[inline]
pub fn from_arrow_via_from_arrow_opt<T: FromArrowOpt>(
    data: &dyn arrow::array::Array,
) -> DeserializationResult<Vec<T>> {
    T::from_arrow_opt(data)?
        .into_iter()
        .map(|opt| opt.ok_or_else(crate::DeserializationError::missing_data))
        .collect()
}

/// Implements [`FromArrowOpt::from_arrow_opt`] in terms of [`FromArrow`].
///
/// The resulting array never contains nulls.
///
/// See [`crate::macros::impl_from_arrow_opt_via_from_arrow`].
#[inline]
pub fn from_arrow_opt_via_from_arrow<T: FromArrow>(
    data: &dyn arrow::array::Array,
) -> DeserializationResult<Vec<Option<T>>> {
    T::from_arrow(data).map(|v| v.into_iter().map(Some).collect())
}

/// A [`Component`] describes semantic data that can be used by any number of [`Archetype`]s.
///
/// A component round-trips through Arrow: it must implement [`ToArrow`] and [`FromArrow`].
///
/// Note that the nullable variants, [`ToArrowOpt`] and [`FromArrowOpt`], are deliberately *not*
/// required: a component has to round-trip, but it does not have to be nullable.
///
/// Implementing the [`Component`] trait automatically derives the [`ComponentBatch`] implementation,
/// which makes it possible to work with lists' worth of data in a generic fashion.
pub trait Component:
    'static + Send + Sync + Clone + Sized + SizeBytes + ToArrow + FromArrow
{
    /// The fully-qualified type of this component, e.g. `rerun.components.Position2D`.
    fn name() -> ComponentType;
}

// ---

pub type UnorderedComponentSet = IntSet<ComponentIdentifier>;

pub type ComponentSet = std::collections::BTreeSet<ComponentIdentifier>;

re_string_interner::declare_new_type_nonempty!(
    /// The fully-qualified name of a [`Component`], e.g. `rerun.components.Position2D`.
    pub struct ComponentType;
);

impl ComponentType {
    /// Runs some asserts in debug mode to make sure the name is not weird.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        let full_type = self.0.as_str();
        re_log::debug_assert!(
            !full_type.starts_with("rerun.components.rerun.components."),
            "Found component with full type {full_type:?}. Maybe some bad round-tripping?"
        );
    }

    /// Returns the fully-qualified name, e.g. `rerun.components.Position2D`.
    ///
    /// This is the default `Display` implementation for [`ComponentType`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.sanity_check();
        self.0.as_str()
    }

    /// Returns the unqualified name, e.g. `Position2D`.
    ///
    /// Used for most UI elements.
    ///
    /// ```
    /// # use re_types_core::ComponentType;
    /// assert_eq!(ComponentType::from("rerun.components.Position2D").short_name(), "Position2D");
    /// ```
    #[inline]
    pub fn short_name(&self) -> &'static str {
        self.sanity_check();
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.blueprint.components.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.components.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.controls.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else {
            full_name
        }
    }

    /// Web URL to the Rerun documentation for this component.
    pub fn doc_url(&self) -> Option<String> {
        if let Some(component_type_pascal_case) = self.full_name().strip_prefix("rerun.components.")
        {
            // This code should be correct as long as this url passes our link checker:
            // https://rerun.io/docs/reference/types/components/line_strip2d

            let component_type_snake_case = re_case::to_snake_case(component_type_pascal_case);
            let base_url = "https://rerun.io/docs/reference/types/components";
            Some(format!("{base_url}/{component_type_snake_case}"))
        } else {
            None // A user component
        }
    }

    /// Determine if component matches a string
    ///
    /// Valid matches are case invariant matches of either the full name or the short name.
    pub fn matches(&self, other: &str) -> bool {
        self.0.as_str() == other
            || self.full_name().to_lowercase() == other.to_lowercase()
            || self.short_name().to_lowercase() == other.to_lowercase()
    }

    /// Returns `true` if this is a known Rerun component type (e.g., `rerun.components.*`, `rerun.blueprint.components.*`).
    ///
    /// Returns `false` for custom user-defined components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use re_types_core::ComponentType;
    /// assert!(ComponentType::from("rerun.components.Position2D").is_rerun_type());
    /// assert!(ComponentType::from("rerun.blueprint.components.Active").is_rerun_type());
    /// assert!(!ComponentType::from("my_custom.MyComponent").is_rerun_type());
    /// ```
    #[inline]
    pub fn is_rerun_type(&self) -> bool {
        self.0.as_str().starts_with("rerun.")
    }
}
