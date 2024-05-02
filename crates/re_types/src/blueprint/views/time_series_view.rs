// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/views/time_series.fbs".

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

/// **View**: A time series view.
#[derive(Clone, Debug)]
pub struct TimeSeriesView {
    /// Configures the vertical axis of the plot.
    pub axis_y: crate::blueprint::archetypes::ScalarAxis,

    /// Configures the legend of the plot.
    pub plot_legend: crate::blueprint::archetypes::PlotLegend,

    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    pub time_ranges: crate::blueprint::archetypes::VisibleTimeRanges,
}

impl ::re_types_core::SizeBytes for TimeSeriesView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.axis_y.heap_size_bytes()
            + self.plot_legend.heap_size_bytes()
            + self.time_ranges.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::ScalarAxis>::is_pod()
            && <crate::blueprint::archetypes::PlotLegend>::is_pod()
            && <crate::blueprint::archetypes::VisibleTimeRanges>::is_pod()
    }
}

impl ::re_types_core::View for TimeSeriesView {
    #[inline]
    fn identifier() -> ::re_types_core::SpaceViewClassIdentifier {
        "TimeSeries".into()
    }
}
