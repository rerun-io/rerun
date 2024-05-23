// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/visual_bounds2d.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Controls the visual bounds of a 2D view.
///
/// Everything within these bounds are guaranteed to be visible.
/// Somethings outside of these bounds may also be visible due to letterboxing.
///
/// If no visual bounds are set, it will be determined automatically,
/// based on the bounding-box of the data or other camera information present in the view.
#[derive(Clone, Debug, Copy)]
pub struct VisualBounds2D {
    /// Controls the visible range of a 2D view.
    ///
    /// Use this to control pan & zoom of the view.
    pub range: crate::blueprint::components::VisualBounds2D,
}

impl ::re_types_core::SizeBytes for VisualBounds2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.range.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::components::VisualBounds2D>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.VisualBounds2D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.VisualBounds2DIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.VisualBounds2D".into(),
            "rerun.blueprint.components.VisualBounds2DIndicator".into(),
        ]
    });

impl VisualBounds2D {
    /// The total number of components in the archetype: 1 required, 1 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`VisualBounds2D`] [`::re_types_core::Archetype`]
pub type VisualBounds2DIndicator = ::re_types_core::GenericIndicatorComponent<VisualBounds2D>;

impl ::re_types_core::Archetype for VisualBounds2D {
    type Indicator = VisualBounds2DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.VisualBounds2D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Visual bounds 2D"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: VisualBounds2DIndicator = VisualBounds2DIndicator::DEFAULT;
        MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let range = {
            let array = arrays_by_name
                .get("rerun.blueprint.components.VisualBounds2D")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.VisualBounds2D#range")?;
            <crate::blueprint::components::VisualBounds2D>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.VisualBounds2D#range")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.VisualBounds2D#range")?
        };
        Ok(Self { range })
    }
}

impl ::re_types_core::AsComponents for VisualBounds2D {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.range as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl VisualBounds2D {
    /// Create a new `VisualBounds2D`.
    #[inline]
    pub fn new(range: impl Into<crate::blueprint::components::VisualBounds2D>) -> Self {
        Self {
            range: range.into(),
        }
    }
}
