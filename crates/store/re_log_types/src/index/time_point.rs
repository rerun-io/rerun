use std::collections::{btree_map, BTreeMap};

use super::{TimeInt, Timeline, TimelineName};

/// A point in time on any number of [`Timeline`]s.
///
/// It can be represented by [`crate::Time`], a sequence index, or a mix of several things.
///
/// If a [`TimePoint`] is empty ([`TimePoint::default`]), the data will be considered _static_.
/// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
/// any temporal data of the same type.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(BTreeMap<Timeline, TimeInt>); // TODO(#9084): use `TimelineName` as the key

impl From<BTreeMap<Timeline, TimeInt>> for TimePoint {
    fn from(timelines: BTreeMap<Timeline, TimeInt>) -> Self {
        Self(timelines)
    }
}

impl TimePoint {
    #[inline]
    pub fn get(&self, timeline_name: &TimelineName) -> Option<&TimeInt> {
        self.0.iter().find_map(|(timeline, time)| {
            if timeline.name() == timeline_name {
                Some(time)
            } else {
                None
            }
        })
    }

    #[inline]
    pub fn insert(&mut self, timeline: Timeline, time: impl TryInto<TimeInt>) -> Option<TimeInt> {
        self.0.retain(|existing_timeline, _| {
            // TODO(#9084): remove this
            if existing_timeline.name() == timeline.name()
                && existing_timeline.typ() != timeline.typ()
            {
                re_log::warn_once!(
                    "Timeline {:?} changed type from {:?} to {:?}. \
                     Rerun does not support using different types for the same timeline.",
                    timeline.name(),
                    existing_timeline.typ(),
                    timeline.typ()
                );
                false // remove old value
            } else {
                true
            }
        });

        let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        self.0.insert(timeline, time)
    }

    #[inline]
    pub fn with(mut self, timeline: Timeline, time: impl TryInto<TimeInt>) -> Self {
        self.insert(timeline, time);
        self
    }

    #[inline]
    pub fn remove(&mut self, timeline: &TimelineName) {
        self.0.retain(|k, _| k.name() != timeline);
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
    pub fn timeline_names(&self) -> impl ExactSizeIterator<Item = &TimelineName> {
        self.0.keys().map(|k| k.name())
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &TimeInt)> {
        self.0.iter()
    }
}

impl re_byte_size::SizeBytes for TimePoint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
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
