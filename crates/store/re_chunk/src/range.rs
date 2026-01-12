use re_log_types::{AbsoluteTimeRange, TimeInt, TimelineName};
use re_types_core::ComponentIdentifier;

use crate::Chunk;

// --- Range ---

/// A query over a time range, for a given timeline.
///
/// Get all the data within this time interval, plus the latest one before the start of the
/// interval.
///
/// Motivation: all data is considered alive until the next logging to the same component path.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RangeQuery {
    pub timeline: TimelineName,
    pub range: AbsoluteTimeRange,
    pub options: RangeQueryOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeQueryOptions {
    /// Should the results contain all extra timeline information available in the [`Chunk`]?
    ///
    /// While this information can be useful in some cases, it comes at a performance cost.
    pub keep_extra_timelines: bool,

    /// Should the results contain all extra component information available in the [`Chunk`]?
    ///
    /// While this information can be useful in some cases, it comes at a performance cost.
    pub keep_extra_components: bool,

    /// If true, the results will include one extra tick on each side of the range.
    ///
    /// Note: this is different from simply subtracting/adding one to the time range of the query,
    /// as this will work even with non-contiguous time values, and even if these non-contiguous
    /// jumps happen across multiple chunks.
    ///
    /// Consider for example this data:
    /// ```text
    /// ┌──────────────────────────────────┬───────────────┬──────────────────────┐
    /// │ RowId                            ┆ frame_nr      ┆ Scalar               │
    /// ╞══════════════════════════════════╪═══════════════╪══════════════════════╡
    /// │ 17E9C11C655B21A9006568024DA10857 ┆ 0             ┆ [2]                  │
    /// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    /// │ 17E9C11C6560E8A6006568024DA10859 ┆ 2             ┆ [2.04]               │
    /// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    /// │ 17E9C11C656504F0006568024DA1085B ┆ 4             ┆ [2.08]               │
    /// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    /// │ 17E9C11C65693204006568024DA1085D ┆ 6             ┆ [2.12]               │
    /// └──────────────────────────────────┴───────────────┴──────────────────────┘
    /// ```
    ///
    /// * A `RangeQuery(#2, #4)` would yield frames #2 and #4.
    /// * A `RangeQuery(#1, #5)` would still only yield frames #2 and #4.
    /// * A `RangeQuery(#2, #4, include_extended_bounds=true)`, on the other hand, would yield all of
    ///   frames #0, #2, #4 and #6.
    pub include_extended_bounds: bool,
}

impl RangeQueryOptions {
    pub const DEFAULT: Self = Self {
        keep_extra_timelines: false,
        keep_extra_components: false,
        include_extended_bounds: false,
    };
}

impl Default for RangeQueryOptions {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Debug for RangeQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<ranging {:?}..={:?} on {:?} ([{}]keep_timelines [{}]keep_components [{}]extended_bounds)>",
            self.range.min(),
            self.range.max(),
            self.timeline,
            if self.options.keep_extra_timelines {
                "✓"
            } else {
                " "
            },
            if self.options.keep_extra_components {
                "✓"
            } else {
                " "
            },
            if self.options.include_extended_bounds {
                "✓"
            } else {
                " "
            },
        ))
    }
}

impl RangeQuery {
    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub const fn new(timeline: TimelineName, range: AbsoluteTimeRange) -> Self {
        Self {
            timeline,
            range,
            options: RangeQueryOptions::DEFAULT,
        }
    }

    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    ///
    /// Keeps all extra timelines and components around.
    #[inline]
    pub const fn with_extras(timeline: TimelineName, range: AbsoluteTimeRange) -> Self {
        Self {
            timeline,
            range,
            options: RangeQueryOptions {
                keep_extra_timelines: true,
                keep_extra_components: true,
                include_extended_bounds: false,
            },
        }
    }

    #[inline]
    pub const fn everything(timeline: TimelineName) -> Self {
        Self {
            timeline,
            range: AbsoluteTimeRange::EVERYTHING,
            options: RangeQueryOptions::DEFAULT,
        }
    }

    /// See [`RangeQueryOptions::keep_extra_timelines`] for more information.
    #[inline]
    pub fn keep_extra_timelines(mut self, toggle: bool) -> Self {
        self.options.keep_extra_timelines = toggle;
        self
    }

