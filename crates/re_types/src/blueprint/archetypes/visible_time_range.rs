// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/visible_time_range.fbs".

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

/// **Archetype**: Configures what range of the timeline is shown on a view.
///
/// Whenever no visual time range applies, queries are done with "latest at" semantics.
/// This means that the view will, starting from the time cursor position,
/// query the latest data available for each component type.
///
/// The default visual time range depends on the type of view this property applies to:
/// - For time series views, the default is to show the entire timeline.
/// - For any other view, the default is to apply latest-at semantics.
///
/// The visual time range can be overridden also individually per entity.
#[derive(Clone, Debug, Default)]
pub struct VisibleTimeRange {
    /// The range of time to show for timelines based on sequence numbers.
    pub sequence: Option<crate::blueprint::components::VisibleTimeRangeSequence>,

    /// The range of time to show for timelines based on time.
    pub time: Option<crate::blueprint::components::VisibleTimeRangeTime>,
}

impl ::re_types_core::SizeBytes for VisibleTimeRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.sequence.heap_size_bytes() + self.time.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::blueprint::components::VisibleTimeRangeSequence>>::is_pod()
            && <Option<crate::blueprint::components::VisibleTimeRangeTime>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.VisibleTimeRangeIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.VisibleTimeRangeSequence".into(),
            "rerun.blueprint.components.VisibleTimeRangeTime".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.VisibleTimeRangeIndicator".into(),
            "rerun.blueprint.components.VisibleTimeRangeSequence".into(),
            "rerun.blueprint.components.VisibleTimeRangeTime".into(),
        ]
    });

impl VisibleTimeRange {
    /// The total number of components in the archetype: 0 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`VisibleTimeRange`] [`::re_types_core::Archetype`]
pub type VisibleTimeRangeIndicator = ::re_types_core::GenericIndicatorComponent<VisibleTimeRange>;

impl ::re_types_core::Archetype for VisibleTimeRange {
    type Indicator = VisibleTimeRangeIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.VisibleTimeRange".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: VisibleTimeRangeIndicator = VisibleTimeRangeIndicator::DEFAULT;
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
        let sequence = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.VisibleTimeRangeSequence")
        {
            <crate::blueprint::components::VisibleTimeRangeSequence>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.VisibleTimeRange#sequence")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let time = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.VisibleTimeRangeTime")
        {
            <crate::blueprint::components::VisibleTimeRangeTime>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.VisibleTimeRange#time")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { sequence, time })
    }
}

impl ::re_types_core::AsComponents for VisibleTimeRange {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.sequence
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.time
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl VisibleTimeRange {
    /// Create a new `VisibleTimeRange`.
    #[inline]
    pub fn new() -> Self {
        Self {
            sequence: None,
            time: None,
        }
    }

    /// The range of time to show for timelines based on sequence numbers.
    #[inline]
    pub fn with_sequence(
        mut self,
        sequence: impl Into<crate::blueprint::components::VisibleTimeRangeSequence>,
    ) -> Self {
        self.sequence = Some(sequence.into());
        self
    }

    /// The range of time to show for timelines based on time.
    #[inline]
    pub fn with_time(
        mut self,
        time: impl Into<crate::blueprint::components::VisibleTimeRangeTime>,
    ) -> Self {
        self.time = Some(time.into());
        self
    }
}
