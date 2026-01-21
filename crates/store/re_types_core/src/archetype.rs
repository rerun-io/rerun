#[expect(unused_imports, clippy::unused_trait_names)] // used in docstrings
use crate::{Component, Loggable};
use crate::{ComponentDescriptor, DeserializationResult};

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
    /// The fully-qualified name of this archetype, e.g. `rerun.archetypes.Points2D`.
    fn name() -> ArchetypeName;

    /// Readable name for displaying in UI.
    fn display_name() -> &'static str;

    // ---

    /// Returns all component descriptors that _must_ be provided by the user when constructing this archetype.
    fn required_components() -> std::borrow::Cow<'static, [ComponentDescriptor]>;

    /// Returns all component descriptors that _should_ be provided by the user when constructing this archetype.
    #[inline]
    fn recommended_components() -> std::borrow::Cow<'static, [ComponentDescriptor]> {
        // TODO(#10512): Maybe add the "marker" component back here?
        std::borrow::Cow::Owned(vec![])
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

    /// Utility method based on [`Self::all_components`] to return all component identifiers.
    #[inline]
    fn all_component_identifiers() -> impl Iterator<Item = ComponentIdentifier> {
        match Self::all_components() {
            // Need to resolve the Cow to work around borrow checker not being able to take ownership of it otherwise.
            std::borrow::Cow::Borrowed(components) => {
                itertools::Either::Left(components.iter().map(|c| c.component))
            }

            std::borrow::Cow::Owned(components) => {
                itertools::Either::Right(components.into_iter().map(|c| c.component))
            }
        }
    }

    // ---

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow(
        data: impl IntoIterator<Item = (arrow::datatypes::Field, ::arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        Self::from_arrow_components(
            data.into_iter()
                .map(|(field, array)| (ComponentDescriptor::from(field), array)),
        )
    }

    /// Given an iterator of Arrow arrays and their respective [`ComponentDescriptor`]s, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    #[inline]
    fn from_arrow_components(
        data: impl IntoIterator<Item = (ComponentDescriptor, ::arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self>
    where
        Self: Sized,
    {
        _ = data; // NOTE: do this here to avoid breaking users' autocomplete snippets
        Err(crate::DeserializationError::NotImplemented {
            fqname: Self::name().to_string(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        })
    }
}

/// Indicates that the archetype has reflection data available for it.
pub trait ArchetypeReflectionMarker {}

// ---

re_string_interner::declare_new_type!(
    /// The fully-qualified name of an [`Archetype`], e.g. `rerun.archetypes.Points3D`.
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct ArchetypeName;
);

impl ArchetypeName {
    /// Runs some asserts in debug mode to make sure the name is not weird.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        let full_name = self.0.as_str();
        debug_assert!(
            !full_name.starts_with("rerun.archetypes.rerun.archetypes."),
            "DEBUG ASSERT: Found archetype with full name {full_name:?}. Maybe some bad round-tripping?"
        );
    }

    /// Returns the fully-qualified name, e.g. `rerun.archetypes.Points3D`.
    ///
    /// This is the default `Display` implementation for [`ArchetypeName`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.sanity_check();
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
        self.sanity_check();
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

    /// Url to the rerun docs for this Rerun archetype.
    pub fn doc_url(&self) -> Option<String> {
        // This code should be correct as long as this url passes our link checker:
        // https://rerun.io/docs/reference/types/archetypes/line_strips3d
        let short_name_pascal_case = self.full_name().strip_prefix("rerun.archetypes.")?;
        let archetype_name_snake_case = re_case::to_snake_case(short_name_pascal_case);
        let base_url = "https://rerun.io/docs/reference/types/archetypes";
        Some(format!("{base_url}/{archetype_name_snake_case}"))
    }
}

impl re_byte_size::SizeBytes for ArchetypeName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

// ---

re_string_interner::declare_new_type!(
    /// An identifier for a component, i.e. a field in an [`Archetype`].
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct ComponentIdentifier;
);

impl re_byte_size::SizeBytes for ComponentIdentifier {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}
