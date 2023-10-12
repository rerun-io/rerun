use std::collections::BTreeMap;

use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, GarbageCollectionOptions};
use re_log_types::{
    ApplicationId, ArrowMsg, ComponentPath, DataCell, DataRow, DataTable, EntityPath,
    EntityPathHash, EntityPathOpMsg, LogMsg, PathOp, RowId, SetStoreInfo, StoreId, StoreInfo,
    StoreKind, TimePoint, Timeline,
};
use re_types::{components::InstanceKey, Loggable as _};

use crate::{Error, TimesPerTimeline};

// ----------------------------------------------------------------------------

/// Stored entities with easy indexing of the paths.
///
/// NOTE: don't go mutating the contents of this. Use the public functions instead.
pub struct EntityDb {
    /// In many places we just store the hashes, so we need a way to translate back.
    pub entity_path_from_hash: IntMap<EntityPathHash, EntityPath>,

    /// Used for time control
    pub times_per_timeline: TimesPerTimeline,

    /// A tree-view (split on path components) of the entities.
    pub tree: crate::EntityTree,

    /// Stores all components for all entities for all timelines.
    pub data_store: re_arrow_store::DataStore,
}

impl Default for EntityDb {
    fn default() -> Self {
        Self {
            entity_path_from_hash: Default::default(),
            times_per_timeline: Default::default(),
            tree: crate::EntityTree::root(),
            data_store: re_arrow_store::DataStore::new(
                InstanceKey::name(),
                DataStoreConfig::default(),
            ),
        }
    }
}

impl EntityDb {
    /// A sorted list of all the entity paths in this database.
    pub fn entity_paths(&self) -> Vec<&EntityPath> {
        use itertools::Itertools as _;
        self.entity_path_from_hash.values().sorted().collect()
    }

    #[inline]
    pub fn entity_path_from_hash(&self, entity_path_hash: &EntityPathHash) -> Option<&EntityPath> {
        self.entity_path_from_hash.get(entity_path_hash)
    }

    /// Returns `true` also for entities higher up in the hierarchy.
    #[inline]
    pub fn is_known_entity(&self, entity_path: &EntityPath) -> bool {
        self.tree.subtree(entity_path).is_some()
    }

    /// If you log `world/points`, then that is a logged entity, but `world` is not,
    /// unless you log something to `world` too.
    #[inline]
    pub fn is_logged_entity(&self, entity_path: &EntityPath) -> bool {
        self.entity_path_from_hash.contains_key(&entity_path.hash())
    }

    fn register_entity_path(&mut self, entity_path: &EntityPath) {
        self.entity_path_from_hash
            .entry(entity_path.hash())
            .or_insert_with(|| entity_path.clone());
    }

    fn try_add_arrow_msg(&mut self, msg: &ArrowMsg) -> Result<(), Error> {
        re_tracing::profile_function!();

        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        let mut table = DataTable::from_arrow_msg(msg)?;
        table.compute_all_size_bytes();

        // TODO(cmc): batch all of this
        for row in table.to_rows() {
            let row = row?;

            self.register_entity_path(&row.entity_path);

            self.try_add_data_row(&row)?;

            // Look for a `ClearIsRecursive` component, and if it's there, go through the clear path
            // instead.
            use re_types::components::ClearIsRecursive;
            if let Some(idx) = row.find_cell(&ClearIsRecursive::name()) {
                let cell = &row.cells()[idx];
                let settings = cell.try_to_native_mono::<ClearIsRecursive>().unwrap();
                let path_op = if settings.map_or(false, |s| s.0) {
                    PathOp::ClearRecursive(row.entity_path.clone())
                } else {
                    PathOp::ClearComponents(row.entity_path.clone())
                };
                // NOTE: We've just added the row itself, so make sure to bump the row ID already!
                self.add_path_op(row.row_id().next(), row.timepoint(), &path_op);
            }
        }

        Ok(())
    }