    /// See [`RangeQueryOptions::keep_extra_components`] for more information.
    #[inline]
    pub fn keep_extra_components(mut self, toggle: bool) -> Self {
        self.options.keep_extra_components = toggle;
        self
    }

    /// See [`RangeQueryOptions::include_extended_bounds`] for more information.
    #[inline]
    pub fn include_extended_bounds(mut self, toggle: bool) -> Self {
        self.options.include_extended_bounds = toggle;
        self
    }

    #[inline]
    pub fn timeline(&self) -> &TimelineName {
        &self.timeline
    }

    #[inline]
    pub fn range(&self) -> AbsoluteTimeRange {
        self.range
    }

    #[inline]
    pub fn options(&self) -> RangeQueryOptions {
        self.options.clone()
    }
}

// ---

impl Chunk {
    /// Runs a [`RangeQuery`] filter on a [`Chunk`].
    ///
    /// This behaves as a row-based filter: the result is a new [`Chunk`] that is vertically
    /// sliced, sorted and filtered in order to only contain the row(s) relevant for the
    /// specified `query`.
    ///
    /// The resulting [`Chunk`] is guaranteed to contain all the same columns has the queried
    /// chunk: there is no horizontal slicing going on.
    ///
    /// An empty [`Chunk`] (i.e. 0 rows, but N columns) is returned if the `query` yields nothing.
    ///
    /// Because the resulting chunk doesn't discard any column information, you can find extra relevant
    /// information by inspecting the data, for examples timestamps on other timelines.
    /// See [`Self::timeline_sliced`] and [`Self::component_sliced`] if you do want to filter this
    /// extra data.
    //
    // TODO(apache/arrow-rs#5375): Since we don't have access to arrow's ListView yet, we must actually clone the
    // data if the chunk requires sorting.
    pub fn range(&self, query: &RangeQuery, component: ComponentIdentifier) -> Self {
        if self.is_empty() {
            return self.clone();
        }

        re_tracing::profile_function!(format!("{query:?}"));

        let RangeQueryOptions {
            keep_extra_timelines,
            keep_extra_components,
            include_extended_bounds,
        } = query.options();

        // Pre-slice the data if the caller allowed us: this will make further slicing
        // (e.g. the range query itself) much cheaper to compute.
        use std::borrow::Cow;
        let chunk = if !keep_extra_timelines {
            Cow::Owned(self.timeline_sliced(*query.timeline()))
        } else {
            Cow::Borrowed(self)
        };
        let chunk = if !keep_extra_components {
            Cow::Owned(chunk.component_sliced(component))
        } else {
            chunk
        };

        if chunk.is_static() {
            // NOTE: A given component for a given entity can only have one static entry associated
            // with it, and this entry overrides everything else, which means it is functionally
            // equivalent to just running a latest-at query.
            chunk.latest_at(
                &crate::LatestAtQuery::new(*query.timeline(), TimeInt::MAX),
                component,
            )
        } else {
            let Some(is_sorted_by_time) = chunk
                .timelines
                .get(query.timeline())
                .map(|time_column| time_column.is_sorted())
            else {
                return chunk.emptied();
            };

            let chunk = chunk.densified(component);

            let chunk = if is_sorted_by_time {
                // Temporal, row-sorted, time-sorted chunk
                chunk
            } else {
                // Temporal, unsorted chunk
                chunk.sorted_by_timeline_if_unsorted(query.timeline())
            };

            let Some(times) = chunk
                .timelines
                .get(query.timeline())
                .map(|time_column| time_column.times_raw())
            else {
                return chunk.emptied();
            };

            let mut start_index =
                times.partition_point(|&time| time < query.range().min().as_i64());
            let mut end_index = times.partition_point(|&time| time <= query.range().max().as_i64());

            // See `RangeQueryOptions::include_extended_bounds` for more information.
            if include_extended_bounds {
                start_index = start_index.saturating_sub(1);
                end_index = usize::min(self.num_rows(), end_index.saturating_add(1));
            }

            chunk.row_sliced_shallow(start_index, end_index.saturating_sub(start_index))
        }
    }
}
