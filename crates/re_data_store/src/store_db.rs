use std::collections::BTreeMap;

use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, GarbageCollectionOptions, StoreEvent, StoreView};
use re_log_types::{
    ApplicationId, ComponentPath, DataCell, DataRow, DataTable, EntityPath, EntityPathHash, LogMsg,
    PathOp, RowId, SetStoreInfo, StoreId, StoreInfo, StoreKind, TimePoint, Timeline,
};
use re_types_core::{components::InstanceKey, Loggable};

use crate::{
    store_views::{TimesPerTimeline, TimesPerTimelineView},
    EntityTreeEvent, Error,
};

// ----------------------------------------------------------------------------

/// Stored entities with easy indexing of the paths.
///
/// NOTE: don't go mutating the contents of this. Use the public functions instead.
// TODO: that is also a non registerable view
pub struct EntityDb {
    /// In many places we just store the hashes, so we need a way to translate back.
    pub entity_path_from_hash: IntMap<EntityPathHash, EntityPath>,

    /// Used for time control
    // TODO
    // TODO: ha, so this one _is_ global
    pub times_per_timeline: TimesPerTimelineView,

    /// A tree-view (split on path components) of the entities.
    pub tree: crate::EntityTree,

    /// Stores all components for all entities for all timelines.
    pub data_store: re_arrow_store::DataStore,
}

