use std::borrow::Cow;

use nohash_hasher::IntSet;

use re_byte_size::SizeBytes;

use crate::{ComponentDescriptor, DeserializationResult, SerializationResult};

#[expect(unused_imports, clippy::unused_trait_names)] // used in docstrings
use crate::{Archetype, ComponentBatch, LoggableBatch};

// ---

/// A [`Loggable`] represents a single instance in an array of loggable data.
///
/// Internally, Arrow, and by extension Rerun, only deal with arrays of data.
/// We refer to individual entries in these arrays as instances.
///
/// A [`Loggable`] has no semantics (such as a name, for example): it's just data.
/// If you want to encode semantics, then you're looking for a [`Component`], which extends [`Loggable`].
///
/// Implementing the [`Loggable`] trait automatically derives the [`LoggableBatch`] implementation,
/// which makes it possible to work with lists' worth of data in a generic fashion.
pub trait Loggable: 'static + Send + Sync + Clone + Sized + SizeBytes {
    /// The underlying [`arrow::datatypes::DataType`], excluding datatype extensions.
    fn arrow_datatype() -> arrow::datatypes::DataType;

    // Returns an empty Arrow array that matches this `Loggable`'s underlying datatype.
    #[inline]
    fn arrow_empty() -> arrow::array::ArrayRef {
        arrow::array::new_empty_array(&Self::arrow_datatype())
    }

    /// Given an iterator of owned or reference values to the current [`Loggable`], serializes
    /// them into an Arrow array.
    ///
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        Self::to_arrow_opt(data.into_iter().map(|v| Some(v)))
    }

    /// Given an iterator of options of owned or reference values to the current
    /// [`Loggable`], serializes them into an Arrow array.
    ///
    /// When using Rerun's builtin components & datatypes, this can only fail if the data
    /// exceeds the maximum number of entries in an Arrow array (2^31 for standard arrays,
    /// 2^63 for large arrays).
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a;

    /// Given an Arrow array, deserializes it into a collection of [`Loggable`]s.
    #[inline]
    fn from_arrow(data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>> {
        Self::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.ok_or_else(crate::DeserializationError::missing_data))
            .collect::<DeserializationResult<Vec<_>>>()
    }

    /// Given an Arrow array, deserializes it into a collection of optional [`Loggable`]s.
    #[inline]
    fn from_arrow_opt(
        data: &dyn arrow::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>> {
        Self::from_arrow(data).map(|v| v.into_iter().map(Some).collect())
    }

    /// Verifies that the given Arrow array can be deserialized into a collection of [`Self`]s.
    ///
    /// Calls [`Self::from_arrow`] and returns an error if it fails.
    fn verify_arrow_array(data: &dyn arrow::array::Array) -> crate::DeserializationResult<()> {
        Self::from_arrow(data).map(|_| ())
    }
}

/// A [`Component`] describes semantic data that can be used by any number of [`Archetype`]s.
///
/// Implementing the [`Component`] trait automatically derives the [`ComponentBatch`] implementation,
/// which makes it possible to work with lists' worth of data in a generic fashion.
pub trait Component: Loggable {
    /// Returns the complete [`ComponentDescriptor`] for this [`Component`].
    ///
    /// Every component is uniquely identified by its [`ComponentDescriptor`].
    //
    // NOTE: Builtin Rerun components don't (yet) have anything but a `ComponentName` attached to
    // them (other tags are injected at the Archetype level), therefore having a full
    // `ComponentDescriptor` might seem overkill.
    // It's not:
    // * Users might still want to register Components with specific tags.
    // * In the future, `ComponentDescriptor`s will very likely cover more than Archetype-related tags
    //   (e.g. generics, metric units, etc).
    // TODO(#6889): This method should be removed, or made dynamic.
    fn descriptor() -> ComponentDescriptor;

