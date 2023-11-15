use nohash_hasher::IntMap;

use re_log_types::{DataCell, EntityPath, RowId, StoreId, TimeInt, TimePoint, Timeline};
use re_types_core::ComponentName;

use crate::StoreGeneration;

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::{DataStore, StoreView};

// ---

/// The atomic unit of change in the Rerun [`DataStore`].
///
/// A [`StoreEvent`] describes the changes caused by the addition or deletion of a
/// [`re_log_types::DataRow`] in the store.
///
/// Methods that mutate the [`DataStore`], such as [`DataStore::insert_row`] and [`DataStore::gc`],
/// return [`StoreEvent`]s that describe the changes.
/// You can also register your own [`StoreView`] in order to be notified of changes as soon as they
/// happen.
///
/// Refer to field-level documentation for more details and check out [`StoreDiff`] for a precise
/// definition of what an event involves.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreEvent {
    /// Which [`DataStore`] sent this event?
    pub store_id: StoreId,

    /// What was the store's generation when it sent that event?
    pub store_generation: StoreGeneration,

    /// Monotonically increasing ID of the event.
    ///
    /// This is on a per-store basis.
    ///
    /// When handling a [`StoreEvent`], if this is the first time you process this [`StoreId`] and
    /// the associated `event_id` is not `1`, it means you registered late and missed some updates.
    pub event_id: u64,

    /// What actually changed?
    ///
    /// Refer to [`StoreDiff`] for more information.
    pub diff: StoreDiff,
}

impl std::ops::Deref for StoreEvent {
    type Target = StoreDiff;

    fn deref(&self) -> &Self::Target {
        &self.diff
    }
}

/// Is it an addition or a deletion?
///
/// Reminder: ⚠ Do not confuse _a deletion_ and _a clear_ ⚠.
///
/// A deletion is the result of a row being completely removed from the store as part of the
/// garbage collection process.
///
/// A clear, on the other hand, is the act of logging an empty [`re_types_core::ComponentBatch`],
/// either directly using the logging APIs, or indirectly through the use of a
/// [`re_types_core::archetypes::Clear`] archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreDiffKind {
    Addition,
    Deletion,
}

impl StoreDiffKind {
    #[inline]
    pub fn delta(&self) -> i64 {
        match self {
            StoreDiffKind::Addition => 1,
            StoreDiffKind::Deletion => -1,
        }
    }
}

/// Describes an atomic change in the Rerun [`DataStore`]: a row has been added or deleted.
///
/// From a query model standpoint, the [`DataStore`] _always_ operates one row at a time:
/// - The contents of a row (i.e. its columns) are immutable past insertion, by virtue of
///   [`RowId`]s being unique and non-reusable.
/// - Similarly, garbage collection always removes _all the data_ associated with a row in one go:
///   there cannot be orphaned columns. When a row is gone, all data associated with it is gone too.
///
/// Refer to field-level documentation for more information.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreDiff {
    /// Addition or deletion?
    ///
    /// The store's internals are opaque and don't necessarily reflect the query model (e.g. there
    /// might be data in the store that cannot by reached by any query).
    ///
    /// A [`StoreDiff`] answers a logical question: "does there exist a query path which can return
    /// data from that row?".
    ///
    /// An event of kind deletion only tells you that, from this point on, no query can return data from that row.
    /// That doesn't necessarily mean that the data is actually gone, i.e. don't make assumptions of e.g. the size
    /// in bytes of the store based on these events.
    /// They are in "query-model space" and are not an accurate representation of what happens in storage space.
    pub kind: StoreDiffKind,

    /// What's the row's [`RowId`]?
    ///
    /// [`RowId`]s are guaranteed to be unique within a single [`DataStore`].
    ///
    /// Put another way, the same [`RowId`] can only appear twice in a [`StoreDiff`] event:
    /// one addition and (optionally) one deletion (in that order!).
    pub row_id: RowId,

    /// The [`TimePoint`] associated with that row.
    ///
    /// Since insertions and deletions both work on a row-level basis, this is guaranteed to be the
    /// same value for both the insertion and deletion events (if any).
    pub timepoint: TimePoint,

    /// The [`EntityPath`] associated with that row.
    ///
    /// Since insertions and deletions both work on a row-level basis, this is guaranteed to be the
    /// same value for both the insertion and deletion events (if any).
    pub entity_path: EntityPath,

    /// All the [`DataCell`]s associated with that row.
    ///
    /// Since insertions and deletions both work on a row-level basis, this is guaranteed to be the
    /// same set of values for both the insertion and deletion events (if any).
    pub cells: IntMap<ComponentName, DataCell>,
}

impl StoreDiff {
    #[inline]
    pub fn addition(row_id: impl Into<RowId>, entity_path: impl Into<EntityPath>) -> Self {
        Self {
            kind: StoreDiffKind::Addition,
            row_id: row_id.into(),
            timepoint: TimePoint::timeless(),
            entity_path: entity_path.into(),
            cells: Default::default(),
        }
    }

