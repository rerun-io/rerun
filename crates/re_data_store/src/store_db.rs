use std::collections::BTreeMap;

use nohash_hasher::IntMap;

use re_arrow_store::{DataStore, DataStoreConfig, GarbageCollectionOptions};
use re_log_types::{
    ApplicationId, ComponentPath, DataCell, DataRow, DataTable, EntityPath, EntityPathHash, LogMsg,
    PathOp, RowId, SetStoreInfo, StoreId, StoreInfo, StoreKind, TimePoint, Timeline,
};
use re_types_core::{components::InstanceKey, Loggable};

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
    pub data_store: DataStore,
}

impl Default for EntityDb {
    fn default() -> Self {
        Self {
            entity_path_from_hash: Default::default(),
            times_per_timeline: Default::default(),
            tree: crate::EntityTree::root(),
            data_store: DataStore::new(InstanceKey::name(), DataStoreConfig::default()),
        }
    }
}

/// See [`insert_row_with_retries`].
const MAX_INSERT_ROW_ATTEMPTS: usize = 1_000;

/// See [`insert_row_with_retries`].
const DEFAULT_INSERT_ROW_STEP_SIZE: u64 = 100;

/// Inserts a [`DataRow`] into the [`DataStore`], retrying in case of duplicated `RowId`s.
///
/// Retries a maximum of `num_attempts` times if the row couldn't be inserted because of a
/// duplicated [`RowId`], bumping the [`RowId`]'s internal counter by `step_size` between attempts.
///
/// Returns the actual [`DataRow`] that was successfully inserted, if any.
///
/// The default value of `num_attempts` (see [`MAX_INSERT_ROW_ATTEMPTS`]) should be (way) more than
/// enough for all valid use cases.
///
/// When using this function, please add a comment explaining the rationale.
fn insert_row_with_retries(
    store: &mut DataStore,
    mut row: DataRow,
    num_attempts: usize,
    step_size: u64,
) -> re_arrow_store::WriteResult<DataRow> {
    fn random_u64() -> u64 {
        let mut bytes = [0_u8; 8];
        getrandom::getrandom(&mut bytes).map_or(0, |_| u64::from_le_bytes(bytes))
    }

    for _ in 0..num_attempts {
        match store.insert_row(&row) {
            Ok(_) => return Ok(row),
            Err(re_arrow_store::WriteError::ReusedRowId(_)) => {
                re_log::warn!(row_id = %row.row_id(), "Found duplicated RowId, retrying…");
                row.row_id = row.row_id.increment(random_u64() % step_size + 1);
            }
            Err(err) => return Err(err),
        }
    }

    Err(re_arrow_store::WriteError::ReusedRowId(row.row_id()))
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

    /// Returns the [`DataRow`] that was inserted. It might have been modified!
    //
    // TODO(#374): Updates of secondary datastructures should be the result of subscribing to the
    // datastore's changelog and reacting to these changes appropriately. We shouldn't be creating
    // many sources of truth.
    fn add_data_row(&mut self, row: DataRow) -> Result<DataRow, Error> {
        // ## RowId duplication
        //
        // We shouldn't be attempting to retry in this instance: a duplicated RowId at this stage
        // is likely a user error.
        //
        // We only do so because, the way our 'save' feature is currently implemented in the
        // viewer can result in a single row's worth of data to be split across several insertions
        // when loading that data back (because we dump per-bucket, and RowIds get duplicated
        // across buckets).
        //
        // TODO(#1894): Remove this once the save/load process becomes RowId-driven.
        let row = insert_row_with_retries(
            &mut self.data_store,
            row,
            MAX_INSERT_ROW_ATTEMPTS,
            DEFAULT_INSERT_ROW_STEP_SIZE,
        )?;

        self.register_entity_path(&row.entity_path);

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

                let row = DataRow::from_cells1(
                    row_id.next(), // see comment below
                    row.entity_path.clone(),
                    time_point.clone(),
                    cell.num_instances(),
                    cell,
                )?;

                // ## RowId duplication
                //
                // We are inserting new data (empty cells) with an old RowId (specifically, the RowId
                // of the original insertion that was used to register the pending clear in the first
                // place).
                // By definition, this is illegal: RowIds are unique.
                //
                // On the other hand, the GC process is driven by RowId order, which means we must make
                // sure that the empty cell we're inserting uses a RowId with a similar timestamp as the
                // one used in the original Clear component cell, so they roughly get GC'd at the same time.
                if let Err(err) = insert_row_with_retries(
                    &mut self.data_store,
                    row,
                    MAX_INSERT_ROW_ATTEMPTS,
                    DEFAULT_INSERT_ROW_STEP_SIZE,
                ) {
                    re_log::error!(%err, "Failed to insert pending clear cell");
                }

                // Also update the tree with the clear-event
                self.tree.add_data_msg(&time_point, &component_path);
            }
        }

        // Look for a `ClearIsRecursive` component, and if it's there, go through the clear path
        // instead.
        use re_types_core::components::ClearIsRecursive;
        if let Some(idx) = row.find_cell(&ClearIsRecursive::name()) {
            let cell = &row.cells()[idx];
            let settings = cell.try_to_native_mono::<ClearIsRecursive>().unwrap();
            let path_op = if settings.map_or(false, |s| s.0) {
                PathOp::ClearRecursive(row.entity_path.clone())
            } else {
                PathOp::ClearComponents(row.entity_path.clone())
            };
            self.add_path_op(row.row_id(), row.timepoint(), &path_op);
        }

        Ok(row)
    }

    fn add_path_op(&mut self, mut row_id: RowId, time_point: &TimePoint, path_op: &PathOp) {
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

        row_id = row_id.next(); // see comment below

        // Create and insert empty components into the arrow store.
        for (ent_path, cells) in cells {
            // NOTE: It is important we insert all those empty components using a single row (id)!
            // 1. It'll be much more efficient when querying that data back.
            // 2. Otherwise we will end up with a flaky row ordering, as we have no way to tie-break
            //    these rows! This flaky ordering will in turn leak through the public
            //    API (e.g. range queries)!!
            row_id = match DataRow::from_cells(row_id, time_point.clone(), ent_path, 0, cells) {
                Ok(row) => {
                    // ## RowId duplication
                    //
                    // We are inserting new data (empty cells) with an already used RowId (specifically,
                    // the RowId of the row containing the clear cell itself).
                    // By definition, this is illegal: RowIds are unique.
                    //
                    // On the other hand, the GC process is driven by RowId order, which means we must make
                    // sure that the empty cell we're inserting uses a RowId with a similar timestamp as the
                    // one used in the original Clear component cell, so they roughly get GC'd at the same time.
                    match insert_row_with_retries(
                        &mut self.data_store,
                        row,
                        MAX_INSERT_ROW_ATTEMPTS,
                        DEFAULT_INSERT_ROW_STEP_SIZE,
                    ) {
                        Ok(row) => row.row_id(),
                        Err(err) => {
                            re_log::error!(%err, ?path_op, "Failed to insert PathOp");
                            row_id
                        }
                    }
                }
                Err(err) => {
                    re_log::error!(%err, ?path_op, "Failed to insert PathOp");
                    row_id
                }
            };

            row_id = row_id.next(); // see comment above
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

/// An in-memory database built from a stream of [`LogMsg`]es.
///
/// NOTE: all mutation is to be done via public functions!
pub struct StoreDb {
    /// The [`StoreId`] for this log.
    store_id: StoreId,

    /// Set by whomever created this [`StoreDb`].
    pub data_source: Option<re_smart_channel::SmartChannelSource>,

    /// Comes in a special message, [`LogMsg::SetStoreInfo`].
    set_store_info: Option<SetStoreInfo>,

    /// Where we store the entities.
    entity_db: EntityDb,

    /// Keeps track of the last time data was inserted into this store (viewer wall-clock).
    last_modified_at: web_time::Instant,
}

impl StoreDb {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            store_id,
            data_source: None,
            set_store_info: None,
            entity_db: Default::default(),
            last_modified_at: web_time::Instant::now(),
        }
    }

    /// Helper function to create a recording from a [`StoreInfo`] and a some [`DataRow`]s.
    ///
    /// This is useful to programmatically create recordings from within the viewer, which cannot
    /// use the `re_sdk`, which is not Wasm-compatible.
    pub fn from_info_and_rows(
        store_info: StoreInfo,
        rows: impl IntoIterator<Item = DataRow>,
    ) -> Result<Self, Error> {
        let mut store_db = StoreDb::new(store_info.store_id.clone());

        store_db.set_store_info(SetStoreInfo {
            row_id: RowId::random(),
            info: store_info,
        });
        for row in rows {
            store_db.add_data_row(row)?;
        }

        Ok(store_db)
    }

    #[inline]
    pub fn entity_db(&self) -> &EntityDb {
        &self.entity_db
    }

    pub fn store_info_msg(&self) -> Option<&SetStoreInfo> {
        self.set_store_info.as_ref()
    }

    pub fn store_info(&self) -> Option<&StoreInfo> {
        self.store_info_msg().map(|msg| &msg.info)
    }

    pub fn app_id(&self) -> Option<&ApplicationId> {
        self.store_info().map(|ri| &ri.application_id)
    }

    #[inline]
    pub fn store(&self) -> &DataStore {
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

    pub fn time_histogram(&self, timeline: &Timeline) -> Option<&crate::TimeHistogram> {
        self.entity_db().tree.prefix_times.get(timeline)
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

    pub fn last_modified_at(&self) -> web_time::Instant {
        self.last_modified_at
    }

    pub fn is_empty(&self) -> bool {
        self.set_store_info.is_none() && self.num_rows() == 0
    }

    pub fn add(&mut self, msg: &LogMsg) -> Result<(), Error> {
        re_tracing::profile_function!();

        debug_assert_eq!(msg.store_id(), self.store_id());

        match &msg {
            LogMsg::SetStoreInfo(msg) => self.set_store_info(msg.clone()),

            LogMsg::ArrowMsg(_, arrow_msg) => {
                let table = DataTable::from_arrow_msg(arrow_msg)?;
                self.add_data_table(table)?;
            }
        }

        Ok(())
    }

    pub fn add_data_table(&mut self, mut table: DataTable) -> Result<(), Error> {
        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        table.compute_all_size_bytes();

        for row in table.to_rows() {
            self.add_data_row(row?)?;
        }

        self.last_modified_at = web_time::Instant::now();

        Ok(())
    }

    pub fn add_data_row(&mut self, row: DataRow) -> Result<(), Error> {
        self.entity_db.add_data_row(row).map(|_| ())
    }

    pub fn set_store_info(&mut self, store_info: SetStoreInfo) {
        self.set_store_info = Some(store_info);
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
            data_source: _,
            set_store_info: _,
            entity_db,
            last_modified_at: _,
        } = self;

        entity_db.purge(&deleted);
    }

    /// Key used for sorting recordings in the UI.
    pub fn sort_key(&self) -> impl Ord + '_ {
        self.store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }
}