    /// The fully-qualified name of this component, e.g. `rerun.components.Position2D`.
    ///
    /// This is a trivial but useful helper for `Self::descriptor().component_name`.
    ///
    /// The default implementation already does the right thing: do not override unless you know
    /// what you're doing.
    /// `Self::name()` must exactly match the value returned by `Self::descriptor().component_name`,
    /// or undefined behavior ensues.
    //
    // TODO(#6889): This method should be removed.
    #[inline]
    fn name() -> ComponentName {
        Self::descriptor().component_name
    }
}

// ---

pub type UnorderedComponentDescriptorSet = IntSet<ComponentDescriptor>;

pub type ComponentDescriptorSet = std::collections::BTreeSet<ComponentDescriptor>;

re_string_interner::declare_new_type!(
    /// The fully-qualified name of a [`Component`], e.g. `rerun.components.Position2D`.
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct ComponentName;
);

// TODO(cmc): The only reason this exists is for convenience, and the only reason we need this
// convenience is because we're still in this weird half-way in-between state where some things
// are still indexed by name. Remove this entirely once we've ported everything to descriptors.
impl From<ComponentName> for Cow<'static, ComponentDescriptor> {
    #[inline]
    fn from(name: ComponentName) -> Self {
        name.sanity_check();
        Cow::Owned(ComponentDescriptor::new(name))
    }
}

// TODO(cmc): The only reason this exists is for convenience, and the only reason we need this
// convenience is because we're still in this weird half-way in-between state where some things
// are still indexed by name. Remove this entirely once we've ported everything to descriptors.
impl From<&ComponentName> for Cow<'static, ComponentDescriptor> {
    #[inline]
    fn from(name: &ComponentName) -> Self {
        name.sanity_check();
        Cow::Owned(ComponentDescriptor::new(*name))
    }
}

impl ComponentName {
    /// Runs some asserts in debug mode to make sure the name is not weird.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        let full_name = self.0.as_str();
        debug_assert!(
            !full_name.starts_with("rerun.components.rerun.components."),
            "DEBUG ASSERT: Found component with full name {full_name:?}. Maybe some bad round-tripping?"
        );
    }

    /// Returns the fully-qualified name, e.g. `rerun.components.Position2D`.
    ///
    /// This is the default `Display` implementation for [`ComponentName`].
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
    /// # use re_types_core::ComponentName;
    /// assert_eq!(ComponentName::from("rerun.components.Position2D").short_name(), "Position2D");
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

    /// Is this an indicator component for an archetype?
    pub fn is_indicator_component(&self) -> bool {
        self.ends_with("Indicator")
    }

    /// If this is an indicator component, for which archetype?
    pub fn indicator_component_archetype_short_name(&self) -> Option<String> {
        self.short_name()
            .strip_suffix("Indicator")
            .map(|name| name.to_owned())
    }

    /// Web URL to the Rerun documentation for this component.
    pub fn doc_url(&self) -> Option<String> {
        if let Some(archetype_name_pascal_case) = self.indicator_component_archetype_short_name() {
            // Link indicator components to their archetype.
            // This code should be correct as long as this url passes our link checker:
            // https://rerun.io/docs/reference/types/archetypes/line_strips3d

            let archetype_name_snake_case = re_case::to_snake_case(&archetype_name_pascal_case);
            let base_url = "https://rerun.io/docs/reference/types/archetypes";
            Some(format!("{base_url}/{archetype_name_snake_case}"))
        } else if let Some(component_name_pascal_case) =
            self.full_name().strip_prefix("rerun.components.")
        {
            // This code should be correct as long as this url passes our link checker:
            // https://rerun.io/docs/reference/types/components/line_strip2d

            let component_name_snake_case = re_case::to_snake_case(component_name_pascal_case);
            let base_url = "https://rerun.io/docs/reference/types/components";
            Some(format!("{base_url}/{component_name_snake_case}"))
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
}

// ---

impl re_byte_size::SizeBytes for ComponentName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

re_string_interner::declare_new_type!(
    /// The fully-qualified name of a [`Datatype`], e.g. `rerun.datatypes.Vec2D`.
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
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
    /// # use re_types_core::DatatypeName;
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

impl re_byte_size::SizeBytes for DatatypeName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}
