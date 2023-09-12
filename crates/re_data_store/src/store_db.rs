use std::collections::BTreeMap;

use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, GarbageCollectionOptions, TimeInt};
use re_log_types::{
    ApplicationId, ArrowMsg, ComponentPath, DataCell, DataRow, DataTable, EntityPath,
    EntityPathHash, EntityPathOpMsg, LogMsg, PathOp, RowId, SetStoreInfo, StoreId, StoreInfo,
    StoreKind, TimePoint, Timeline,
};
use re_types::{components::InstanceKey, Loggable as _};

use crate::{Error, TimesPerTimeline};

// ----------------------------------------------------------------------------

/// Stored entities with easy indexing of the paths.
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

    fn register_entity_path(&mut self, entity_path: &EntityPath) {
        self.entity_path_from_hash
            .entry(entity_path.hash())
            .or_insert_with(|| entity_path.clone());
    }

    fn try_add_arrow_msg(&mut self, msg: &ArrowMsg) -> Result<(), Error> {
        re_tracing::profile_function!();

        #[cfg(debug_assertions)]
        check_known_component_schemas(msg);

        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        let mut table = DataTable::from_arrow_msg(msg)?;
        table.compute_all_size_bytes();

        // TODO(cmc): batch all of this
        for row in table.to_rows() {
            self.try_add_data_row(&row)?;
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

        self.register_entity_path(&row.entity_path);

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
                );
                self.data_store.insert_row(&row).ok();

                // Also update the tree with the clear-event
                self.tree.add_data_msg(&time_point, &component_path);
            }
        }

        self.data_store.insert_row(row).map_err(Into::into)
    }

    fn add_path_op(&mut self, row_id: RowId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(row_id, time_point, path_op);

        for component_path in cleared_paths {
            if let Some(data_type) = self
                .data_store
                .lookup_datatype(&component_path.component_name)
            {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let cell =
                    DataCell::from_arrow_empty(component_path.component_name, data_type.clone());

                let row = DataRow::from_cells1(
                    RowId::random(),
                    component_path.entity_path.clone(),
                    time_point.clone(),
                    cell.num_instances(),
                    cell,
                );
                self.data_store.insert_row(&row).ok();
                // Also update the tree with the clear-event
                self.tree.add_data_msg(time_point, &component_path);
            }
        }
    }

    pub fn purge(
        &mut self,
        cutoff_times: &std::collections::BTreeMap<Timeline, TimeInt>,
        drop_row_ids: &ahash::HashSet<RowId>,
    ) {
        re_tracing::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
        } = self;

        {
            re_tracing::profile_scope!("times_per_timeline");
            times_per_timeline.purge(cutoff_times);
        }

        {
            re_tracing::profile_scope!("tree");
            tree.purge(cutoff_times, drop_row_ids);
        }
    }
}

/// Check that known (`rerun.`) components have the expected schemas.
#[cfg(debug_assertions)]
fn check_known_component_schemas(msg: &ArrowMsg) {
    // Check that we have the expected schemas
    let known_fields: ahash::HashMap<&str, &arrow2::datatypes::Field> =
        re_components::iter_registered_field_types()
            .map(|field| (field.name.as_str(), field))
            .collect();

    for actual in &msg.schema.fields {
        if let Some(expected) = known_fields.get(actual.name.as_str()) {
            if let arrow2::datatypes::DataType::List(actual_field) = &actual.data_type {
                // NOTE: Don't care about extensions until the migration is over (arrow2-convert
                // issues).
                let actual_datatype = actual_field.data_type.to_logical_type();
                let expected_datatype = expected.data_type.to_logical_type();
                if actual_datatype != expected_datatype {
                    re_log::warn_once!(
                        "The incoming component {:?} had the type:\n{:#?}\nExpected type:\n{:#?}",
                        actual.name,
                        actual_field.data_type,
                        expected.data_type,
                    );
                }
                if actual.is_nullable != expected.is_nullable {
                    re_log::warn_once!(
                        "The incoming component {:?} has is_nullable={}, expected is_nullable={}",
                        actual.name,
                        actual.is_nullable,
                        expected.is_nullable,
                    );
                }
            } else {
                re_log::warn_once!(
                    "The incoming component {:?} was:\n{:#?}\nExpected:\n{:#?}",
                    actual.name,
                    actual.data_type,
                    expected.data_type,
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
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
    pub entity_db: EntityDb,
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
    pub fn store_mut(&mut self) -> &mut re_arrow_store::DataStore {
        &mut self.entity_db.data_store
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

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();
        assert!((0.0..=1.0).contains(&fraction_to_purge));

        let (drop_row_ids, stats_diff) = self.entity_db.data_store.gc(GarbageCollectionOptions {
            target: re_arrow_store::GarbageCollectionTarget::DropAtLeastFraction(
                fraction_to_purge as _,
            ),
            gc_timeless: false,
            protect_latest: 0,
            purge_empty_tables: false,
        });
        re_log::trace!(
            num_row_ids_dropped = drop_row_ids.len(),
            size_bytes_dropped = re_format::format_bytes(stats_diff.total.num_bytes as _),
            "purged datastore"
        );

        let drop_row_ids: ahash::HashSet<_> = drop_row_ids.into_iter().collect();
        let cutoff_times = self.entity_db.data_store.oldest_time_per_timeline();

        let Self {
            store_id: _,
            entity_op_msgs,
            data_source: _,
            recording_msg: _,
            entity_db,
        } = self;

        {
            re_tracing::profile_scope!("entity_op_msgs");
            entity_op_msgs.retain(|row_id, _| !drop_row_ids.contains(row_id));
        }

        entity_db.purge(&cutoff_times, &drop_row_ids);
    }

    /// Key used for sorting recordings in the UI.
    pub fn sort_key(&self) -> impl Ord + '_ {
        self.store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }
}
