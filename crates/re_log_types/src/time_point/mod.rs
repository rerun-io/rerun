use std::collections::{btree_map, BTreeMap};

mod non_min_i64;
mod time_int;
mod timeline;

use crate::{
    time::{Time, TimeZone},
    TimeRange,
};

// Re-exports
pub use non_min_i64::{NonMinI64, TryFromIntError};
pub use time_int::TimeInt;
pub use timeline::{Timeline, TimelineName};

/// A point in time on any number of [`Timeline`]s.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
///
/// If this is empty, the data is _timeless_.
/// Timeless data will show up on all timelines, past and future,
/// and will hit all time queries. In other words, it is always there.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(BTreeMap<Timeline, TimeInt>);

impl From<BTreeMap<Timeline, TimeInt>> for TimePoint {
    fn from(timelines: BTreeMap<Timeline, TimeInt>) -> Self {
        Self(timelines)
    }
}

impl TimePoint {
    /// Logging to this time means the data will show up in all timelines, past and future.
    ///
    /// The time will be [`TimeInt::MIN`], meaning it will always be in range for any time query.
    //
    // TODO(#5264): rework this when we migrate away from the legacy timeless model
    pub fn timeless() -> Self {
        Self::default()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&TimeInt> {
        self.0.get(timeline)
    }

    pub fn insert(&mut self, timeline: Timeline, time: TimeInt) -> Option<TimeInt> {
        self.0.insert(timeline, time)
    }

    pub fn remove(&mut self, timeline: &Timeline) -> Option<TimeInt> {
        self.0.remove(timeline)
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    #[inline]
    pub fn times(&self) -> impl ExactSizeIterator<Item = &TimeInt> {
        self.0.values()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &TimeInt)> {
        self.0.iter()
    }

    /// Computes the union of two `TimePoint`s, keeping the maximum time value in case of
    /// conflicts.
    #[inline]
    pub fn union_max(mut self, rhs: &Self) -> Self {
        for (&timeline, &time) in rhs {
            match self.0.entry(timeline) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(time);
                }
                btree_map::Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();
                    *entry = TimeInt::max(*entry, time);
                }
            }
        }
        self
    }
}

impl re_types_core::SizeBytes for TimePoint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

// ----------------------------------------------------------------------------

/// The type of a [`TimeInt`] or [`Timeline`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, num_derive::FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

impl TimeType {
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }

    pub fn format(&self, time_int: TimeInt, time_zone_for_timestamps: TimeZone) -> String {
        match time_int {
            TimeInt::STATIC => "<static>".into(),
            // TODO(#5264): remove time panel hack once we migrate to the new static UI
            TimeInt::MIN | TimeInt::MIN_TIME_PANEL => "-∞".into(),
            TimeInt::MAX => "+∞".into(),
            _ => match self {
                Self::Time => Time::from(time_int).format(time_zone_for_timestamps),
                Self::Sequence => format!("#{}", time_int.as_i64()),
            },
        }
    }

    pub fn format_utc(&self, time_int: TimeInt) -> String {
        self.format(time_int, TimeZone::Utc)
    }

    pub fn format_range(
        &self,
        time_range: TimeRange,
        time_zone_for_timestamps: TimeZone,
    ) -> String {
        format!(
            "{}..={}",
            self.format(time_range.min, time_zone_for_timestamps),
            self.format(time_range.max, time_zone_for_timestamps)
        )
    }

    pub fn format_range_utc(&self, time_range: TimeRange) -> String {
        self.format_range(time_range, TimeZone::Utc)
    }
}

// ----------------------------------------------------------------------------

impl IntoIterator for TimePoint {
    type Item = (Timeline, TimeInt);

    type IntoIter = btree_map::IntoIter<Timeline, TimeInt>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a TimePoint {
    type Item = (&'a Timeline, &'a TimeInt);

    type IntoIter = btree_map::Iter<'a, Timeline, TimeInt>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<(Timeline, TimeInt)> for TimePoint {
    #[inline]
    fn from_iter<T: IntoIterator<Item = (Timeline, TimeInt)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<const N: usize> From<[(Timeline, TimeInt); N]> for TimePoint {
    #[inline]
    fn from(timelines: [(Timeline, TimeInt); N]) -> Self {
        Self(timelines.into_iter().collect())
    }
}
