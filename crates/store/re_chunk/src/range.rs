use re_log_types::{ResolvedTimeRange, TimeInt, Timeline};
use re_types_core::ComponentName;

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
    pub timeline: Timeline,
    pub range: ResolvedTimeRange,
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
}

impl RangeQueryOptions {
    pub const DEFAULT: Self = Self {
        keep_extra_timelines: false,
        keep_extra_components: false,
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
            "<ranging {}..={} on {:?} ([{}]keep_timelines [{}]keep_components)>",
            self.timeline.typ().format_utc(self.range.min()),
            self.timeline.typ().format_utc(self.range.max()),
            self.timeline.name(),
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
        ))
    }
}

impl RangeQuery {
    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub const fn new(timeline: Timeline, range: ResolvedTimeRange) -> Self {
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
    pub const fn with_extras(timeline: Timeline, range: ResolvedTimeRange) -> Self {
        Self {
            timeline,
            range,
            options: RangeQueryOptions {
                keep_extra_timelines: true,
                keep_extra_components: true,
            },
        }
    }

    #[inline]
    pub const fn everything(timeline: Timeline) -> Self {
        Self {
            timeline,
            range: ResolvedTimeRange::EVERYTHING,
            options: RangeQueryOptions::DEFAULT,
        }
    }

    /// Should the results contain all extra timeline information available in the [`Chunk`]?
    ///
    /// While this information can be useful in some cases, it comes at a performance cost.
    #[inline]
    pub fn keep_extra_timelines(mut self, toggle: bool) -> Self {
        self.options.keep_extra_timelines = toggle;
        self
    }

    /// Should the results contain all extra component information available in the [`Chunk`]?
    ///
    /// While this information can be useful in some cases, it comes at a performance cost.
    #[inline]
    pub fn keep_extra_components(mut self, toggle: bool) -> Self {
        self.options.keep_extra_components = toggle;
        self
    }

    #[inline]
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    #[inline]
    pub fn range(&self) -> ResolvedTimeRange {
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
    // TODO(#3741): Since we don't have access to arrow's ListView yet, we must actually clone the
    // data if the chunk requires sorting.
    pub fn range(&self, query: &RangeQuery, component_name: ComponentName) -> Self {
        if self.is_empty() {
            return self.clone();
        }

        re_tracing::profile_function!(format!("{query:?}"));

        let RangeQueryOptions {
            keep_extra_timelines,
            keep_extra_components,
        } = query.options();

        // Pre-slice the data if the caller allowed us: this will make further slicing
        // (e.g. the range query itself) much cheaper to compute.
        use std::borrow::Cow;
        let chunk = if !keep_extra_timelines {
            Cow::Owned(self.timeline_sliced(query.timeline()))
        } else {
            Cow::Borrowed(self)
        };
        let chunk = if !keep_extra_components {
            Cow::Owned(chunk.component_sliced(component_name))
        } else {
            chunk
        };

        if chunk.is_static() {
            // NOTE: A given component for a given entity can only have one static entry associated
            // with it, and this entry overrides everything else, which means it is functionally
            // equivalent to just running a latest-at query.
            chunk.latest_at(
                &crate::LatestAtQuery::new(query.timeline(), TimeInt::MAX),
                component_name,
            )
        } else {
            let Some(is_sorted_by_time) = chunk
                .timelines
                .get(&query.timeline())
                .map(|time_column| time_column.is_sorted())
            else {
                return chunk.emptied();
            };

            let chunk = chunk.densified(component_name);

            let chunk = if is_sorted_by_time {
                // Temporal, row-sorted, time-sorted chunk
                chunk
            } else {
                // Temporal, unsorted chunk
                chunk.sorted_by_timeline_if_unsorted(&query.timeline())
            };

            let Some(times) = chunk
                .timelines
                .get(&query.timeline())
                .map(|time_column| time_column.times_raw())
            else {
                return chunk.emptied();
            };

            let start_index = times.partition_point(|&time| time < query.range().min().as_i64());
            let end_index = times.partition_point(|&time| time <= query.range().max().as_i64());

            chunk.row_sliced(start_index, end_index.saturating_sub(start_index))
        }
    }
}
