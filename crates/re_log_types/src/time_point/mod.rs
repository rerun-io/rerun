use chrono::{Local, TimeZone as _};
use std::collections::{btree_map, BTreeMap};

mod time_int;
mod timeline;

use crate::{time::Time, TimeRange};

// Re-exports
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
    /// Logging to this time means the data will show upp in all timelines,
    /// past and future. The time will be [`TimeInt::BEGINNING`], meaning it will
    /// always be in range for any time query.
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

    pub fn format(&self, time_int: TimeInt) -> String {
        if time_int <= TimeInt::BEGINNING {
            "-∞".into()
        } else if time_int >= TimeInt::MAX {
            "+∞".into()
        } else {
            match self {
                Self::Time => Time::from(time_int).format(),
                Self::Sequence => format!("#{}", time_int.0),
            }
        }
    }

    pub fn format_range(&self, time_range: TimeRange) -> String {
        format!(
            "{}..={}",
            self.format(time_range.min),
            self.format(time_range.max)
        )
    }

    pub fn format_to_local_time(&self, time_int: TimeInt) -> String {
        if time_int <= TimeInt::BEGINNING {
            "-∞".into()
        } else if time_int >= TimeInt::MAX {
            "+∞".into()
        } else {
            match self {
                Self::Time => Local
                    .timestamp_nanos(time_int.0)
                    .format("%H:%M:%S%.3f")
                    .to_string(),
                Self::Sequence => format!("#{}", time_int.0),
            }
        }
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