    #[inline]
    pub fn deletion(row_id: impl Into<RowId>, entity_path: impl Into<EntityPath>) -> Self {
        Self {
            kind: StoreDiffKind::Deletion,
            row_id: row_id.into(),
            timepoint: TimePoint::timeless(),
            entity_path: entity_path.into(),
            cells: Default::default(),
        }
    }

    #[inline]
    pub fn at_timepoint(mut self, timepoint: impl Into<TimePoint>) -> StoreDiff {
        self.timepoint = self.timepoint.union_max(&timepoint.into());
        self
    }

    #[inline]
    pub fn at_timestamp(
        mut self,
        timeline: impl Into<Timeline>,
        time: impl Into<TimeInt>,
    ) -> StoreDiff {
        self.timepoint.insert(timeline.into(), time.into());
        self
    }

    #[inline]
    pub fn with_cells(mut self, cells: impl IntoIterator<Item = DataCell>) -> Self {
        self.cells
            .extend(cells.into_iter().map(|cell| (cell.component_name(), cell)));
        self
    }

    /// Returns the union of two [`StoreDiff`]s.
    ///
    /// They must share the same [`RowId`], [`EntityPath`] and [`StoreDiffKind`].
    #[inline]
    pub fn union(&self, rhs: &Self) -> Option<Self> {
        let Self {
            kind: lhs_kind,
            row_id: lhs_row_id,
            timepoint: lhs_timepoint,
            entity_path: lhs_entity_path,
            cells: lhs_cells,
        } = self;
        let Self {
            kind: rhs_kind,
            row_id: rhs_row_id,
            timepoint: rhs_timepoint,
            entity_path: rhs_entity_path,
            cells: rhs_cells,
        } = rhs;

        let same_kind = lhs_kind == rhs_kind;
        let same_row_id = lhs_row_id == rhs_row_id;
        let same_entity_path = lhs_entity_path == rhs_entity_path;

        (same_kind && same_row_id && same_entity_path).then(|| Self {
            kind: *lhs_kind,
            row_id: *lhs_row_id,
            timepoint: lhs_timepoint.clone().union_max(rhs_timepoint),
            entity_path: lhs_entity_path.clone(),
            cells: [lhs_cells.values(), rhs_cells.values()]
                .into_iter()
                .flatten()
                .map(|cell| (cell.component_name(), cell.clone()))
                .collect(),
        })
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.timepoint.is_timeless()
    }

    /// `-1` for deletions, `+1` for additions.
    #[inline]
    pub fn delta(&self) -> i64 {
        self.kind.delta()
    }

    #[inline]
    pub fn num_components(&self) -> usize {
        self.cells.len()
    }
}

#[cfg(tests)]
mod tests {
    use std::collections::BTreeMap;

    use re_log_types::{
        example_components::{MyColor, MyPoint, MyPoints},
        DataRow, DataTable, RowId, TableId, Time, TimePoint, Timeline,
    };
    use re_types_core::{components::InstanceKey, Loggable as _};

    use crate::{DataStore, GarbageCollectionOptions, StoreView, StoreViewHandle};

    use super::*;

    /// A simple store subscriber for test purposes that keeps track of the quantity of data available
    /// in the store a the lowest level of detail.
    ///
    /// The counts represent numbers of rows: e.g. how many unique rows contain this entity path?
    #[derive(Default, Debug, PartialEq, Eq)]
    struct GlobalCounts {
        row_ids: BTreeMap<RowId, i64>,
        timelines: BTreeMap<Timeline, i64>,
        entity_paths: BTreeMap<EntityPath, i64>,
        component_names: BTreeMap<ComponentName, i64>,
        times: BTreeMap<TimeInt, i64>,
        timeless: i64,
    }

    impl GlobalCounts {
        fn new(
            row_ids: impl IntoIterator<Item = (RowId, i64)>, //
            timelines: impl IntoIterator<Item = (Timeline, i64)>, //
            entity_paths: impl IntoIterator<Item = (EntityPath, i64)>, //
            component_names: impl IntoIterator<Item = (ComponentName, i64)>, //
            times: impl IntoIterator<Item = (TimeInt, i64)>, //
            timeless: i64,
        ) -> Self {
            Self {
                row_ids: row_ids.into_iter().collect(),
                timelines: timelines.into_iter().collect(),
                entity_paths: entity_paths.into_iter().collect(),
                component_names: component_names.into_iter().collect(),
                times: times.into_iter().collect(),
                timeless,
            }
        }
    }

