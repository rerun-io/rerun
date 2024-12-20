use std::collections::{btree_map, BTreeMap};

mod non_min_i64;
mod time_int;
mod timeline;

use crate::{
    time::{Time, TimeZone},
    ResolvedTimeRange,
};

// Re-exports
pub use non_min_i64::{NonMinI64, TryFromIntError};
pub use time_int::TimeInt;
pub use timeline::{Timeline, TimelineName};

/// A point in time on any number of [`Timeline`]s.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
///
/// If a [`TimePoint`] is empty ([`TimePoint::default`]), the data will be considered _static_.
/// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
/// any temporal data of the same type.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(BTreeMap<Timeline, TimeInt>);

impl From<BTreeMap<Timeline, TimeInt>> for TimePoint {
    fn from(timelines: BTreeMap<Timeline, TimeInt>) -> Self {
        Self(timelines)
    }
}

impl TimePoint {
    #[inline]
    pub fn get(&self, timeline: &Timeline) -> Option<&TimeInt> {
        self.0.get(timeline)
    }

    #[inline]
    pub fn insert(&mut self, timeline: Timeline, time: impl TryInto<TimeInt>) -> Option<TimeInt> {
        let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        self.0.insert(timeline, time)
    }

    #[inline]
    pub fn with(mut self, timeline: Timeline, time: impl TryInto<TimeInt>) -> Self {
        self.insert(timeline, time);
        self
    }

    #[inline]
    pub fn remove(&mut self, timeline: &Timeline) -> Option<TimeInt> {
        self.0.remove(timeline)
    }

    #[inline]
    pub fn is_static(&self) -> bool {
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

impl re_byte_size::SizeBytes for TimePoint {
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
    #[inline]
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }

    pub fn format_sequence(time_int: TimeInt) -> String {
        Self::Sequence.format(time_int, TimeZone::Utc)
    }

    pub fn parse_sequence(s: &str) -> Option<TimeInt> {
        match s {
            "<static>" => Some(TimeInt::STATIC),
            "−∞" => Some(TimeInt::MIN),
            "+∞" => Some(TimeInt::MAX),
            _ => {
                let s = s.strip_prefix('#').unwrap_or(s);
                re_format::parse_i64(s).map(TimeInt::new_temporal)
            }
        }
    }

    pub fn format(
        &self,
        time_int: impl Into<TimeInt>,
        time_zone_for_timestamps: TimeZone,
    ) -> String {
        let time_int = time_int.into();
        match time_int {
            TimeInt::STATIC => "<static>".into(),
            TimeInt::MIN => "−∞".into(),
            TimeInt::MAX => "+∞".into(),
            _ => match self {
                Self::Time => Time::from(time_int).format(time_zone_for_timestamps),
                Self::Sequence => format!("#{}", re_format::format_int(time_int.as_i64())),
            },
        }
    }

    #[inline]
    pub fn format_utc(&self, time_int: TimeInt) -> String {
        self.format(time_int, TimeZone::Utc)
    }

    #[inline]
    pub fn format_range(
        &self,
        time_range: ResolvedTimeRange,
        time_zone_for_timestamps: TimeZone,
    ) -> String {
        format!(
            "{}..={}",
            self.format(time_range.min(), time_zone_for_timestamps),
            self.format(time_range.max(), time_zone_for_timestamps)
        )
    }

    #[inline]
    pub fn format_range_utc(&self, time_range: ResolvedTimeRange) -> String {
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

impl<T: TryInto<TimeInt>> FromIterator<(Timeline, T)> for TimePoint {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (Timeline, T)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(timeline, time)| {
                    let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
                    (timeline, time)
                })
                .collect(),
        )
    }
}

impl<T: TryInto<TimeInt>, const N: usize> From<[(Timeline, T); N]> for TimePoint {
    #[inline]
    fn from(timelines: [(Timeline, T); N]) -> Self {
        Self(
            timelines
                .into_iter()
                .map(|(timeline, time)| {
                    let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
                    (timeline, time)
                })
                .collect(),
        )
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{TimeInt, TimeType};

    #[test]
    fn test_format_parse() {
        let cases = [
            (TimeInt::STATIC, "<static>"),
            (TimeInt::MIN, "−∞"),
            (TimeInt::MAX, "+∞"),
            (TimeInt::new_temporal(-42), "#−42"),
            (TimeInt::new_temporal(12345), "#12 345"),
        ];

        for (int, s) in cases {
            assert_eq!(TimeType::format_sequence(int), s);
            assert_eq!(TimeType::parse_sequence(s), Some(int));
        }
    }
}