    // TODO(jleibs): If this shouldn't be public, chain together other setters
    // TODO(cmc): Updates of secondary datastructures should be the result of subscribing to the
    // datastore's changelog and reacting to these changes appropriately. We shouldn't be creating
    // many sources of truth.
    pub fn try_add_data_row(&mut self, row: &DataRow) -> Result<(), Error> {
        for (&timeline, &time_int) in row.timepoint().iter() {
            self.times_per_timeline.insert(timeline, time_int);
        }

        for cell in row.cells().iter() {
            let component_path =
                ComponentPath::new(row.entity_path().clone(), cell.component_name());
            let pending_clears = self.tree.add_data_msg(row.timepoint(), &component_path);

            for (row_id, time_point) in pending_clears {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let cell =
                    DataCell::from_arrow_empty(cell.component_name(), cell.datatype().clone());

                // NOTE(cmc): The fact that this inserts data to multiple entity paths using a
                // single `RowId` is… interesting. Keep it in mind.
                let row = DataRow::from_cells1(
                    row_id,
                    row.entity_path.clone(),
                    time_point.clone(),
                    cell.num_instances(),
                    cell,
                )?;
                self.data_store.insert_row(&row).ok();

                // Also update the tree with the clear-event
                self.tree.add_data_msg(&time_point, &component_path);
            }
        }

        self.data_store.insert_row(row).map_err(Into::into)
    }

    fn add_path_op(&mut self, row_id: RowId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(row_id, time_point, path_op);

        // NOTE: Btree! We need a stable ordering here!
        let mut cells = BTreeMap::<EntityPath, Vec<DataCell>>::default();
        for component_path in cleared_paths {
            if let Some(data_type) = self
                .data_store
                .lookup_datatype(&component_path.component_name)
            {
                let cells = cells
                    .entry(component_path.entity_path.clone())
                    .or_insert_with(Vec::new);

                cells.push(DataCell::from_arrow_empty(
                    component_path.component_name,
                    data_type.clone(),
                ));

                // Update the tree with the clear-event.
                self.tree.add_data_msg(time_point, &component_path);
            }
        }

        // Create and insert empty components into the arrow store.
        let mut row_id = row_id;
        for (ent_path, cells) in cells {
            // NOTE: It is important we insert all those empty components using a single row (id)!
            // 1. It'll be much more efficient when querying that data back.
            // 2. Otherwise we will end up with a flaky row ordering, as we have no way to tie-break
            //    these rows! This flaky ordering will in turn leak through the public
            //    API (e.g. range queries)!!
            match DataRow::from_cells(row_id, time_point.clone(), ent_path, 0, cells) {
                Ok(row) => {
                    self.data_store.insert_row(&row).ok();
                }
                Err(err) => {
                    re_log::error_once!("Failed to insert PathOp {path_op:?}: {err}");
                }
            }

            // Don't reuse the same row ID for the next entity!
            row_id = row_id.next();
        }
    }

    pub fn purge(&mut self, deleted: &re_arrow_store::Deleted) {
        re_tracing::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
        } = self;

        let mut actually_deleted = Default::default();

        {
            re_tracing::profile_scope!("tree");
            tree.purge(deleted, &mut actually_deleted);
        }

        {
            re_tracing::profile_scope!("times_per_timeline");
            for (timeline, times) in actually_deleted.timeful {
                if let Some(time_set) = times_per_timeline.get_mut(&timeline) {
                    for time in times {
                        time_set.remove(&time);
                    }
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
///
/// NOTE: all mutation is to be done via public functions!
pub struct StoreDb {
    /// The [`StoreId`] for this log.
    store_id: StoreId,

    /// All [`EntityPathOpMsg`]s ever received.
    entity_op_msgs: BTreeMap<RowId, EntityPathOpMsg>,

    /// Set by whomever created this [`StoreDb`].
    pub data_source: Option<re_smart_channel::SmartChannelSource>,

    /// Comes in a special message, [`LogMsg::SetStoreInfo`].
    recording_msg: Option<SetStoreInfo>,

    /// Where we store the entities.
    pub entity_db: EntityDb, // TODO(emilk): remove `pub`
}

impl StoreDb {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            store_id,
            entity_op_msgs: Default::default(),
            data_source: None,
            recording_msg: None,
            entity_db: Default::default(),
        }
    }

    #[inline]
    pub fn entity_db(&self) -> &EntityDb {
        &self.entity_db
    }

    pub fn recording_msg(&self) -> Option<&SetStoreInfo> {
        self.recording_msg.as_ref()
    }

    pub fn store_info(&self) -> Option<&StoreInfo> {
        self.recording_msg().map(|msg| &msg.info)
    }

    pub fn app_id(&self) -> Option<&ApplicationId> {
        self.store_info().map(|ri| &ri.application_id)
    }

    #[inline]
    pub fn store(&self) -> &re_arrow_store::DataStore {
        &self.entity_db.data_store
    }

    pub fn store_kind(&self) -> StoreKind {
        self.store_id.kind
    }

    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.times_per_timeline().timelines()
    }

    pub fn times_per_timeline(&self) -> &TimesPerTimeline {
        &self.entity_db.times_per_timeline
    }

    pub fn num_timeless_messages(&self) -> usize {
        self.entity_db.tree.num_timeless_messages()
    }

    pub fn num_rows(&self) -> usize {
        self.entity_db.data_store.num_timeless_rows() as usize
            + self.entity_db.data_store.num_temporal_rows() as usize
    }

    /// Return the current `StoreGeneration`. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    pub fn generation(&self) -> re_arrow_store::StoreGeneration {
        self.entity_db.data_store.generation()
    }

