use std::collections::BTreeMap;

use ahash::HashSet;
use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, TimeInt};
use re_log_types::{
    component_types::InstanceKey,
    external::arrow2::{chunk::Chunk, datatypes::DataType},
    ArrowMsg, BeginRecordingMsg, Component as _, ComponentPath, DataCell, DataRow, DataTable,
    EntityPath, EntityPathHash, EntityPathOpMsg, LogMsg, PathOp, RecordingId, RecordingInfo, RowId,
    TimePoint, Timeline,
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

    pub datatypes: HashSet<DataType>,
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
            datatypes: Default::default(),
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

    fn try_add_arrow_msg(&mut self, msg: ArrowMsg) -> Result<(), Error> {
        crate::profile_function!();

        let ArrowMsg {
            table_id,
            timepoint_max,
            schema,
            chunk,
        } = msg;

        // TODO: move everything in datatable?
        // TODO: this obviously cannot be doing all these crazy clones
        {
            crate::profile_scope!("collect_datatypes");
            for column in chunk.arrays() {
                collect_datatypes(&mut self.datatypes, &**column);
            }
        }

        let msg = ArrowMsg {
            table_id,
            timepoint_max,
            schema,
            chunk,
        };

        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        let mut table = DataTable::from_arrow_msg(msg, Some(&self.datatypes))?;
        table.compute_all_size_bytes();

        // TODO(#1619): batch all of this
        for row in table.to_rows() {
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
            let pending_clears = self.tree.add_data_msg(row.timepoint(), &component_path);

            for (row_id, time_point) in pending_clears {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let cell =
                    DataCell::from_arrow_empty(cell.component_name(), cell.datatype().clone());

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
                    row_id,
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
        crate::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
            datatypes: _,
        } = self;

        {
            crate::profile_scope!("times_per_timeline");
            times_per_timeline.purge(cutoff_times);
        }

        {
            crate::profile_scope!("tree");
            tree.purge(cutoff_times, drop_row_ids);
        }
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
#[derive(Default)]
pub struct LogDb {
    /// All [`EntityPathOpMsg`]s ever received.
    entity_op_msgs: BTreeMap<RowId, EntityPathOpMsg>,

    /// Set by whomever created this [`LogDb`].
    pub data_source: Option<re_smart_channel::Source>,

    /// Comes in a special message, [`LogMsg::BeginRecordingMsg`].
    recording_msg: Option<BeginRecordingMsg>,

    /// Where we store the entities.
    pub entity_db: EntityDb,
}

impl LogDb {
    pub fn recording_msg(&self) -> Option<&BeginRecordingMsg> {
        self.recording_msg.as_ref()
    }

    pub fn recording_info(&self) -> Option<&RecordingInfo> {
        self.recording_msg().map(|msg| &msg.info)
    }

    pub fn recording_id(&self) -> RecordingId {
        if let Some(msg) = &self.recording_msg {
            msg.info.recording_id
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

    pub fn num_rows(&self) -> usize {
        self.entity_db.data_store.num_timeless_rows() as usize
            + self.entity_db.data_store.num_temporal_rows() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }

    pub fn add(&mut self, msg: LogMsg) -> Result<(), Error> {
        crate::profile_function!();

        match msg {
            LogMsg::BeginRecordingMsg(msg) => self.add_begin_recording_msg(msg),
            LogMsg::EntityPathOpMsg(_, msg) => {
                let EntityPathOpMsg {
                    row_id,
                    time_point,
                    path_op,
                } = &msg;
                self.entity_op_msgs.insert(*row_id, msg.clone());
                self.entity_db.add_path_op(*row_id, time_point, path_op);
            }
            LogMsg::ArrowMsg(_, inner) => self.entity_db.try_add_arrow_msg(inner)?,
            LogMsg::Goodbye(_) => {}
        }

        Ok(())
    }

    fn add_begin_recording_msg(&mut self, msg: BeginRecordingMsg) {
        self.recording_msg = Some(msg);
    }

    /// Returns an iterator over all [`EntityPathOpMsg`]s that have been written to this `LogDb`.
    pub fn iter_entity_op_msgs(&self) -> impl Iterator<Item = &EntityPathOpMsg> {
        self.entity_op_msgs.values()
    }

    pub fn get_entity_op_msg(&self, row_id: &RowId) -> Option<&EntityPathOpMsg> {
        self.entity_op_msgs.get(row_id)
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        crate::profile_function!();
        assert!((0.0..=1.0).contains(&fraction_to_purge));

        let (drop_row_ids, stats_diff) = self.entity_db.data_store.gc(
            re_arrow_store::GarbageCollectionTarget::DropAtLeastFraction(fraction_to_purge as _),
        );
        re_log::debug!(
            num_row_ids_dropped = drop_row_ids.len(),
            size_bytes_dropped = re_format::format_bytes(stats_diff.total.num_bytes as _),
            "purged datastore"
        );

        let drop_row_ids: ahash::HashSet<_> = drop_row_ids.into_iter().collect();
        let cutoff_times = self.entity_db.data_store.oldest_time_per_timeline();

        let Self {
            entity_op_msgs,
            data_source: _,
            recording_msg: _,
            entity_db,
        } = self;

        {
            crate::profile_scope!("entity_op_msgs");
            entity_op_msgs.retain(|row_id, _| !drop_row_ids.contains(row_id));
        }

        entity_db.purge(&cutoff_times, &drop_row_ids);
    }
}

// ---

use itertools::Itertools;
use re_log_types::external::arrow2::{
    array::{
        growable::make_growable, Array, FixedSizeListArray, ListArray, StructArray, UnionArray,
    },
    bitmap::Bitmap,
    datatypes::{Field, UnionMode},
    offset::Offsets,
};
use std::sync::Arc;

pub fn collect_datatypes(datatypes: &mut HashSet<DataType>, array: &dyn Array) {
    crate::profile_function!();

    fn fill(datatypes: &mut HashSet<DataType>, array: &dyn Array) {
        let datatype = array.data_type().clone();

        if !datatypes.insert(datatype.clone()) {
            return;
        }

        match &datatype {
            DataType::List(_) => {
                let array = array.as_any().downcast_ref::<ListArray<i32>>().unwrap();
                fill(datatypes, array.values().as_ref());
            }
            DataType::LargeList(_) => {
                let array = array.as_any().downcast_ref::<ListArray<i64>>().unwrap();
                fill(datatypes, array.values().as_ref());
            }
            DataType::FixedSizeList(_, _) => {
                let array = array.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
                fill(datatypes, array.values().as_ref());
            }
            DataType::Struct(_) => {
                let array = array.as_any().downcast_ref::<StructArray>().unwrap();
                array
                    .values()
                    .iter()
                    .map(|v| fill(datatypes, v.as_ref()))
                    .for_each(|_| {});
            }
            DataType::Union(_, _, _) => {
                let array = array.as_any().downcast_ref::<UnionArray>().unwrap();
                array
                    .fields()
                    .iter()
                    .map(|v| fill(datatypes, v.as_ref()))
                    .for_each(|_| {});
            }
            _ => {}
        }
    }

    fill(datatypes, array);
}

// TODO
// - we shouldnt have to pay for crazy expensive virtual clones... datatype should be overridable
fn dedupe_datatypes(datatypes: &mut HashSet<DataType>, array: &dyn Array) -> Box<dyn Array> {
    crate::profile_function!();

    fn fill(datatypes: &mut HashSet<DataType>, mut array: &dyn Array) -> Box<dyn Array> {
        let datatype = if let Some(datatype) = datatypes.get(array.data_type()) {
            datatype.clone()
        } else {
            let datatype = array.data_type().clone();
            datatypes.insert(datatype.clone());
            datatype
        };

        match array.data_type() {
            DataType::List(_) => {
                let array = array.as_any().downcast_ref::<ListArray<i32>>().unwrap();
                ListArray::<i32>::new(
                    datatype,
                    array.offsets().clone(),
                    fill(datatypes, array.values().as_ref()),
                    array.validity().cloned(),
                )
                .boxed()
            }
            DataType::LargeList(_) => {
                let array = array.as_any().downcast_ref::<ListArray<i64>>().unwrap();
                ListArray::<i64>::new(
                    datatype,
                    array.offsets().clone(),
                    fill(datatypes, array.values().as_ref()),
                    array.validity().cloned(),
                )
                .boxed()
            }
            DataType::FixedSizeList(_, _) => {
                let array = array.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
                FixedSizeListArray::new(
                    datatype,
                    fill(datatypes, array.values().as_ref()),
                    array.validity().cloned(),
                )
                .boxed()
            }
            DataType::Struct(_) => {
                let array = array.as_any().downcast_ref::<StructArray>().unwrap();
                StructArray::new(
                    datatype,
                    array
                        .values()
                        .iter()
                        .map(|v| fill(datatypes, v.as_ref()))
                        .collect_vec(),
                    array.validity().cloned(),
                )
                .boxed()
            }
            DataType::Union(_, _, _) => {
                let array = array.as_any().downcast_ref::<UnionArray>().unwrap();
                UnionArray::new(
                    datatype,
                    array.types().clone(),
                    array
                        .fields()
                        .iter()
                        .map(|v| fill(datatypes, v.as_ref()))
                        .collect_vec(),
                    array.offsets().cloned(),
                )
                .boxed()
            }
            _ => array.to_boxed(),
        }
    }

    fill(datatypes, array)
}
