// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/dataframe_query.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: The query for the dataframe view.
#[derive(Clone, Debug)]
pub struct DataframeQuery {
    /// The timeline for this query.
    ///
    /// If unset, use the time panel's timeline and a latest-at query, ignoring all other components of this archetype.
    pub timeline: Option<crate::blueprint::components::TimelineName>,

    /// Kind of query: latest-at or range.
    pub kind: Option<crate::blueprint::components::QueryKind>,

    /// Configuration for latest-at queries.
    ///
    /// Note: configuration as saved on a per-timeline basis.
    pub latest_at_queries: Option<crate::blueprint::components::LatestAtQueries>,

    /// Configuration for the time range queries.
    ///
    /// Note: configuration as saved on a per-timeline basis.
    pub time_range_queries: Option<crate::blueprint::components::TimeRangeQueries>,
}

impl ::re_types_core::SizeBytes for DataframeQuery {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.timeline.heap_size_bytes()
            + self.kind.heap_size_bytes()
            + self.latest_at_queries.heap_size_bytes()
            + self.time_range_queries.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::blueprint::components::TimelineName>>::is_pod()
            && <Option<crate::blueprint::components::QueryKind>>::is_pod()
            && <Option<crate::blueprint::components::LatestAtQueries>>::is_pod()
            && <Option<crate::blueprint::components::TimeRangeQueries>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.DataframeQueryIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.TimelineName".into(),
            "rerun.blueprint.components.QueryKind".into(),
            "rerun.blueprint.components.LatestAtQueries".into(),
            "rerun.blueprint.components.TimeRangeQueries".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.DataframeQueryIndicator".into(),
            "rerun.blueprint.components.TimelineName".into(),
            "rerun.blueprint.components.QueryKind".into(),
            "rerun.blueprint.components.LatestAtQueries".into(),
            "rerun.blueprint.components.TimeRangeQueries".into(),
        ]
    });

impl DataframeQuery {
    /// The total number of components in the archetype: 0 required, 1 recommended, 4 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`DataframeQuery`] [`::re_types_core::Archetype`]
pub type DataframeQueryIndicator = ::re_types_core::GenericIndicatorComponent<DataframeQuery>;

impl ::re_types_core::Archetype for DataframeQuery {
    type Indicator = DataframeQueryIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.DataframeQuery".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Dataframe query"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: DataframeQueryIndicator = DataframeQueryIndicator::DEFAULT;
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
        let timeline =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.TimelineName") {
                <crate::blueprint::components::TimelineName>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.DataframeQuery#timeline")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let kind = if let Some(array) = arrays_by_name.get("rerun.blueprint.components.QueryKind") {
            <crate::blueprint::components::QueryKind>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.DataframeQuery#kind")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let latest_at_queries =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.LatestAtQueries") {
                <crate::blueprint::components::LatestAtQueries>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.DataframeQuery#latest_at_queries")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let time_range_queries = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.TimeRangeQueries")
        {
            <crate::blueprint::components::TimeRangeQueries>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.DataframeQuery#time_range_queries")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self {
            timeline,
            kind,
            latest_at_queries,
            time_range_queries,
        })
    }
}

impl ::re_types_core::AsComponents for DataframeQuery {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.timeline
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.kind
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.latest_at_queries
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.time_range_queries
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for DataframeQuery {}

impl DataframeQuery {
    /// Create a new `DataframeQuery`.
    #[inline]
    pub fn new() -> Self {
        Self {
            timeline: None,
            kind: None,
            latest_at_queries: None,
            time_range_queries: None,
        }
    }

    /// The timeline for this query.
    ///
    /// If unset, use the time panel's timeline and a latest-at query, ignoring all other components of this archetype.
    #[inline]
    pub fn with_timeline(
        mut self,
        timeline: impl Into<crate::blueprint::components::TimelineName>,
    ) -> Self {
        self.timeline = Some(timeline.into());
        self
    }

    /// Kind of query: latest-at or range.
    #[inline]
    pub fn with_kind(mut self, kind: impl Into<crate::blueprint::components::QueryKind>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// Configuration for latest-at queries.
    ///
    /// Note: configuration as saved on a per-timeline basis.
    #[inline]
    pub fn with_latest_at_queries(
        mut self,
        latest_at_queries: impl Into<crate::blueprint::components::LatestAtQueries>,
    ) -> Self {
        self.latest_at_queries = Some(latest_at_queries.into());
        self
    }

    /// Configuration for the time range queries.
    ///
    /// Note: configuration as saved on a per-timeline basis.
    #[inline]
    pub fn with_time_range_queries(
        mut self,
        time_range_queries: impl Into<crate::blueprint::components::TimeRangeQueries>,
    ) -> Self {
        self.time_range_queries = Some(time_range_queries.into());
        self
    }
}
