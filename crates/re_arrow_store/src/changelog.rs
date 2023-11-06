use std::{collections::BTreeMap, sync::Arc};

use ahash::HashSet;
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_log_types::{
    DataCell, DataCellVec, DataRow, EntityPath, EntityPathHash, RowId, StoreId, TimeInt, TimePoint,
    Timeline,
};
use re_types_core::ComponentName;

use crate::{DataStore, StoreGeneration};

// TODO: Compaction view for bootstrapping
// TODO: document expectations when registering after startup

// TODO: we do everything synchronously for now. keep more problems for later.

// pub trait ChangelogSubscriber {
//     // TODO: introduce a non mut version if needed i guess
//     fn on_update<'a>(&mut self, modifications: impl IntoIterator<Item = &'a CellAddition>);
// }

// TODO: I shouldnt need the box, parking_lot is behaving all weird
// TODO: thread semantics
pub type SharedStoreView = RwLock<Box<dyn StoreView>>;

pub type StoreViewBuilder = dyn FnMut() -> Box<dyn StoreView> + Send + Sync;

// TODO: doc
pub trait StoreView: std::any::Any + Send + Sync {
    fn name(&self) -> String;

    fn registerable(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    // TODO: batch to open opportunities for optimization on users' end
    fn on_events(&mut self, events: &[StoreEvent]);
}

// impl StoreView for Box<dyn

static VIEW_BUILDERS: once_cell::sync::Lazy<RwLock<Vec<Box<StoreViewBuilder>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

static VIEWS: once_cell::sync::Lazy<RwLock<Vec<SharedStoreView>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

// impl DataStore {
//     // TODO
//     pub fn register_view_builder(view_builder: Box<StoreViewBuilder>) {
//         VIEW_BUILDERS.write().push(view_builder);
//     }
//
//     // TODO: unregister
//
//     fn autoregister_views(&mut self) {
//         for view_builder in &mut *VIEW_BUILDERS.write() {
//             self.register_view(view_builder());
//         }
//     }
// }

// TODO: how does all of this behave with empty data (clears)?

// TODO: internal views might care about implementation details (e.g. bucket splits and such),
// while external views are about caching our public-facing data model.

// TODO: would be nice to list all internal & external views.

// TODO: remove ChangeLog, keep only StoreModification?

// TODO:
// - row based doesnt really make sense...

// TODO: really name better names
// - Changelog
// - ChangelogEntry
// - ChangelogRowEntry?

// TODO: "represents a bunch of additions/deletions to/from the store"
#[derive(Debug, Clone)]
pub struct Changelog {
    pub modifications: Vec<CellAddition>,
}

#[derive(Debug, Clone, Copy)]
pub struct StoreViewHandle(u32);

impl DataStore {
    // TODO
    // TODO: or update manually if you need more control
    // TODO: fn registration_allowed?
    pub fn register_view(view: Box<dyn StoreView>) -> StoreViewHandle {
        if !view.registerable() {
            panic!("canot register view '{}'", view.name());
            // TODO: dont panic
        }
        let mut views = VIEWS.write();
        views.push(RwLock::new(view));
        StoreViewHandle(views.len() as u32 - 1)
    }

    pub fn with_view<V: StoreView, T, F: FnMut(&V) -> T>(
        StoreViewHandle(handle): StoreViewHandle,
        mut f: F,
    ) -> Option<T> {
        let views = VIEWS.read();
        views.get(handle as usize).and_then(|view| {
            let view = view.read();
            view.as_any().downcast_ref::<V>().map(&mut f)
        })
    }

    pub fn with_view_mut<V: StoreView, T, F: FnMut(&mut V) -> T>(
        StoreViewHandle(handle): StoreViewHandle,
        mut f: F,
    ) -> Option<T> {
        let views = VIEWS.read();
        views.get(handle as usize).and_then(|view| {
            let mut view = view.write();
            view.as_any_mut().downcast_mut::<V>().map(&mut f)
        })
    }

    pub(crate) fn on_events(events: &[StoreEvent]) {
        let views = VIEWS.read();
        for view in views.iter() {
            view.write().on_events(events);
        }
    }

