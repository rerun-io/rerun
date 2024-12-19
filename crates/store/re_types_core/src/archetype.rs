use std::borrow::Cow;

use crate::{
    ComponentBatch, ComponentBatchCowWithDescriptor, ComponentDescriptor, ComponentName,
    DeserializationResult, SerializationResult, _Backtrace,
};

#[allow(unused_imports)] // used in docstrings
use crate::{Component, Loggable, LoggableBatch};

// ---

/// An archetype is a high-level construct that represents a set of [`Component`]s that usually
/// play well with each other (i.e. they compose nicely).
///
/// Internally, it is no different than a collection of components, but working at that higher
/// layer of abstraction opens opportunities for nicer APIs & tools that wouldn't be possible
/// otherwise.
///
/// E.g. consider the `crate::archetypes::Points3D` archetype, which represents the set of
/// components to consider when working with a 3D point cloud within Rerun.
pub trait Archetype {
    /// The associated indicator component, whose presence indicates that the high-level
    /// archetype-based APIs were used to log the data.
    ///
    /// ## Internal representation
    ///
    /// Indicator components are always unit-length null arrays.
    /// Their names follow the pattern `rerun.components.{ArchetypeName}Indicator`, e.g.
    /// `rerun.components.Points3DIndicator`.
    ///
    /// Since null arrays aren't actually arrays and we don't actually have any data to shuffle
    /// around per-se, we can't implement the usual [`Loggable`] traits.
    /// For this reason, indicator components directly implement [`LoggableBatch`] instead, and
    /// bypass the entire iterator machinery.
    //
    // TODO(rust-lang/rust#29661): We'd like to just default this to the right thing which is
    // pretty much always `A::Indicator`, but defaults are unstable.
    // type Indicator: ComponentBatch = A::Indicator;
    type Indicator: 'static + ComponentBatch + Default;

    /// The fully-qualified name of this archetype, e.g. `rerun.archetypes.Points2D`.
    fn name() -> ArchetypeName;

    /// Readable name for displaying in UI.
    fn display_name() -> &'static str;

    // ---

    /// Creates a [`ComponentBatch`] out of the associated [`Self::Indicator`] component.
    ///
    /// This allows for associating arbitrary indicator components with arbitrary data.
    /// Check out the `manual_indicator` API example to see what's possible.
    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        ComponentBatchCowWithDescriptor::new(
            Box::<<Self as Archetype>::Indicator>::default() as Box<dyn ComponentBatch>
        )
    }

    /// Returns all component descriptors that _must_ be provided by the user when constructing this archetype.
    fn required_components() -> std::borrow::Cow<'static, [ComponentDescriptor]>;

    /// Returns all component descriptors that _should_ be provided by the user when constructing this archetype.
    #[inline]
    fn recommended_components() -> std::borrow::Cow<'static, [ComponentDescriptor]> {
        std::borrow::Cow::Owned(vec![Self::indicator().descriptor().into_owned()])
    }

    /// Returns all component descriptors that _may_ be provided by the user when constructing this archetype.
    #[inline]
    fn optional_components() -> std::borrow::Cow<'static, [ComponentDescriptor]> {
        std::borrow::Cow::Borrowed(&[])
    }

    /// Returns all component descriptors that must, should and may be provided by the user when constructing
    /// this archetype.
    ///
    /// The default implementation always does the right thing, at the cost of some runtime
    /// allocations.
    /// If you know all your component descriptors statically, you can override this method to get rid of the
    /// extra allocations.
    #[inline]
    fn all_components() -> std::borrow::Cow<'static, [ComponentDescriptor]> {
        [
            Self::required_components().into_owned(),
            Self::recommended_components().into_owned(),
            Self::optional_components().into_owned(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .into()
    }

    // ---

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow(
        data: impl IntoIterator<Item = (arrow2::datatypes::Field, ::arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        Self::from_arrow_components(
            data.into_iter()
                .map(|(field, array)| (field.name.into(), array)),
        )
    }

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow2(
        data: impl IntoIterator<Item = (arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        Self::from_arrow2_components(
            data.into_iter()
                .map(|(field, array)| (field.name.into(), array)),
        )
    }

    /// Given an iterator of Arrow arrays and their respective `ComponentNames`, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow_components(
        data: impl IntoIterator<Item = (ComponentName, ::arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        _ = data; // NOTE: do this here to avoid breaking users' autocomplete snippets
        Err(crate::DeserializationError::NotImplemented {
            fqname: Self::name().to_string(),
            backtrace: _Backtrace::new_unresolved(),
        })
    }

    /// Given an iterator of Arrow arrays and their respective `ComponentNames`, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow2_components(
        data: impl IntoIterator<Item = (ComponentName, Box<dyn ::arrow2::array::Array>)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        Self::from_arrow_components(data.into_iter().map(|(component, arrow2_array)| {
            (component, arrow::array::ArrayRef::from(arrow2_array))
        }))
    }
}

/// Indicates that the archetype has reflection data available for it.
pub trait ArchetypeReflectionMarker {}

// ---

re_string_interner::declare_new_type!(
    /// The fully-qualified name of an [`Archetype`], e.g. `rerun.archetypes.Points3D`.
    pub struct ArchetypeName;
);

impl ArchetypeName {
    /// Returns the fully-qualified name, e.g. `rerun.archetypes.Points3D`.
    ///
    /// This is the default `Display` implementation for [`ArchetypeName`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.0.as_str()
    }

    /// Returns the unqualified name, e.g. `Points3D`.
    ///
    /// Used for most UI elements.
    ///
    /// ```
    /// # use re_types_core::ArchetypeName;
    /// assert_eq!(ArchetypeName::from("rerun.archetypes.Points3D").short_name(), "Points3D");
    /// ```
    #[inline]
    pub fn short_name(&self) -> &'static str {
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.archetypes.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.blueprint.archetypes.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else {
            full_name
        }
    }
}

