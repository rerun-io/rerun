use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, TimeInt};
use re_log_types::{
    component_types::InstanceKey, ArrowMsg, BeginRecordingMsg, Component as _, ComponentPath,
    DataCell, DataRow, DataTable, EntityPath, EntityPathHash, EntityPathOpMsg, LogMsg, MsgId,
    PathOp, RecordingId, RecordingInfo, TimePoint, Timeline,
};

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
                DataStoreConfig {
                    // TODO
                    indexed_bucket_num_rows: 2048,
                    ..Default::default()
                },
            ),
        }
    }
}

impl EntityDb {
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
        let table: DataTable = msg.try_into()?;

        // TODO(#1619): batch all of this
        for row in table.as_rows() {
            self.try_add_data_row(&row)?;
        }

        Ok(())
    }

    fn try_add_data_row(&mut self, row: &DataRow) -> Result<(), Error> {
        for (&timeline, &time_int) in row.timepoint().iter() {
            self.times_per_timeline.insert(timeline, time_int);
        }

        self.register_entity_path(&row.entity_path);

        for cell in row.cells().iter() {
            let component_path =
                ComponentPath::new(row.entity_path().clone(), cell.component_name());
            if cell.component_name() == MsgId::name() {
                continue;
            }
            let pending_clears = self.tree.add_data_msg(row.timepoint(), &component_path);

            for (msg_id, time_point) in pending_clears {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let cell =
                    DataCell::from_arrow_empty(cell.component_name(), cell.datatype().clone());

                let row = DataRow::from_cells1(
                    msg_id,
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

    fn add_path_op(&mut self, msg_id: MsgId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(msg_id, time_point, path_op);

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
                    msg_id,
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
        drop_msg_ids: &ahash::HashSet<MsgId>,
    ) {
        crate::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
        } = self;

        {
            crate::profile_scope!("times_per_timeline");
            times_per_timeline.purge(cutoff_times);
        }

        {
            crate::profile_scope!("tree");
            tree.purge(cutoff_times, drop_msg_ids);
        }
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
#[derive(Default)]
pub struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<MsgId>,
    log_messages: ahash::HashMap<MsgId, LogMsg>,

    /// Data that was logged with [`TimePoint::timeless`].
    /// We need to re-insert those in any new timelines
    /// that are created after they were logged.
    timeless_message_ids: Vec<MsgId>,

    /// Set by whomever created this [`LogDb`].
    pub data_source: Option<re_smart_channel::Source>,

    /// Comes in a special message, [`LogMsg::BeginRecordingMsg`].
    recording_info: Option<RecordingInfo>,

    /// Where we store the entities.
    pub entity_db: EntityDb,
}

impl LogDb {
    pub fn recording_info(&self) -> Option<&RecordingInfo> {
        self.recording_info.as_ref()
    }

    pub fn recording_id(&self) -> RecordingId {
        if let Some(info) = &self.recording_info {
            info.recording_id
        } else {
            RecordingId::ZERO
        }
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

    pub fn is_empty(&self) -> bool {
        self.log_messages.is_empty()
    }

    pub fn add(&mut self, msg: LogMsg) -> Result<(), Error> {
        crate::profile_function!();

        match &msg {
            LogMsg::BeginRecordingMsg(msg) => self.add_begin_recording_msg(msg),
            LogMsg::EntityPathOpMsg(msg) => {
                let EntityPathOpMsg {
                    msg_id,
                    time_point,
                    path_op,
                } = msg;
                self.entity_db.add_path_op(*msg_id, time_point, path_op);
            }
            LogMsg::ArrowMsg(inner) => self.entity_db.try_add_arrow_msg(inner)?,
            LogMsg::Goodbye(_) => {}
        }

        // TODO(#1619): the following only makes sense because, while we support sending and
        // receiving batches, we don't actually do so yet.
        // We need to stop storing raw `LogMsg`s before we can benefit from our batching.
        self.chronological_message_ids.push(msg.id());
        self.log_messages.insert(msg.id(), msg);

        Ok(())
    }

    fn add_begin_recording_msg(&mut self, msg: &BeginRecordingMsg) {
        self.recording_info = Some(msg.info.clone());
    }

    pub fn len(&self) -> usize {
        self.log_messages.len()
    }

    /// In the order they arrived
    pub fn chronological_log_messages(&self) -> impl Iterator<Item = &LogMsg> {
        self.chronological_message_ids
            .iter()
            .filter_map(|id| self.get_log_msg(id))
    }

    pub fn get_log_msg(&self, msg_id: &MsgId) -> Option<&LogMsg> {
        self.log_messages.get(msg_id)
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        crate::profile_function!();
        assert!((0.0..=1.0).contains(&fraction_to_purge));

        // TODO
        let drop_msg_ids: ahash::HashSet<_> = Default::default();
        // let drop_msg_ids = {
        //     let msg_id_chunks = self.entity_db.data_store.gc(
        //         GarbageCollectionTarget::DropAtLeastPercentage(fraction_to_purge as _),
        //         Timeline::log_time(),
        //         MsgId::name(),
        //     );

        //     msg_id_chunks
        //         .iter()
        //         .flat_map(|chunk| {
        //             arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap()
        //         })
        //         .map(Option::unwrap) // MsgId is always present
        //         .collect::<ahash::HashSet<_>>()
        // };

        let cutoff_times = self.entity_db.data_store.oldest_time_per_timeline();

        let Self {
            chronological_message_ids,
            log_messages,
            timeless_message_ids,
            data_source: _,
            recording_info: _,
            entity_db,
        } = self;

        {
            crate::profile_scope!("chronological_message_ids");
            chronological_message_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
        }

        {
            crate::profile_scope!("log_messages");
            log_messages.retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
        }
        {
            crate::profile_scope!("timeless_message_ids");
            timeless_message_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
        }

        entity_db.purge(&cutoff_times, &drop_msg_ids);
    }
}