    // pub(crate) fn new_added_event(&self, diff: impl IntoStoreDiff) -> StoreEvent {
    //     StoreEvent {
    //         generation: self.generation(),
    //         event_id: self
    //             .event_id
    //             .load(std::sync::atomic::Ordering::Relaxed)
    //             .into(),
    //         diff: diff.added(),
    //     }
    // }
    //
    // pub(crate) fn new_removed_event(&self, diff: impl IntoStoreDiff) -> StoreEvent {
    //     StoreEvent {
    //         generation: self.generation(),
    //         event_id: self
    //             .event_id
    //             .load(std::sync::atomic::Ordering::Relaxed)
    //             .into(),
    //         diff: diff.removed(),
    //     }
    // }
}

// TODO:
// - implementing views
// - implementing triggers and the art of counting
//      - if delta sum ever < 0, it's a bug in the store
// - ordering: views will likely be parallized at some point!
#[derive(Debug, Clone)]
pub struct StoreEvent {
    pub store_id: StoreId,
    pub store_generation: StoreGeneration,
    pub event_id: u64,
    pub diff: StoreDiff, // TODO: Arc?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreDiffKind {
    Addition,
    Deletion,
}

impl StoreEvent {
    #[inline]
    pub fn row_id(&self) -> RowId {
        self.diff.row_id
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.diff.is_timeless()
    }
}

// // TODO: remove the enum and use counts instead so streams can be compacted
// // ^^^ that can be future work though really
// #[derive(Debug, Clone)]
// pub enum StoreDiff {
//     TimelessDataAdded(DataDiff),
//     TimelessDataRemoved(DataDiff),
//     DataAdded(Timeline, TimeInt, DataDiff),
//     DataRemoved(Timeline, TimeInt, DataDiff),
// }

// TODO: decorrelate store internals from data model: the user doesnt care that the store still has
// an empty bucket around for a given entity path, rather they care whether a query would return
// such an entity path or ot. We're building views of the _data model_.

// TODO: split everything in dedicated structs + box everything?
//
// TODO: document that these things' order matter, and are not idempotent by definition.
//
// TODO: Reminders:
// - RowIds cannot be shared across entities
// - RowIds cannot be shared across timeful & timeless data
//
// TODO: we could even have CellAdded/CellRemoved with this model.
//
// TODO: we want A) to make sure views/plugins don't have to deal with all the crazy complexity of
// our data model (e.g. "is this RowId gone now?") and B) to make sure they only execute as
// minimally as necessary.
//
// TODO: what about rows? cells? sizes in general?
//
// TODO: ComponentAdded vs. ComponentDataAdded added is not the same thing...
// but ComponentDataAdded vs. TimestampAdded effectively is!
//
// TODO: an atomic diff, specifically
// #[derive(Debug, Clone)]
// pub enum StoreDiff2 {
//     // --- Global scope ---
//     // TODO: should we indicate whether that's timeless?
//     RowIdAdded(RowIdDiff),
//     RowIdRemoved(RowIdDiff),
//
//     // --- Global/Timeless Boxscope ---
//     TimelessEntityPathAdded(Box<TimelessEntityPathDiff>),
//     TimelessEntityPathRemoved(Box<TimelessEntityPathDiff>),
//
//     // --- Global/Timeless/Entity scope ---
//     TimelessComponentAdded(Box<TimelessComponentDiff>),
//     TimelessComponentRemoved(Box<TimelessComponentDiff>),
//
//     // --- Global/Timeless/Entity/Component scope ---
//     TimelessDataAdded(Box<TimelessDataDiff>),
//     TimelessDataRemoved(Box<TimelessDataDiff>),
//
//     // --- Global/Timeline scope ---
//     EntityPathAdded(Box<EntityPathDiff>),
//     EntityPathRemoved(Box<EntityPathDiff>),
//
//     // --- Global/Timeline/Entity scope ---
//     ComponentAdded(Box<ComponentDiff>),
//     ComponentRemoved(Box<ComponentDiff>),
//
//     // --- Global/Timeline/Entity/Component scope ---
//     // TODO: We're gonna have to explain that the same timestamp can be added/removed more than once:
//     // ```
//     // rr.set_time_sequence("frame", 42)
//     // rr.log("my_ent", rr.Points3D([0, 0, 0])) # TimestampAdded("frame", hash("my_ent"), "Position3D", 42)
//     // rr.log("my_ent", rr.Points3D([0, 0, 0])) # TimestampAdded("frame", hash("my_ent"), "Position3D", 42)
//     // ```
//     // TODO: TimestampAdded goes away then i guess
//     TimestampAdded(RowId, Timeline, EntityPathHash, ComponentName, TimeInt),
//     TimestampRemoved(RowId, Timeline, EntityPathHash, ComponentName, TimeInt),
//     // TODO: unless you count?
//     // TODO: still not sure what to do with cluster cells? implementation detail, so no?
//     DataAdded(Box<DataDiff>),
//     DataRemoved(Box<DataDiff>),
// }

// TODO: what if we only keep DataAdded in the end?

// impl StoreDiff {
//     // TODO
//     // pub fn add_timeless_entity_path(row_id: impl Into<RowId>, entity_path_hash: impl Into<EntityPathHash>) ->Self{
//     //     Self::add_timeless_entity_path(row_id, entity_path_hash)
//     // }
//
//     pub fn row_id(&self) -> RowId {
//         match self {
//             StoreDiff::TimelessDataAdded(diff)
//             | StoreDiff::TimelessDataRemoved(diff)
//             | StoreDiff::DataAdded(_, _, diff)
//             | StoreDiff::DataRemoved(_, _, diff) => diff.row_id,
//         }
//     }
//
//     pub fn is_timeless(&self) -> bool {
//         match self {
//             StoreDiff::TimelessDataAdded(_) | StoreDiff::TimelessDataRemoved(_) => true,
//             StoreDiff::DataAdded(_, _, _) | StoreDiff::DataRemoved(_, _, _) => false,
//         }
//     }
// }

// pub trait IntoStoreDiff {
//     fn added(self) -> StoreDiff;
//     fn removed(self) -> StoreDiff;
// }
//
// macro_rules! impl_into_store_diff {
//     ($typ:ty => + $added:expr, - $removed:expr) => {
//         impl $crate::changelog::IntoStoreDiff for $typ {
//             fn added(self) -> StoreDiff {
//                 use $crate::changelog::StoreDiff::*;
//                 $added(Box::new(self))
//             }
//
//             fn removed(self) -> StoreDiff {
//                 use $crate::changelog::StoreDiff::*;
//                 $removed(Box::new(self))
//             }
//         }
//     };
// }
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct RowIdDiff(pub RowId);
//
// impl IntoStoreDiff for RowIdDiff {
//     fn added(self) -> StoreDiff {
//         StoreDiff::RowIdAdded(self)
//     }
//
//     fn removed(self) -> StoreDiff {
//         StoreDiff::RowIdRemoved(self)
//     }
// }
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct TimelessEntityPathDiff {
//     pub row_id: RowId,
//     pub entity_path_hash: EntityPathHash,
// }
//
// impl TimelessEntityPathDiff {
//     pub fn new(row_id: impl Into<RowId>, entity_path_hash: impl Into<EntityPathHash>) -> Self {
//         Self {
//             row_id: row_id.into(),
//             entity_path_hash: entity_path_hash.into(),
//         }
//     }
// }
//
// impl_into_store_diff!(TimelessEntityPathDiff => + TimelessEntityPathAdded, - TimelessEntityPathRemoved);
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct TimelessComponentDiff {
//     pub row_id: RowId,
//     pub entity_path_hash: EntityPathHash,
//     pub component_name: ComponentName,
// }
//
// impl TimelessComponentDiff {
//     pub fn new(
//         row_id: impl Into<RowId>,
//         entity_path_hash: impl Into<EntityPathHash>,
//         component_name: impl Into<ComponentName>,
//     ) -> Self {
//         Self {
//             row_id: row_id.into(),
//             entity_path_hash: entity_path_hash.into(),
//             component_name: component_name.into(),
//         }
//     }
// }
//
// impl_into_store_diff!(TimelessComponentDiff => + TimelessComponentAdded, - TimelessComponentRemoved);
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct TimelessDataDiff {
//     pub row_id: RowId,
//     pub entity_path: EntityPath,
//     pub component_name: ComponentName,
//     pub cell: DataCell,
// }
//
// impl TimelessDataDiff {
//     pub fn new(
//         row_id: impl Into<RowId>,
//         entity_path: impl Into<EntityPath>,
//         component_name: impl Into<ComponentName>,
//         cell: impl Into<DataCell>,
//     ) -> Self {
//         Self {
//             row_id: row_id.into(),
//             entity_path: entity_path.into(),
//             component_name: component_name.into(),
//             cell: cell.into(),
//         }
//     }
// }
//
// impl_into_store_diff!(TimelessDataDiff => + TimelessDataAdded, - TimelessDataRemoved);
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct EntityPathDiff {
//     pub row_id: RowId,
//     pub timeline: Timeline,
//     pub entity_path_hash: EntityPathHash,
// }
//
// impl EntityPathDiff {
//     pub fn new(
//         row_id: impl Into<RowId>,
//         timeline: impl Into<Timeline>,
//         entity_path_hash: impl Into<EntityPathHash>,
//     ) -> Self {
//         Self {
//             row_id: row_id.into(),
//             timeline: timeline.into(),
//             entity_path_hash: entity_path_hash.into(),
//         }
//     }
// }
//
// impl_into_store_diff!(EntityPathDiff => + EntityPathAdded, - EntityPathRemoved);
//
// #[derive(Debug, Clone, PartialEq)]
// pub struct ComponentDiff {
//     pub row_id: RowId,
//     pub timeline: Timeline,
//     pub entity_path_hash: EntityPathHash,
//     pub component_name: ComponentName,
// }
//
// impl ComponentDiff {
//     pub fn new(
//         row_id: impl Into<RowId>,
//         timeline: impl Into<Timeline>,
//         entity_path_hash: impl Into<EntityPathHash>,
//         component_name: impl Into<ComponentName>,
//     ) -> Self {
//         Self {
//             row_id: row_id.into(),
//             timeline: timeline.into(),
//             entity_path_hash: entity_path_hash.into(),
//             component_name: component_name.into(),
//         }
//     }
// }
//
// impl_into_store_diff!(ComponentDiff => + ComponentAdded, - ComponentRemoved);

// TODO
#[derive(Debug, Clone, PartialEq)]
pub struct StoreDiff {
    pub row_id: RowId,
    // TODO: we always insert a full RowId at a time... well, except when we don't, argh
    pub timestamp: Option<(Timeline, TimeInt)>,
    pub entity_path: EntityPath,
    pub component_name: ComponentName,
    pub cell: DataCell,
    // TODO: event compaction etc
    // TODO: if we include a RowId, and the RowId is our atomic unit of insertion/deletion, then
    // this always becomes either zero or one, in which case an enum is actually better in the end.
    pub delta: i64,
}

impl StoreDiff {
    fn new(
        row_id: RowId,
        entity_path: EntityPath,
        component_name: ComponentName,
        cell: DataCell,
        delta: i64,
    ) -> Self {
        Self {
            row_id,
            timestamp: None,
            entity_path,
            component_name,
            cell,
            delta,
        }
    }

    #[inline]
    pub fn addition(
        row_id: impl Into<RowId>,
        entity_path: impl Into<EntityPath>,
        component_name: impl Into<ComponentName>,
        cell: impl Into<DataCell>,
    ) -> Self {
        Self::new(
            row_id.into(),
            entity_path.into(),
            component_name.into(),
            cell.into(),
            1,
        )
    }

    #[inline]
    pub fn deletion(
        row_id: impl Into<RowId>,
        entity_path: impl Into<EntityPath>,
        component_name: impl Into<ComponentName>,
        cell: impl Into<DataCell>,
    ) -> Self {
        Self::new(
            row_id.into(),
            entity_path.into(),
            component_name.into(),
            cell.into(),
            -1,
        )
    }

    #[inline]
    pub fn at(mut self, timeline: impl Into<Timeline>, time: impl Into<TimeInt>) -> StoreDiff {
        self.timestamp = Some((timeline.into(), time.into()));
        self
    }

    pub fn is_timeless(&self) -> bool {
        self.timestamp.is_none()
    }

    // #[inline]
    // pub fn added(self) -> StoreDiff {
    //     StoreDiff::TimelessDataAdded(self)
    // }
    //
    // #[inline]
    // pub fn removed(self) -> StoreDiff {
    //     StoreDiff::TimelessDataRemoved(self)
    // }

    // #[inline]
    // pub fn added_at(self, timeline: impl Into<Timeline>, time: impl Into<TimeInt>) -> StoreDiff {
    //     StoreDiff::DataAdded(timeline.into(), time.into(), self)
    // }
    //
    // // TODO: this is a misnomer, and a very bad one at that...
    // #[inline]
    // pub fn removed_at(self, timeline: impl Into<Timeline>, time: impl Into<TimeInt>) -> StoreDiff {
    //     StoreDiff::DataRemoved(timeline.into(), time.into(), self)
    // }
}

// impl_into_store_diff!(DataDiff => + DataAdded, - DataRemoved);

// TODO:
//
// - [x] RowIdAdded(RowId),
// - [ ] RowIdRemoved(RowId),
//
// - [x] TimelessEntityPathAdded(EntityPathHash),
// - [ ] TimelessEntityPathRemoved(EntityPathHash),
//
// - [x] TimelessComponentAdded(EntityPathHash, ComponentName),
// - [ ] TimelessComponentRemoved(EntityPathHash, ComponentName),
//
// - [x] EntityPathAdded(Timeline, EntityPathHash),
// - [ ] EntityPathRemoved(Timeline, EntityPathHash),
//
// - [x] ComponentAdded(Timeline, EntityPathHash, ComponentName),
// - [ ] ComponentRemoved(Timeline, EntityPathHash, ComponentName),
//
// - [x] TimestampAdded(Timeline, EntityPathHash, ComponentName, TimeInt),
// - [ ] TimestampRemoved(Timeline, EntityPathHash, ComponentName, TimeInt),
//
// - [ ] DataAdded(Timeline, EntityPathHash, ComponentName, TimeInt, DataCell),
// - [ ] DataRemoved(Timeline, EntityPathHash, ComponentName, TimeInt, DataCell),

// TODO: what if *_is_new and *_is_gone were their own events though?

// TODO:
// - we need to know when a RowId/timepoint/entitypath are added/deleted for the first time

// TODO: StoreAddition vs. StoreDeletion?

// TODO: it is likely impossible to have any issue if we fire per-cell, isn't it?

// TODO: should we stick a `(InsertId, GcId)` generation in there? most likely
// TODO: RawStoreAddition?
// TODO: always one row's worth
// TODO: internal datastructures: bucket changes, index updates, etc
#[derive(Debug, Clone, PartialEq)]
pub struct CellAddition {
    //     ~~~~~~~~~~~~~ CellAddition! Atomic unit of change.
    //
    /// The [`RowId`] impacted by this addition.
    pub row_id: RowId,

    /// Whether this [`RowId`] was introduced for the first time in this addition.
    ///
    /// `RowId`s are global to the store: `true` indicates that this is the first time the store as
    /// a whole ever indexed this `RowId`.
    pub row_id_is_new: bool,

    /// The timestamps impacted by this addition. Empty for timeless data.
    ///
    /// The boolean indicates whether or not this timestamp was introduced for the first time in
    /// this addition.
    ///
    // Timestamps are local to a specific `(EntityPath, Set<ComponentName>)` pair: `true` indicates
    // that this is the first time we've stored this timestamp for this [`EntityPath`] with these
    // specific [`Component`]s.
    //
    // The [`EntityPath`] in question is [`Self::entity_path`], while the set of components can be
    // computed using [`Self::cells`].
    //
    /// Timestamps are local to a specific `(EntityPath, ComponentName)` pair: `true` indicates
    /// that this is the first time we've stored this timestamp for this [`EntityPath`] with this
    /// specific [`Component`].
    ///
    /// The [`EntityPath`] in question is [`Self::entity_path`], while the component can be
    /// computed using [`Self::cell`].
    //
    // TODO: new in reference to what? the store as a whole? this RowID? this entity path? this
    // component? we have to go with the most detailed if we want everything downstream to work.
    pub times: IntMap<Timeline, (TimeInt, bool)>,

    // TODO: true if the data is the first timeless entry for this path/component pair.
    pub is_new_timeless: bool,

    /// The [`EntityPath`] impacted by this addition.
    pub entity_path: EntityPathHash,

    /// Whether this [`EntityPath`] was introduced for the first time in this addition.
    ///
    /// `EntityPath`s are global to the store: `true` indicates that this is the first time the store as
    /// a whole ever indexed this `EntityPath`.
    pub entity_path_is_new: bool,

    // TODO: the datacell implictly brings a component in the picture. we need a component_is_new
    // then.

    // /// The [`DataCell`]s introduced in this addition.
    // ///
    // /// We always assume that cells are new.
    // pub cells: DataCellVec,
    //
    /// The [`DataCell`] introduced in this addition.
    ///
    /// We always assume that cells are new.
    //
    // TODO: ^^^ explain wth that even means though.
    pub cell: DataCell,
    // TODO: do we really care about those tho? this is an internal view anyway
    // /// The [`DataCell`] of autogenerated instance keys introduced in this addition, if any.
    // pub cached_cell: Option<DataCell>,
}

// TODO: name
#[derive(Default)]
pub struct Added {
    /// What rows were added?
    pub row_ids: HashSet<RowId>,

    /// What timestamps were added for each `(EntityPath, Timeline, Component)` triplet?
    pub timeful: IntMap<EntityPathHash, IntMap<Timeline, IntMap<ComponentName, Vec<TimeInt>>>>,

    /// For each entity+component, how many timeless entries were added?
    pub timeless: IntMap<EntityPathHash, IntMap<ComponentName, u64>>,
}

impl Added {
    pub fn from_additions<'a>(additions: impl IntoIterator<Item = &'a CellAddition>) -> Self {
        let mut this = Self::default();

        for addition in additions {
            if addition.row_id_is_new {
                this.row_ids.insert(addition.row_id);
            }

            if addition.times.is_empty() {
                let per_component = this.timeless.entry(addition.entity_path).or_default();
                *per_component
                    .entry(addition.cell.component_name())
                    .or_default() += 1;
            } else {
                let per_timeline = this.timeful.entry(addition.entity_path).or_default();
                // TODO: pretty sure we have a problem here
                for (timeline, (time, time_is_new)) in &addition.times {
                    if *time_is_new {
                        let per_component = per_timeline.entry(*timeline).or_default();
                        per_component
                            .entry(addition.cell.component_name())
                            .or_default()
                            .push(*time);
                    }
                }
            }
        }

        this
    }
}

// TODO: gotta think: can we derive `Deleted` from an array of these?
// pub struct Deleted {
//     pub row_ids: HashSet<RowId>,
//     pub timeful: IntMap<EntityPathHash, IntMap<Timeline, IntMap<ComponentName, Vec<TimeInt>>>>,
//     pub timeless: IntMap<EntityPathHash, IntMap<ComponentName, u64>>,
// }
//
// Can't see why not!
#[derive(Debug, Clone, PartialEq)]
pub struct StoreDeletion {
    pub row_id: RowId,
    pub row_id_is_gone: bool,

    pub times: IntMap<Timeline, (TimeInt, bool)>,
    pub timepoint: TimePoint,
    pub timepoint_is_gone: BTreeMap<(Timeline, TimeInt), bool>,

    pub entity_path: EntityPath,
    pub entity_path_is_gone: bool, // TODO: yeah that's gonna be rare

    pub cells: DataCellVec,
}

// TODO: in that case we can just merge the two though

// TODO: invert docs
#[derive(Debug, Clone, PartialEq)]
pub struct CellDeletion {
    /// The [`RowId`] impacted by this deletion.
    pub row_id: RowId,

    /// Whether the last instance of this [`RowId`] was removed in this deletion.
    ///
    /// `RowId`s are global to the store: `true` indicates that this was the last standing instance
    /// of this `RowId` in the entire store.
    pub row_id_is_gone: bool,

    /// The timestamps impacted by this deletion. Empty for timeless data.
    ///
    /// The boolean indicates whether or not this timestamp was introduced for the first time in
    /// this deletion.
    ///
    /// Timestamps are local to a specific `(EntityPath, ComponentName)` pair: `true` indicates
    /// that this is the first time we've stored this timestamp for this [`EntityPath`] with this
    /// specific [`Component`].
    ///
    /// The [`EntityPath`] in question is [`Self::entity_path`], while the component can be
    /// computed using [`Self::cell`].
    //
    // TODO: new in reference to what? the store as a whole? this RowID? this entity path? this
    // component? we have to go with the most detailed if we want everything downstream to work.
    pub times: IntMap<Timeline, (TimeInt, bool)>,

    /// The [`EntityPath`] impacted by this deletion.
    pub entity_path: EntityPathHash,

    /// Whether this [`EntityPath`] was introduced for the first time in this deletion.
    ///
    /// `EntityPath`s are global to the store: `true` indicates that this is the first time the store as
    /// a whole ever indexed this `EntityPath`.
    pub entity_path_is_gone: bool,

    /// The [`DataCell`] introduced in this deletion.
    ///
    /// We always assume that cells are new.
    //
    // TODO: ^^^ explain wth that even means though.
    pub cell: DataCell,
}

// TODO: name
#[derive(Default)]
pub struct Deleted {
    /// What rows were removed?
    pub row_ids: HashSet<RowId>,

    /// What timestamps were removed for each `(EntityPath, Timeline, Component)` triplet?
    pub timeful: IntMap<EntityPathHash, IntMap<Timeline, IntMap<ComponentName, Vec<TimeInt>>>>,

    /// For each entity+component, how many timeless entries were removed?
    pub timeless: IntMap<EntityPathHash, IntMap<ComponentName, u64>>,
}

impl Deleted {
    pub fn from_deletions<'a>(deletions: impl IntoIterator<Item = &'a CellDeletion>) -> Self {
        let mut this = Self::default();

        for deletion in deletions {
            if deletion.row_id_is_gone {
                this.row_ids.insert(deletion.row_id);
            }

            if deletion.times.is_empty() {
                let per_component = this.timeless.entry(deletion.entity_path).or_default();
                *per_component
                    .entry(deletion.cell.component_name())
                    .or_default() += 1;
            } else {
                let per_timeline = this.timeful.entry(deletion.entity_path).or_default();
                // TODO: pretty sure we have a problem here
                for (timeline, (time, time_is_gone)) in &deletion.times {
                    if *time_is_gone {
                        let per_component = per_timeline.entry(*timeline).or_default();
                        per_component
                            .entry(deletion.cell.component_name())
                            .or_default()
                            .push(*time);
                    }
                }
            }
        }

        this
    }
}
//
// impl CellAddition {
//     // TODO: new empty
//     #[inline]
//     pub fn at(
//         row_id: impl Into<RowId>,
//         timepoint: impl Into<TimePoint>,
//         entity_path: impl Into<EntityPath>,
//     ) -> Self {
//         Self {
//             row_id: row_id.into(),
//             row_id_is_new: false,
//             timepoint: timepoint.into(),
//             timepoint_is_new: Default::default(),
//             entity_path: entity_path.into(),
//             entity_path_is_new: false,
//             cells: Default::default(),
//             cached_cell: Default::default(),
//         }
//     }
//
//     pub fn add_cells(mut self, cells: impl IntoIterator<Item = DataCell>) -> Self {
//         self.cells.extend(cells);
//         self
//     }
//
//     // pub fn remove_cells(mut self, cells: impl IntoIterator<Item = DataCell>) -> Self {
//     //     self.cells_removed.extend(cells);
//     //     self
//     // }
//
//     pub fn add_cached_cell(mut self, cell: DataCell) -> Self {
//         self.cached_cell = Some(cell);
//         self
//     }
// }

#[derive(Debug, Clone)]
pub struct RowModification {
    /// The actual cells (== columns, == components).
    pub cells: DataCellVec,
}

#[derive(Debug, Clone, Default)]
pub struct MetadataDiff {
    /// Monotonically increasing ID for insertions.
    pub insert_id: Diff<u64>,

    /// Monotonically increasing ID for queries.
    pub query_id: Diff<u64>,

    /// Monotonically increasing ID for GCs.
    pub gc_id: Diff<u64>,
}

#[derive(Debug, Clone, Default)]
pub enum Diff<T> {
    Added(T),

    Modified {
        before: T,
        after: T,
    },

    Removed(T),

    #[default]
    None,
}

impl<T> Diff<T> {}

// pub struct Diff<T> {
//     before: Option<T>,
// }

#[derive(Debug, Clone)]
pub struct RowDiff {
    // added: BTreeMap,
}

// #[cfg(tests)]
mod tests {
    use ahash::HashMap;
    use re_log_types::{
        example_components::{MyColor, MyPoint, MyPoints},
        DataRow, DataTable, RowId, TableId, Time, TimePoint, Timeline,
    };
    use re_types_core::{components::InstanceKey, Loggable as _};

    use crate::{DataStore, GarbageCollectionOptions};

    use super::*;

    /// A test view that keeps track of what kind of data is logically available in the store, at a
    /// global scope.
    #[derive(Default, Debug, PartialEq, Eq)]
    struct ViewCounters {
        row_ids: BTreeMap<RowId, i64>,
        timelines: BTreeMap<Timeline, i64>,
        entity_paths: BTreeMap<EntityPath, i64>,
        component_names: BTreeMap<ComponentName, i64>,
        times: BTreeMap<TimeInt, i64>,
        timeless: i64,
    }

    impl ViewCounters {
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

    // TODO: we could expose this as a general utility for other view builders
    impl StoreView for ViewCounters {
        fn name(&self) -> String {
            "rerun.testing.store_view.Counters".into()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn on_events(&mut self, events: &[StoreEvent]) {
            for event in events {
                let delta = event.diff.delta;
                *self.row_ids.entry(event.diff.row_id).or_default() += delta;
                *self
                    .entity_paths
                    .entry(event.diff.entity_path.clone())
                    .or_default() += delta;
                *self
                    .component_names
                    .entry(event.diff.component_name)
                    .or_default() += delta;

                if let Some((timeline, time)) = event.diff.timestamp {
                    *self.timelines.entry(timeline).or_default() += delta;
                    *self.times.entry(time).or_default() += delta;
                } else {
                    self.timeless += delta;
                }
            }
        }
    }

    // TODO: gonna have to comment each result or this test is going to be unmaintable
    // TODO: sprinkle in timeless stuff
    #[test]
    fn equilibrium() -> anyhow::Result<()> {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );

        let view_handle = DataStore::register_view(Box::new(ViewCounters::default()));
        // let view = store.register_view(Box::new(ViewCounters::default()));

        fn assert_view(got_handle: StoreViewHandle, expected: &ViewCounters) {
            DataStore::with_view(got_handle, |got| {
                similar_asserts::assert_eq!(expected, got);
            });
        }

        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_temporal("other");
        let timeline_yet_some_more = Timeline::new_sequence("yet_some_more");

        let row_id1 = RowId::random();
        let timepoint1 = TimePoint::from_iter([
            (timeline_frame, 42.into()),        //
            (timeline_other, 666.into()),       //
            (timeline_yet_some_more, 1.into()), //
        ]);
        let entity_path1: EntityPath = "entity_a".into();
        let row1 = DataRow::from_component_batches(
            row_id1,
            timepoint1.clone(),
            entity_path1.clone(),
            [&InstanceKey::from_iter(0..10) as _],
        )?;

        let _events = store.insert_row(&row1);

        assert_view(
            view_handle,
            &ViewCounters::new(
                [
                    (row_id1, 3), //
                ],
                [
                    (timeline_frame, 1),
                    (timeline_other, 1),
                    (timeline_yet_some_more, 1),
                ],
                [
                    (entity_path1.clone(), 3), //
                ],
                [
                    (InstanceKey::name(), 3), //
                ],
                [
                    (42.into(), 1), //
                    (666.into(), 1),
                    (1.into(), 1),
                ],
                0,
            ),
        );

        let row_id2 = RowId::random();
        let timepoint2 = TimePoint::from_iter([
            (timeline_frame, 42.into()),        //
            (timeline_yet_some_more, 1.into()), //
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

        let _events = store.insert_row(&row2);

        assert_view(
            view_handle,
            &ViewCounters::new(
                [
                    (row_id1, 3), //
                    (row_id2, 6),
                ],
                [
                    (timeline_frame, 4),
                    (timeline_other, 1),
                    (timeline_yet_some_more, 4),
                ],
                [
                    (entity_path1.clone(), 3), //
                    (entity_path2.clone(), 6), //
                ],
                [
                    (InstanceKey::name(), 5), //
                    (MyPoint::name(), 2),     //
                    (MyColor::name(), 2),     //
                ],
                [
                    (42.into(), 4), //
                    (666.into(), 1),
                    (1.into(), 4),
                ],
                0,
            ),
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

        let _events = store.insert_row(&row3);

        assert_view(
            view_handle,
            &ViewCounters::new(
                [
                    (row_id1, 3), //
                    (row_id2, 6),
                    (row_id3, 2),
                ],
                [
                    (timeline_frame, 4),
                    (timeline_other, 1),
                    (timeline_yet_some_more, 4),
                ],
                [
                    (entity_path1.clone(), 3), //
                    (entity_path2.clone(), 8), //
                ],
                [
                    (InstanceKey::name(), 6), //
                    (MyPoint::name(), 2),     //
                    (MyColor::name(), 3),     //
                ],
                [
                    (42.into(), 4), //
                    (666.into(), 1),
                    (1.into(), 4),
                ],
                2,
            ),
        );

        store.gc(GarbageCollectionOptions::gc_everything());
        eprintln!("{store}");

        assert_view(
            view_handle,
            &ViewCounters::new(
                [
                    (row_id1, 0), //
                    (row_id2, 0),
                    (row_id3, 0),
                ],
                [
                    (timeline_frame, 0),
                    (timeline_other, 0),
                    (timeline_yet_some_more, 0),
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
        );

        Ok(())
    }
}

// TODO:
// - registry?
// - stats?

// #[cfg(tests)]
// mod tests {
//     use re_log_types::{
//         example_components::MyColor, DataRow, DataTable, RowId, TableId, Time, TimePoint, Timeline,
//     };
//     use re_types_core::{components::InstanceKey, Loggable as _};
//
//     use crate::DataStore;
//
//     use super::*;
//
//     #[test]
//     fn basics() -> anyhow::Result<()> {
//         fn create_row() -> anyhow::Result<(DataRow, CellAddition)> {
//             let row_id = RowId::random();
//             let timepoint = TimePoint::from_iter([(Timeline::log_time(), Time::now().into())]);
//             let entity_path: EntityPath = "entity_a".into();
//
//             let row = DataRow::from_component_batches(
//                 row_id,
//                 timepoint.clone(),
//                 entity_path.clone(),
//                 [&InstanceKey::from_iter(0..10) as _],
//             )?;
//             let modif =
//                 CellAddition::at(row_id, timepoint, entity_path).add_cells(row.cells.clone().0);
//
//             Ok((row, modif))
//         }
//
//         let num_rows = 7;
//
//         let rows: anyhow::Result<Vec<_>> =
//             std::iter::repeat_with(create_row).take(num_rows).collect();
//         let (rows, expected_modifs): (Vec<_>, Vec<_>) = rows?.into_iter().unzip();
//
//         let table = DataTable::from_rows(TableId::random(), rows);
//
//         let mut store = DataStore::new(re_log_types::StoreId::random(re_log_types::StoreKind::Recording), InstanceKey::name(), Default::default());
//         let changelog = store.insert_table(&table)?;
//
//         assert_eq!(expected_modifs, changelog.modifications);
//
//         Ok(())
//     }
//
//     #[test]
//     fn cached_cells() -> anyhow::Result<()> {
//         fn create_row(num_instances: usize) -> anyhow::Result<(DataRow, CellAddition)> {
//             let row_id = RowId::random();
//             let timepoint = TimePoint::from_iter([(Timeline::log_time(), Time::now().into())]);
//             let entity_path: EntityPath = "entity_a".into();
//
//             let colors: Vec<_> = std::iter::repeat_with(|| MyColor::from(0xFF0000FF))
//                 .take(num_instances)
//                 .collect();
//             let row = DataRow::from_component_batches(
//                 row_id,
//                 timepoint.clone(),
//                 entity_path.clone(),
//                 [&colors as _],
//             )?;
//             let modif =
//                 CellAddition::at(row_id, timepoint, entity_path).add_cells(row.cells.clone().0);
//
//             Ok((row, modif))
//         }
//
//         let num_rows = 7;
//
//         let rows: anyhow::Result<Vec<_>> = (0..num_rows).map(create_row).collect();
//         let (rows, mut expected_modifs): (Vec<_>, Vec<_>) = rows?.into_iter().unzip();
//
//         let table = DataTable::from_rows(TableId::random(), rows);
//
//         let mut store = DataStore::new(re_log_types::StoreId::random(re_log_types::StoreKind::Recording), InstanceKey::name(), Default::default());
//         let changelog = store.insert_table(&table)?;
//
//         for (i, modif) in expected_modifs.iter_mut().enumerate() {
//             modif.cached_cell = Some(store.cluster_cell_cache.0[&(i as u32)].clone());
//         }
//
//         assert_eq!(expected_modifs, changelog.modifications);
//
//         Ok(())
//     }
// }
