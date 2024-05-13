// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/visible_time_ranges.fbs".

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

/// **Archetype**: Configures what range of each timeline is shown on a view.
///
/// Whenever no visual time range applies, queries are done with "latest at" semantics.
/// This means that the view will, starting from the time cursor position,
/// query the latest data available for each component type.
///
/// The default visual time range depends on the type of view this property applies to:
/// - For time series views, the default is to show the entire timeline.
/// - For any other view, the default is to apply latest-at semantics.
#[derive(Clone, Debug, Default)]
pub struct VisibleTimeRanges {
    /// The time ranges to show for each timeline unless specified otherwise on a per-entity basis.
    ///
    /// If a timeline is specified more than once, the first entry will be used.
    pub ranges: Vec<crate::blueprint::components::VisibleTimeRange>,
}

impl ::re_types_core::SizeBytes for VisibleTimeRanges {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.ranges.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::blueprint::components::VisibleTimeRange>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.VisibleTimeRange".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.VisibleTimeRangesIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.VisibleTimeRange".into(),
            "rerun.blueprint.components.VisibleTimeRangesIndicator".into(),
        ]
    });

impl VisibleTimeRanges {
    /// The total number of components in the archetype: 1 required, 1 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`VisibleTimeRanges`] [`::re_types_core::Archetype`]
pub type VisibleTimeRangesIndicator = ::re_types_core::GenericIndicatorComponent<VisibleTimeRanges>;

impl ::re_types_core::Archetype for VisibleTimeRanges {
    type Indicator = VisibleTimeRangesIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.VisibleTimeRanges".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: VisibleTimeRangesIndicator = VisibleTimeRangesIndicator::DEFAULT;
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
        let ranges = {
            let array = arrays_by_name
                .get("rerun.blueprint.components.VisibleTimeRange")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.VisibleTimeRanges#ranges")?;
            <crate::blueprint::components::VisibleTimeRange>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.VisibleTimeRanges#ranges")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.archetypes.VisibleTimeRanges#ranges")?
        };
        Ok(Self { ranges })
    }
}

impl ::re_types_core::AsComponents for VisibleTimeRanges {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.ranges as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl VisibleTimeRanges {
    /// Create a new `VisibleTimeRanges`.
    #[inline]
    pub fn new(
        ranges: impl IntoIterator<Item = impl Into<crate::blueprint::components::VisibleTimeRange>>,
    ) -> Self {
        Self {
            ranges: ranges.into_iter().map(Into::into).collect(),
        }
    }
}