impl re_byte_size::SizeBytes for ArchetypeName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

// ---

re_string_interner::declare_new_type!(
    /// The name of an [`Archetype`]'s field, e.g. `positions`.
    pub struct ArchetypeFieldName;
);

impl re_byte_size::SizeBytes for ArchetypeFieldName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

// ---

/// A generic [indicator component] that can be specialized for any [`Archetype`].
///
/// ```ignore
/// type MyArchetypeIndicator = GenericIndicatorComponent<MyArchetype>;
/// ```
///
/// [indicator component]: [`Archetype::Indicator`]
#[derive(Debug, Clone, Copy)]
pub struct GenericIndicatorComponent<A: Archetype> {
    _phantom: std::marker::PhantomData<A>,
}

impl<A: Archetype> GenericIndicatorComponent<A> {
    pub const DEFAULT: Self = Self {
        _phantom: std::marker::PhantomData::<A>,
    };

    /// Create an array of indicator components of this type with the given length.
    ///
    /// This can be useful when sending columns of indicators with
    /// `rerun::RecordingStream::send_columns`.
    #[inline]
    pub fn new_array(len: usize) -> GenericIndicatorComponentArray<A> {
        GenericIndicatorComponentArray {
            len,
            _phantom: std::marker::PhantomData::<A>,
        }
    }
}

impl<A: Archetype> Default for GenericIndicatorComponent<A> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<A: Archetype> crate::LoggableBatch for GenericIndicatorComponent<A> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn arrow2::array::Array>> {
        let datatype = arrow2::datatypes::DataType::Null;
        Ok(arrow2::array::NullArray::new(datatype, 1).boxed())
    }
}

impl<A: Archetype> crate::ComponentBatch for GenericIndicatorComponent<A> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        let component_name =
            format!("{}Indicator", A::name().full_name()).replace("archetypes", "components");
        ComponentDescriptor::new(component_name).into()
    }
}

/// A generic [indicator component] array of a given length.
///
/// This can be useful when sending columns of indicators with
/// `rerun::RecordingStream::send_columns`.
///
/// To create this type, call [`GenericIndicatorComponent::new_array`].
///
/// [indicator component]: [`Archetype::Indicator`]
#[derive(Debug, Clone, Copy)]
pub struct GenericIndicatorComponentArray<A: Archetype> {
    len: usize,
    _phantom: std::marker::PhantomData<A>,
}

impl<A: Archetype> crate::LoggableBatch for GenericIndicatorComponentArray<A> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn arrow2::array::Array>> {
        let datatype = arrow2::datatypes::DataType::Null;
        Ok(arrow2::array::NullArray::new(datatype, self.len).boxed())
    }
}

impl<A: Archetype> crate::ComponentBatch for GenericIndicatorComponentArray<A> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        ComponentDescriptor::new(GenericIndicatorComponent::<A>::DEFAULT.name()).into()
    }
}

// ---

/// An arbitrary named [indicator component].
///
/// [indicator component]: [`Archetype::Indicator`]
#[derive(Debug, Clone, Copy)]
pub struct NamedIndicatorComponent(pub ComponentName);

impl NamedIndicatorComponent {
    #[inline]
    pub fn as_batch(&self) -> ComponentBatchCowWithDescriptor<'_> {
        ComponentBatchCowWithDescriptor::new(self as &dyn crate::ComponentBatch)
    }

    #[inline]
    pub fn to_batch(self) -> ComponentBatchCowWithDescriptor<'static> {
        ComponentBatchCowWithDescriptor::new(Box::new(self) as Box<dyn crate::ComponentBatch>)
    }
}

impl crate::LoggableBatch for NamedIndicatorComponent {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn arrow2::array::Array>> {
        let datatype = arrow2::datatypes::DataType::Null;
        Ok(arrow2::array::NullArray::new(datatype, 1).boxed())
    }
}

impl crate::ComponentBatch for NamedIndicatorComponent {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        ComponentDescriptor::new(self.0).into()
    }
}