    pub fn is_empty(&self) -> bool {
        self.recording_msg.is_none() && self.num_rows() == 0
    }

    pub fn add(&mut self, msg: &LogMsg) -> Result<(), Error> {
        re_tracing::profile_function!();

        debug_assert_eq!(msg.store_id(), self.store_id());

        match &msg {
            LogMsg::SetStoreInfo(msg) => self.add_begin_recording_msg(msg),
            LogMsg::EntityPathOpMsg(_, msg) => {
                let EntityPathOpMsg {
                    row_id,
                    time_point,
                    path_op,
                } = msg;
                self.entity_op_msgs.insert(*row_id, msg.clone());
                self.entity_db.add_path_op(*row_id, time_point, path_op);
            }
            LogMsg::ArrowMsg(_, inner) => self.entity_db.try_add_arrow_msg(inner)?,
        }

        Ok(())
    }

    pub fn add_begin_recording_msg(&mut self, msg: &SetStoreInfo) {
        self.recording_msg = Some(msg.clone());
    }

    /// Returns an iterator over all [`EntityPathOpMsg`]s that have been written to this `StoreDb`.
    pub fn iter_entity_op_msgs(&self) -> impl Iterator<Item = &EntityPathOpMsg> {
        self.entity_op_msgs.values()
    }

    pub fn get_entity_op_msg(&self, row_id: &RowId) -> Option<&EntityPathOpMsg> {
        self.entity_op_msgs.get(row_id)
    }

    pub fn gc_everything_but_the_latest_row(&mut self) {
        re_tracing::profile_function!();

        self.gc(GarbageCollectionOptions {
            target: re_arrow_store::GarbageCollectionTarget::Everything,
            gc_timeless: true,
            protect_latest: 1, // TODO(jleibs): Bump this after we have an undo buffer
            purge_empty_tables: true,
        });
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        assert!((0.0..=1.0).contains(&fraction_to_purge));
        self.gc(GarbageCollectionOptions {
            target: re_arrow_store::GarbageCollectionTarget::DropAtLeastFraction(
                fraction_to_purge as _,
            ),
            gc_timeless: true,
            protect_latest: 1,
            purge_empty_tables: false,
        });
    }

    pub fn gc(&mut self, gc_options: GarbageCollectionOptions) {
        re_tracing::profile_function!();

        let (deleted, stats_diff) = self.entity_db.data_store.gc(gc_options);
        re_log::trace!(
            num_row_ids_dropped = deleted.row_ids.len(),
            size_bytes_dropped = re_format::format_bytes(stats_diff.total.num_bytes as _),
            "purged datastore"
        );

        let Self {
            store_id: _,
            entity_op_msgs,
            data_source: _,
            recording_msg: _,
            entity_db,
        } = self;

        {
            re_tracing::profile_scope!("entity_op_msgs");
            entity_op_msgs.retain(|row_id, _| !deleted.row_ids.contains(row_id));
        }

        entity_db.purge(&deleted);
    }

    /// Key used for sorting recordings in the UI.
    pub fn sort_key(&self) -> impl Ord + '_ {
        self.store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }
}
