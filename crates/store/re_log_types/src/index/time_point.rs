use std::collections::{btree_map, BTreeMap};

use super::{IndexCell, NonMinI64, TimeInt, Timeline, TimelineName};

/// A point in time on any number of [`Timeline`]s.
///
/// You can think of this as all the index values for one row of data.
///
/// If a [`TimePoint`] is empty ([`TimePoint::default`]), the data will be considered _static_.
/// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
/// any temporal data of the same type.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(BTreeMap<TimelineName, IndexCell>);

impl From<BTreeMap<TimelineName, IndexCell>> for TimePoint {
    fn from(map: BTreeMap<TimelineName, IndexCell>) -> Self {
        Self(map)
    }
}

impl TimePoint {
    #[inline]
    pub fn get(&self, timeline_name: &TimelineName) -> Option<NonMinI64> {
        self.0.get(timeline_name).map(|cell| cell.value)
    }

    #[inline]
    pub fn insert_index(
        &mut self,
        timeline_name: impl Into<TimelineName>,
        cell: impl Into<IndexCell>,
    ) {
        let timeline_name = timeline_name.into();
        let cell = cell.into();

        match self.0.entry(timeline_name) {
            btree_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(cell);
            }
            btree_map::Entry::Occupied(mut occupied_entry) => {
                let existing_typ = occupied_entry.get().typ();
                if existing_typ != cell.typ() {
                    re_log::warn_once!(
                        "Timeline {timeline_name:?} changed type from {existing_typ:?} to {:?}. \
                         Rerun does not support using different types for the same timeline.",
                        cell.typ()
                    );
                }
                occupied_entry.insert(cell);
            }
        }
    }

    #[inline]
    pub fn insert(&mut self, timeline: Timeline, time: impl TryInto<TimeInt>) {
        let cell = IndexCell::new(timeline.typ(), TimeInt::saturated_nonstatic(time).as_i64());
        self.insert_index(*timeline.name(), cell);
    }

    #[must_use]
    #[inline]
    pub fn with_index(
        mut self,
        timeline_name: impl Into<TimelineName>,
        cell: impl Into<IndexCell>,
    ) -> Self {
        self.insert_index(timeline_name, cell);
        self
    }

    #[must_use]
    #[inline]
    pub fn with(mut self, timeline: Timeline, time: impl TryInto<TimeInt>) -> Self {
        self.insert(timeline, time);
        self
    }

    #[inline]
    pub fn remove(&mut self, timeline: &TimelineName) {
        self.0.remove(timeline);
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
        self.0.keys()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TimelineName, &IndexCell)> {
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
    type Item = (TimelineName, IndexCell);

    type IntoIter = btree_map::IntoIter<TimelineName, IndexCell>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a TimePoint {
    type Item = (&'a TimelineName, &'a IndexCell);

    type IntoIter = btree_map::Iter<'a, TimelineName, IndexCell>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<Name, Cell> FromIterator<(Name, Cell)> for TimePoint
where
    Name: Into<TimelineName>,
    Cell: Into<IndexCell>,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = (Name, Cell)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(name, cell)| (name.into(), cell.into()))
                .collect(),
        )
    }
}

impl<Name, Cell, const N: usize> From<[(Name, Cell); N]> for TimePoint
where
    Name: Into<TimelineName>,
    Cell: Into<IndexCell>,
{
    #[inline]
    fn from(timelines: [(Name, Cell); N]) -> Self {
        Self(
            timelines
                .into_iter()
                .map(|(name, cell)| (name.into(), cell.into()))
                .collect(),
        )
    }
}

impl<T: TryInto<TimeInt>> FromIterator<(Timeline, T)> for TimePoint {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (Timeline, T)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(timeline, time)| {
                    let time = TimeInt::saturated_nonstatic(time);
                    (
                        *timeline.name(),
                        IndexCell::new(timeline.typ(), time.as_i64()),
                    )
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
                    let time = TimeInt::saturated_nonstatic(time);
                    (
                        *timeline.name(),
                        IndexCell::new(timeline.typ(), time.as_i64()),
                    )
                })
                .collect(),
        )
    }
}
