use crate::{
    ComponentList, ComponentName, DeserializationResult, ResultExt as _, SerializationResult,
    _Backtrace,
};

#[allow(unused_imports)] // used in docstrings
use crate::Component;

// ---

/// An archetype is a high-level construct that represents a set of [`Component`]s that usually
/// play well with each other (i.e. they compose nicely).
///
/// Internally, it is no different than a collection of components, but working at that higher
/// layer of abstraction opens opportunities for nicer APIs & tools that wouldn't be possible
/// otherwise.
///
/// E.g. consider the [`crate::archetypes::Points3D`] archetype, which represents the set of
/// components to consider when working with a 3D point cloud within Rerun.
///
/// ## Custom Archetypes
///
/// While, in most cases, archetypes are code generated from our [IDL definitions], it is possible to
/// manually extend existing archetypes, or even implement fully custom ones.
///
/// Most [`Archetype`] methods are optional to implement.
/// The most important method to implement is [`Archetype::as_component_lists`], which describes how
/// the archetype can be interpreted as a [`ComponentList`]: a set of components that are ready to
/// be serialized.
///
/// Have a look at our [Custom Data] example to learn more about handwritten archetypes.
///
/// [IDL definitions]: https://github.com/rerun-io/rerun/tree/latest/crates/re_types/definitions/rerun
/// [Custom Data]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data/src/main.rs
pub trait Archetype {
    /// The fully-qualified name of this archetype, e.g. `rerun.archetypes.Points2D`.
    fn name() -> ArchetypeName;

    // ---

    /// Returns the names of all components that _must_ be provided by the user when constructing
    /// this archetype.
    fn required_components() -> std::borrow::Cow<'static, [ComponentName]>;

    /// Returns the names of all components that _should_ be provided by the user when constructing
    /// this archetype.
    #[inline]
    fn recommended_components() -> std::borrow::Cow<'static, [ComponentName]> {
        std::borrow::Cow::Borrowed(&[])
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

    /// Returns the name of the associated indicator component, whose presence indicates that the
    /// high-level archetype-based APIs were used to log the data.
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
    #[inline]
    fn indicator_component() -> ComponentName {
        format!("rerun.components.{}Indicator", Self::name().short_name()).into()
    }

    /// Returns the number of instances of the archetype, i.e. the number of instances currently
    /// present in its required component(s).
    #[inline]
    fn num_instances(&self) -> usize {
        self.as_component_lists()
            .first()
            .map_or(0, |comp_list| comp_list.num_instances())
    }

    /// Exposes the archetype's contents as a set of [`ComponentList`]s.
    ///
    /// This is the main mechanism for easily extending builtin archetypes or even writing
    /// fully custom ones.
    /// Have a look at our [Custom Data] example to learn more about extending archetypes.
    ///
    /// [Custom Data]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data/src/main.rs
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    #[inline]
    fn as_component_lists(&self) -> Vec<&dyn ComponentList> {
        vec![]
    }

    // ---

    /// Serializes all non-null [`Component`]s of this [`Archetype`] into Arrow arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_lists`]
    /// as-is, which is what you want in 99.9% of cases.
    ///
    /// This can _never_ fail for Rerun's built-in archetypes.
    /// For the non-fallible version, see [`Archetype::to_arrow`].
    #[inline]
    fn try_to_arrow(
        &self,
    ) -> SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>>
    {
        self.as_component_lists()
            .into_iter()
            .map(|comp_list| {
                comp_list
                    .try_to_arrow()
                    .map(|array| (comp_list.arrow_field(), array))
                    .with_context(comp_list.name())
            })
            .collect()
    }

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

    // ---

    /// Given an iterator of Arrow arrays and their respective field metadata, deserializes them
    /// into this archetype.
    ///
    /// Arrow arrays that are unknown to this [`Archetype`] will simply be ignored and a warning
    /// logged to stderr.
    ///
    /// For the non-fallible version, see [`Archetype::from_arrow`].
    #[inline]
    fn try_from_arrow(
        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
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
    /// # use re_types::ArchetypeName;
    /// assert_eq!(ArchetypeName::from("rerun.archetypes.Points3D").short_name(), "Points3D");
    /// ```
    #[inline]
    pub fn short_name(&self) -> &'static str {
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.archetypes.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else {
            full_name
        }
    }
}