    impl GlobalCounts {
        fn on_events(&mut self, events: &[StoreEvent]) {
            for event in events {
                let delta = event.delta();

                *self.row_ids.entry(event.row_id).or_default() += delta;
                *self
                    .entity_paths
                    .entry(event.entity_path.clone())
                    .or_default() += delta;

                for component_name in event.cells.keys() {
                    *self.component_names.entry(*component_name).or_default() += delta;
                }

                if event.is_timeless() {
                    self.timeless += delta;
                } else {
                    for (&timeline, &time) in &event.timepoint {
                        *self.timelines.entry(timeline).or_default() += delta;
                        *self.times.entry(time).or_default() += delta;
                    }
                }
            }
        }
    }

    #[test]
    fn store_events() -> anyhow::Result<()> {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );

        let mut view = GlobalCounts::default();

        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_temporal("other");
        let timeline_yet_another = Timeline::new_sequence("yet_another");

        let row_id1 = RowId::random();
        let timepoint1 = TimePoint::from_iter([
            (timeline_frame, 42.into()),      //
            (timeline_other, 666.into()),     //
            (timeline_yet_another, 1.into()), //
        ]);
        let entity_path1: EntityPath = "entity_a".into();
        let row1 = DataRow::from_component_batches(
            row_id1,
            timepoint1.clone(),
            entity_path1.clone(),
            [&InstanceKey::from_iter(0..10) as _],
        )?;

        view.on_events(&[store.insert_row(&row1)?]);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                ],
                [
                    (timeline_frame, 1),
                    (timeline_other, 1),
                    (timeline_yet_another, 1),
                ],
                [
                    (entity_path1.clone(), 1), //
                ],
                [
                    (InstanceKey::name(), 1), //
                ],
                [
                    (42.into(), 1), //
                    (666.into(), 1),
                    (1.into(), 1),
                ],
                0,
            ),
            view,
        );

        let row_id2 = RowId::random();
        let timepoint2 = TimePoint::from_iter([
            (timeline_frame, 42.into()),      //
            (timeline_yet_another, 1.into()), //
        ]);
        let entity_path2: EntityPath = "entity_b".into();
        let row2 = {
            let num_instances = 3;
            let points: Vec<_> = (0..num_instances)
                .map(|i| MyPoint::new(0.0, i as f32))
                .collect();
            let colors = vec![MyColor::from(0xFF0000FF)];
            DataRow::from_component_batches(
                row_id2,
                timepoint2.clone(),
                entity_path2.clone(),
                [&points as _, &colors as _],
            )?
        };

        view.on_events(&[store.insert_row(&row2)?]);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                    (row_id2, 1),
                ],
                [
                    (timeline_frame, 2),
                    (timeline_other, 1),
                    (timeline_yet_another, 2),
                ],
                [
                    (entity_path1.clone(), 1), //
                    (entity_path2.clone(), 1), //
                ],
                [
                    (InstanceKey::name(), 2), //
                    (MyPoint::name(), 1),     //
                    (MyColor::name(), 1),     //
                ],
                [
                    (42.into(), 2), //
                    (666.into(), 1),
                    (1.into(), 2),
                ],
                0,
            ),
            view,
        );

        let row_id3 = RowId::random();
        let timepoint3 = TimePoint::timeless();
        let row3 = {
            let num_instances = 6;
            let colors = vec![MyColor::from(0x00DD00FF); num_instances];
            DataRow::from_component_batches(
                row_id3,
                timepoint3.clone(),
                entity_path2.clone(),
                [
                    &InstanceKey::from_iter(0..num_instances as _) as _,
                    &colors as _,
                ],
            )?
        };

        view.on_events(&[store.insert_row(&row3)?]);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                    (row_id2, 1),
                    (row_id3, 1),
                ],
                [
                    (timeline_frame, 2),
                    (timeline_other, 1),
                    (timeline_yet_another, 2),
                ],
                [
                    (entity_path1.clone(), 1), //
                    (entity_path2.clone(), 2), //
                ],
                [
                    (InstanceKey::name(), 3), //
                    (MyPoint::name(), 1),     //
                    (MyColor::name(), 2),     //
                ],
                [
                    (42.into(), 2), //
                    (666.into(), 1),
                    (1.into(), 2),
                ],
                1,
            ),
            view,
        );

        view.on_events(&store.gc(GarbageCollectionOptions::gc_everything()).0);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 0), //
                    (row_id2, 0),
                    (row_id3, 0),
                ],
                [
                    (timeline_frame, 0),
                    (timeline_other, 0),
                    (timeline_yet_another, 0),
                ],
                [
                    (entity_path1.clone(), 0), //
                    (entity_path2.clone(), 0), //
                ],
                [
                    (InstanceKey::name(), 0), //
                    (MyPoint::name(), 0),     //
                    (MyColor::name(), 0),     //
                ],
                [
                    (42.into(), 0), //
                    (666.into(), 0),
                    (1.into(), 0),
                ],
                0,
            ),
            view,
        );

        Ok(())
    }
}