impl EntityDb {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            entity_path_from_hash: Default::default(),
            times_per_timeline: Default::default(),
            tree: crate::EntityTree::root(),
            data_store: re_arrow_store::DataStore::new(
                store_id,
                InstanceKey::name(),
                DataStoreConfig::default(),
            ),
        }
    }

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

    // TODO: explain two-passes
    fn add_data_row(&mut self, row: &DataRow) -> Result<(), Error> {
        re_tracing::profile_function!(format!("num_cells={}", row.num_cells()));

        self.register_entity_path(&row.entity_path);

        // First pass: write row to storage, apply resulting store events onto the entity-tree
        // and collect resulting entity-tree events.
        let store_events = self.data_store.insert_row(row)?;
        self.times_per_timeline.on_events(&store_events); // TODO
        let entity_tree_events = self.tree.on_additions(&store_events);

        // Seconds pass: apply entity-tree events onto the store, collect resulting store events
        // then apply store events onto entity-tree one final time.
        let store_events = self.on_entity_tree_events(entity_tree_events)?;
        self.times_per_timeline.on_events(&store_events); // TODO
        let entity_tree_events = self.tree.on_additions(&store_events);

        // There should be no changes in the entity-tree at this point since the whole process
        // should self-stabilize after a single roundtrip.
        debug_assert!(entity_tree_events.is_empty());

        Ok(())
    }

    // TODO
    fn on_entity_tree_events(
        &mut self,
        entity_tree_events: EntityTreeEvent,
    ) -> Result<Vec<StoreEvent>, Error> {
        let mut store_events = Vec::new();

        // Apply partial clears caused by freshly registered clears.
        {
            let mut cells_per_entity_per_row_id =
                BTreeMap::<RowId, BTreeMap<EntityPath, Vec<DataCell>>>::default();
            for (row_id, component_paths) in entity_tree_events.paths_to_clear {
                for component_path in component_paths {
                    if let Some(data_type) = self
                        .data_store
                        .lookup_datatype(&component_path.component_name)
                    {
                        let per_entity = cells_per_entity_per_row_id.entry(row_id).or_default();
                        let cells = per_entity
                            .entry(component_path.entity_path.clone())
                            .or_default();

                        cells.push(DataCell::from_arrow_empty(
                            component_path.component_name,
                            data_type.clone(),
                        ));
                    }
                }
            }

            // Create and insert empty components into the arrow store.
            for (original_row_id, per_entity) in cells_per_entity_per_row_id {
                let timepoint = entity_tree_events
                    .timepoints_to_clear
                    .get(&original_row_id)
                    .cloned()
                    .unwrap_or_default();
                let mut row_id = original_row_id;
                for (entity_path, cells) in per_entity {
                    // NOTE: It is important we insert all those empty components using a single row (id)!
                    // 1. It'll be much more efficient when querying that data back.
                    // 2. Otherwise we will end up with a flaky row ordering, as we have no way to tie-break
                    //    these rows! This flaky ordering will in turn leak through the public
                    //    API (e.g. range queries)!!
                    match DataRow::from_cells(row_id, timepoint.clone(), entity_path, 0, cells) {
                        Ok(row) => {
                            store_events.extend(self.data_store.insert_row(&row)?);
                        }
                        Err(err) => {
                            re_log::error_once!("XXXXXXXXXXXXXXXXX: {err}"); // TODO
                        }
                    }

                    // Don't reuse the same row ID for the next entity!
                    row_id = row_id.next();
                }
            }
        }

        Ok(store_events)
    }

    // fn add_path_op(
    //     &mut self,
    //     row_id: RowId,
    //     time_point: &TimePoint,
    //     path_op: &PathOp,
    // ) -> Result<Vec<StoreEvent>, Error> {
    //     let mut store_events = Vec::new();
    //     let cleared_paths = self.tree.add_path_op(row_id, time_point, path_op);
    //
    //     // NOTE: Btree! We need a stable ordering here!
    //     let mut cells = BTreeMap::<EntityPath, Vec<DataCell>>::default();
    //     for component_path in cleared_paths {
    //         if let Some(data_type) = self
    //             .data_store
    //             .lookup_datatype(&component_path.component_name)
    //         {
    //             let cells = cells
    //                 .entry(component_path.entity_path.clone())
    //                 .or_insert_with(Vec::new);
    //
    //             cells.push(DataCell::from_arrow_empty(
    //                 component_path.component_name,
    //                 data_type.clone(),
    //             ));
    //
    //             // Update the tree with the clear-event.
    //             self.tree.add_data_msg(time_point, &component_path);
    //         }
    //     }
    //
    //     // Create and insert empty components into the arrow store.
    //     let mut row_id = row_id;
    //     for (ent_path, cells) in cells {
    //         // NOTE: It is important we insert all those empty components using a single row (id)!
    //         // 1. It'll be much more efficient when querying that data back.
    //         // 2. Otherwise we will end up with a flaky row ordering, as we have no way to tie-break
    //         //    these rows! This flaky ordering will in turn leak through the public
    //         //    =PI (e.g. range queries)!!
    //         match DataRow::from_cells(row_id, time_point.clone(), ent_path, 0, cells) {
    //             Ok(row) => {
    //                 store_events.extend(self.data_store.insert_row(&row)?);
    //             }
    //             Err(err) => {
    //                 re_log::error_once!("Failed to insert PathOp {path_op:?}: {err}");
    //             }
    //         }
    //
    //         // Don't reuse the same row ID for the next entity!
    //         row_id = row_id.next();
    //     }
    //
    //     Ok(store_events)
    // }

    pub fn purge(&mut self, deleted: &re_arrow_store::Deleted, store_events: &[StoreEvent]) {
        re_tracing::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
        } = self;

        // let mut actually_deleted = Default::default();

        // TODO: let's filter here based on counts?

        tree.purge2(store_events);
        // tree.on_additions(store_events);
        // {
        //     re_tracing::profile_scope!("tree");
        //     tree.purge(deleted, &mut actually_deleted);
        // }

        times_per_timeline.on_events(store_events);

        // TODO
        // {
        //     re_tracing::profile_scope!("times_per_timeline");
        //     for (timeline, times) in actually_deleted.timeful {
        //         if let Some(time_set) = times_per_timeline.get_mut(&timeline) {
        //             for time in times {
        //                 time_set.remove(&time);
        //             }
        //         }
        //     }
        // }
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
}

impl StoreDb {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            store_id: store_id.clone(),
            data_source: None,
            set_store_info: None,
            entity_db: EntityDb::new(store_id),
        }
    }

    /// Helper function to create a recording from a [`StoreInfo`] and some [`DataRow`]s.
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
            store_db.add_data_row(&row)?;
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
        &self.entity_db.times_per_timeline.times
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

        // TODO(cmc): batch all of this
        for row in table.to_rows() {
            self.add_data_row(&row?)?;
        }

        Ok(())
    }

    pub fn add_data_row(&mut self, row: &DataRow) -> Result<(), Error> {
        self.entity_db.add_data_row(row)
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

        let (deleted, store_events, stats_diff) = self.entity_db.data_store.gc(gc_options);
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
        } = self;

        entity_db.purge(&deleted, &store_events);
    }

    /// Key used for sorting recordings in the UI.
    pub fn sort_key(&self) -> impl Ord + '_ {
        self.store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }
}
