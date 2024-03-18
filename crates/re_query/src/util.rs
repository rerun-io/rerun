use re_data_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_log_types::EntityPath;
use re_types_core::Archetype;

use crate::{query_archetype, range::range_archetype, ArchetypeView};

// ---

/// One of the boundaries of the visible history.
///
/// For [`VisibleHistoryBoundary::RelativeToTimeCursor`] and [`VisibleHistoryBoundary::Absolute`],
/// the value are either nanos or frames, depending on the type of timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleHistoryBoundary {
    /// Boundary is a value relative to the time cursor
    RelativeToTimeCursor(i64),

    /// Boundary is an absolute value
    Absolute(i64),

    /// The boundary extends to infinity.
    Infinite,
}

impl VisibleHistoryBoundary {
    /// Value when the boundary is set to the current time cursor.
    pub const AT_CURSOR: Self = Self::RelativeToTimeCursor(0);
}

impl Default for VisibleHistoryBoundary {
    fn default() -> Self {
        Self::AT_CURSOR
    }
}

/// Visible history bounds.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct VisibleHistory {
    /// Low time boundary.
    pub from: VisibleHistoryBoundary,

    /// High time boundary.
    pub to: VisibleHistoryBoundary,
}

impl VisibleHistory {
    /// Value with the visible history feature is disabled.
    pub const OFF: Self = Self {
        from: VisibleHistoryBoundary::AT_CURSOR,
        to: VisibleHistoryBoundary::AT_CURSOR,
    };

    pub const ALL: Self = Self {
        from: VisibleHistoryBoundary::Infinite,
        to: VisibleHistoryBoundary::Infinite,
    };

    /// Returns the start boundary of the time range given an input cursor position.
    ///
    /// This is not guaranteed to be lesser than or equal to [`Self::to`].
    /// Do not use this to build a [`TimeRange`], use [`Self::time_range`].
    #[doc(hidden)]
    pub fn range_start_from_cursor(&self, cursor: TimeInt) -> TimeInt {
        match self.from {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::from(value),
            VisibleHistoryBoundary::RelativeToTimeCursor(value) => cursor + TimeInt::from(value),
            VisibleHistoryBoundary::Infinite => TimeInt::MIN,
        }
    }

    /// Returns the end boundary of the time range given an input cursor position.
    ///
    /// This is not guaranteed to be greater than [`Self::from`].
    /// Do not use this to build a [`TimeRange`], use [`Self::time_range`].
    #[doc(hidden)]
    pub fn range_end_from_cursor(&self, cursor: TimeInt) -> TimeInt {
        match self.to {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::from(value),
            VisibleHistoryBoundary::RelativeToTimeCursor(value) => cursor + TimeInt::from(value),
            VisibleHistoryBoundary::Infinite => TimeInt::MAX,
        }
    }

    /// Returns a _sanitized_ [`TimeRange`], i.e. guaranteed to be monotonically increasing.
    pub fn time_range(&self, cursor: TimeInt) -> TimeRange {
        let mut from = self.range_start_from_cursor(cursor);
        let mut to = self.range_end_from_cursor(cursor);

        // TODO(#4993): visible time range UI can yield inverted ranges
        if from > to {
            std::mem::swap(&mut from, &mut to);
        }

        TimeRange::new(from, to)
    }
}

/// When showing an entity in the history view, add this much history to it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ExtraQueryHistory {
    /// Is the feature enabled?
    pub enabled: bool,

    /// Visible history settings for time timelines
    pub nanos: VisibleHistory,

    /// Visible history settings for frame timelines
    pub sequences: VisibleHistory,
}

// ---

pub fn query_archetype_with_history<'a, A: Archetype + 'a, const N: usize>(
    store: &'a DataStore,
    timeline: &'a Timeline,
    time: &'a TimeInt,
    history: &ExtraQueryHistory,
    ent_path: &'a EntityPath,
) -> crate::Result<impl Iterator<Item = ArchetypeView<A>> + 'a> {
    let visible_history = match timeline.typ() {
        re_log_types::TimeType::Time => history.nanos,
        re_log_types::TimeType::Sequence => history.sequences,
    };

    let time_range = visible_history.time_range(*time);

    if !history.enabled || time_range.min == time_range.max {
        let latest_query = LatestAtQuery::new(*timeline, time_range.min);
        let latest = query_archetype::<A>(store, &latest_query, ent_path)?;

        Ok(itertools::Either::Left(std::iter::once(latest)))
    } else {
        let range_query = RangeQuery::new(*timeline, time_range);

        let range = range_archetype::<A, N>(store, &range_query, ent_path);

        Ok(itertools::Either::Right(range))
    }
}
