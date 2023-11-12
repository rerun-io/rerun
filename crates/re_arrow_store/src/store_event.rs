use nohash_hasher::IntMap;

use re_log_types::{DataCell, EntityPath, RowId, StoreId, TimeInt, TimePoint, Timeline};
use re_types_core::ComponentName;

use crate::StoreGeneration;

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::DataStore;

// ---

/// The atomic unit of change in the Rerun [`DataStore`].
///
/// A [`StoreEvent`] describes the changes caused by the addition or deletion of a
/// [`re_log_types::DataRow`] in the store.
///
/// Methods that mutate the [`DataStore`], such as [`DataStore::insert_row`] and [`DataStore::gc`],
/// return [`StoreEvent`]s that describe the changes.
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
    /// They must share the same [`RowId`] and [`EntityPath`].
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
