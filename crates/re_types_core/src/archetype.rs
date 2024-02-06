use std::sync::Arc;

use crate::{
    ComponentBatch, ComponentName, DeserializationResult, MaybeOwnedComponentBatch,
    SerializationResult, _Backtrace,
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
    /// Indicator components are always-splatted null arrays.
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

    // ---

    // TODO(cmc): Should we also generate and return static IntSets?

    /// Creates a [`ComponentBatch`] out of the associated [`Self::Indicator`] component.
    ///
    /// This allows for associating arbitrary indicator components with arbitrary data.
    /// Check out the `manual_indicator` API example to see what's possible.
    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        MaybeOwnedComponentBatch::Owned(Box::<<Self as Archetype>::Indicator>::default())
    }

    /// Returns the names of all components that _must_ be provided by the user when constructing
    /// this archetype.
    fn required_components() -> std::borrow::Cow<'static, [ComponentName]>;

    /// Returns the names of all components that _should_ be provided by the user when constructing
    /// this archetype.
    #[inline]
    fn recommended_components() -> std::borrow::Cow<'static, [ComponentName]> {
        std::borrow::Cow::Owned(vec![Self::indicator().name()])
    }

    /// Returns the names of all components that _may_ be provided by the user when constructing
    /// this archetype.
    #[inline]
    fn optional_components() -> std::borrow::Cow<'static, [ComponentName]> {
        std::borrow::Cow::Borrowed(&[])
    }

    /// Returns the names of all components that must, should and may be provided by the user when
    /// constructing this archetype.
    ///
    /// The default implementation always does the right thing, at the cost of some runtime
    /// allocations.
    /// If you know all your components statically, you can override this method to get rid of the
    /// extra allocations.
    #[inline]
    fn all_components() -> std::borrow::Cow<'static, [ComponentName]> {
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
        data: impl IntoIterator<Item = (arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        Self::from_arrow_components(
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
        data: impl IntoIterator<Item = (ComponentName, Box<dyn ::arrow2::array::Array>)>,
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
}

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
}

impl<A: Archetype> Default for GenericIndicatorComponent<A> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<A: Archetype> crate::LoggableBatch for GenericIndicatorComponent<A> {
    type Name = ComponentName;

    #[inline]
    fn name(&self) -> Self::Name {
        format!("{}Indicator", A::name().full_name())
            .replace("archetypes", "components")
            .into()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        let name = self.name().to_string();
        arrow2::datatypes::Field::new(
            name.clone(),
            arrow2::datatypes::DataType::Extension(
                name,
                Arc::new(arrow2::datatypes::DataType::Null),
                None,
            ),
            false,
        )
    }

    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn arrow2::array::Array>> {
        Ok(
            arrow2::array::NullArray::new(arrow2::datatypes::DataType::Null, self.num_instances())
                .boxed(),
        )
    }
}

impl<A: Archetype> crate::ComponentBatch for GenericIndicatorComponent<A> {}

// ---

/// An arbitrary named [indicator component].
///
/// [indicator component]: [`Archetype::Indicator`]
#[derive(Debug, Clone, Copy)]
pub struct NamedIndicatorComponent(pub ComponentName);

impl NamedIndicatorComponent {
    #[inline]
    pub fn as_batch(&self) -> MaybeOwnedComponentBatch<'_> {
        MaybeOwnedComponentBatch::Ref(self)
    }

    #[inline]
    pub fn to_batch(self) -> MaybeOwnedComponentBatch<'static> {
        MaybeOwnedComponentBatch::Owned(Box::new(self))
    }
}

impl crate::LoggableBatch for NamedIndicatorComponent {
    type Name = ComponentName;

    #[inline]
    fn name(&self) -> Self::Name {
        self.0
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        let name = self.name().to_string();
        arrow2::datatypes::Field::new(
            name.clone(),
            arrow2::datatypes::DataType::Extension(
                name,
                Arc::new(arrow2::datatypes::DataType::Null),
                None,
            ),
            false,
        )
    }

    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn arrow2::array::Array>> {
        Ok(
            arrow2::array::NullArray::new(arrow2::datatypes::DataType::Null, self.num_instances())
                .boxed(),
        )
    }
}

impl crate::ComponentBatch for NamedIndicatorComponent {}
