use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};

use re_arrow_store::{DataStoreConfig, GarbageCollectionTarget, TimeType};
use re_log::warn_once;
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    field_types::{Instance, Scalar, TextEntry},
    msg_bundle::{Component as _, ComponentBundle, MsgBundle},
    objects, ArrowMsg, BatchIndex, BeginRecordingMsg, DataMsg, DataPath, DataVec, FieldOrComponent,
    LogMsg, LoggedData, MsgId, ObjPath, ObjPathHash, ObjTypePath, ObjectType, PathOp, PathOpMsg,
    RecordingId, RecordingInfo, TimeInt, TimePoint, Timeline, TypeMsg,
};

use crate::{Error, TimesPerTimeline};

// ----------------------------------------------------------------------------

/// Stored objects and their types, with easy indexing of the paths.
pub struct ObjDb {
    /// The types of all the objects.
    /// Must be registered before adding them.
    pub types: IntMap<ObjTypePath, ObjectType>,

    /// In many places we just store the hashes, so we need a way to translate back.
    pub obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,

    /// A tree-view (split on path components) of the objects.
    pub tree: crate::ObjectTree,

    /// The old store of data. Being deprecated.
    pub store: crate::DataStore,

    /// The arrow store of data.
    pub arrow_store: re_arrow_store::DataStore,
}

impl Default for ObjDb {
    fn default() -> Self {
        Self {
            types: Default::default(),
            obj_path_from_hash: Default::default(),
            tree: crate::ObjectTree::root(),
            store: Default::default(),
            arrow_store: re_arrow_store::DataStore::new(
                Instance::name(),
                DataStoreConfig {
                    component_bucket_size_bytes: 1024 * 1024, // 1 MiB
                    index_bucket_size_bytes: 1024,            // 1KiB
                    ..Default::default()
                },
            ),
        }
    }
}

impl ObjDb {
    #[inline]
    pub fn obj_path_from_hash(&self, obj_path_hash: &ObjPathHash) -> Option<&ObjPath> {
        self.obj_path_from_hash.get(obj_path_hash)
    }

    fn register_obj_path(&mut self, obj_path: &ObjPath) {
        self.obj_path_from_hash
            .entry(*obj_path.hash())
            .or_insert_with(|| obj_path.clone());
    }

    fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) {
        // Validate:
        {
            let obj_type_path = &data_path.obj_path.obj_type_path();
            let field_name = &data_path.field_name;

            let is_meta_field = re_log_types::objects::META_FIELDS.contains(&field_name.as_str());
            if !is_meta_field {
                if let Some(obj_type) = self.types.get(obj_type_path) {
                    let valid_members = obj_type.members();
                    if !valid_members.contains(&field_name.as_str()) {
                        re_log::warn_once!(
                            "Logged to {obj_type_path}.{field_name}, but the parent object ({obj_type:?}) does not have that field. Expected one of: {}",
                            valid_members.iter().format(", ")
                        );
                    }
                } else {
                    re_log::warn_once!(
                        "Logging to {obj_type_path}.{field_name} without first registering object type"
                    );
                }
            }
        }

        self.register_obj_path(&data_path.obj_path);

        if let Err(err) = self.store.insert_data(msg_id, time_point, data_path, data) {
            re_log::warn!("Failed to add data to data_store: {err:?}");
        }

        let pending_clears = self
            .tree
            .add_data_msg(msg_id, time_point, data_path, Some(data));

        // Since we now know the type, we can retroactively add any collected nulls at the correct timestamps
        for (msg_id, time_point) in pending_clears {
            if !objects::META_FIELDS.contains(&data_path.field_name.as_str()) {
                // TODO(jleibs) After we reconcile Mono & Multi objects this can be simplified to just use Null
                match data {
                    LoggedData::Null(_) | LoggedData::Single(_) => {
                        self.add_data_msg(
                            msg_id,
                            &time_point,
                            data_path,
                            &LoggedData::Null(data.data_type()),
                        );
                    }
                    LoggedData::Batch { .. } | LoggedData::BatchSplat(_) => {
                        self.add_data_msg(
                            msg_id,
                            &time_point,
                            data_path,
                            &LoggedData::Batch {
                                indices: BatchIndex::SequentialIndex(0),
                                data: DataVec::empty_from_data_type(data.data_type()),
                            },
                        );
                    }
                };
            }
        }
    }

    fn try_add_arrow_data_msg(&mut self, msg: &ArrowMsg) -> Result<(), Error> {
        let msg_bundle = MsgBundle::try_from(msg).map_err(Error::MsgBundleError)?;

        // Determine the kind of object we're looking at based on the components that have been
        // uploaded _first_.
        //
        // TODO(cmc): That's an extension of the hack below, and will disappear at the same time
        // and for the same reasons.
        {
            let components = msg_bundle
                .components
                .iter()
                .map(|bundle| bundle.name)
                .collect::<IntSet<_>>();

            let obj_type = if components.contains(&TextEntry::name()) {
                ObjectType::TextEntry
            } else if components.contains(&Scalar::name()) {
                ObjectType::Scalar
            } else {
                // TODO(jleibs): Hack in a type so the UI treats these objects as visible
                // This can go away once we determine object categories directly from the arrow
                // table
                ObjectType::ArrowObject
            };
            self.types
                .entry(msg_bundle.obj_path.obj_type_path().clone())
                .or_insert(obj_type);
        }

        self.register_obj_path(&msg_bundle.obj_path);

        for component in &msg_bundle.components {
            let data_path = DataPath::new_arrow(msg_bundle.obj_path.clone(), component.name);
            if component.name == MsgId::name() {
                continue;
            }
            let pending_clears =
                self.tree
                    .add_data_msg(msg.msg_id, &msg_bundle.time_point, &data_path, None);

            for (msg_id, time_point) in pending_clears {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let bundle =
                    ComponentBundle::new_empty(component.name, component.data_type().clone());
                let msg_bundle = MsgBundle::new(
                    msg_id,
                    msg_bundle.obj_path.clone(),
                    time_point.clone(),
                    vec![bundle],
                );
                self.arrow_store.insert(&msg_bundle).ok();

                // Also update the object tree with the clear-event
                self.tree
                    .add_data_msg(msg_id, &time_point, &data_path, None);
            }
        }

        self.arrow_store.insert(&msg_bundle).map_err(Into::into)
    }

    fn add_path_op(&mut self, msg_id: MsgId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(msg_id, time_point, path_op);

        for (data_path, data_type, mono_or_multi) in cleared_paths {
            if data_path.is_arrow() {
                if let FieldOrComponent::Component(component) = data_path.field_name {
                    if let Some(data_type) = self.arrow_store.lookup_data_type(&component) {
                        // Create and insert an empty component into the arrow store
                        // TODO(jleibs): Faster empty-array creation
                        let bundle = ComponentBundle::new_empty(component, data_type.clone());
                        let msg_bundle = MsgBundle::new(
                            msg_id,
                            data_path.obj_path.clone(),
                            time_point.clone(),
                            vec![bundle],
                        );
                        self.arrow_store.insert(&msg_bundle).ok();
                        // Also update the object tree with the clear-event
                        self.tree.add_data_msg(msg_id, time_point, &data_path, None);
                    }
                }
            } else if !objects::META_FIELDS.contains(&data_path.field_name.as_str()) {
                match mono_or_multi {
                    crate::MonoOrMulti::Mono => {
                        self.add_data_msg(
                            msg_id,
                            time_point,
                            &data_path,
                            &LoggedData::Null(data_type),
                        );
                    }
                    crate::MonoOrMulti::Multi => {
                        self.add_data_msg(
                            msg_id,
                            time_point,
                            &data_path,
                            &LoggedData::Batch {
                                indices: BatchIndex::SequentialIndex(0),
                                data: DataVec::empty_from_data_type(data_type),
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn retain(
        &mut self,
        keep_msg_ids: Option<&ahash::HashSet<MsgId>>,
        drop_msg_ids: Option<&ahash::HashSet<MsgId>>,
    ) {
        crate::profile_function!();

        let Self {
            types: _,
            obj_path_from_hash: _,
            tree,
            store,
            arrow_store: _,
        } = self;

        {
            crate::profile_scope!("tree");
            tree.retain(keep_msg_ids, drop_msg_ids);
        }

        store.retain(keep_msg_ids, drop_msg_ids);
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

    recording_info: Option<RecordingInfo>,

    /// Where we store the objects.
    pub obj_db: ObjDb,
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
        &self.obj_db.tree.prefix_times
    }

    pub fn is_empty(&self) -> bool {
        self.log_messages.is_empty()
    }

    pub fn add(&mut self, msg: LogMsg) -> Result<(), Error> {
        crate::profile_function!();
        match &msg {
            LogMsg::BeginRecordingMsg(msg) => self.add_begin_recording_msg(msg),
            LogMsg::TypeMsg(msg) => self.add_type_msg(msg),
            LogMsg::DataMsg(msg) => {
                let DataMsg {
                    msg_id,
                    time_point,
                    data_path,
                    data,
                } = msg;
                self.add_data_msg(*msg_id, time_point, data_path, data);
            }
            LogMsg::PathOpMsg(msg) => {
                let PathOpMsg {
                    msg_id,
                    time_point,
                    path_op,
                } = msg;
                self.obj_db.add_path_op(*msg_id, time_point, path_op);
            }
            LogMsg::ArrowMsg(msg) => {
                self.obj_db.try_add_arrow_data_msg(msg)?;
            }
        }
        self.chronological_message_ids.push(msg.id());
        self.log_messages.insert(msg.id(), msg);
        Ok(())
    }

    fn add_begin_recording_msg(&mut self, msg: &BeginRecordingMsg) {
        self.recording_info = Some(msg.info.clone());
    }

    fn add_type_msg(&mut self, msg: &TypeMsg) {
        let previous_value = self
            .obj_db
            .types
            .insert(msg.type_path.clone(), msg.obj_type);

        if let Some(previous_value) = previous_value {
            if previous_value != msg.obj_type {
                re_log::warn!(
                    "Object {} changed type from {:?} to {:?}",
                    msg.type_path,
                    previous_value,
                    msg.obj_type
                );
            }
        } else {
            re_log::debug!(
                "Registered object type {}: {:?}",
                msg.type_path,
                msg.obj_type
            );
        }
    }

    fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) {
        crate::profile_function!();

        if time_point.is_timeless() {
            // Timeless data should be added to all existing timelines,
            // as well to all future timelines, so we special-case it here.
            // See https://linear.app/rerun/issue/PRO-97

            // Remember to add it to future timelines:
            self.timeless_message_ids.push(msg_id);

            let has_any_timelines = self.timelines().next().is_some();
            if has_any_timelines {
                // Add to existing timelines (if any):
                let mut time_point = TimePoint::default();
                for &timeline in self.timelines() {
                    time_point.insert(timeline, TimeInt::BEGINNING);
                }
                self.add_data_msg(msg_id, &time_point, data_path, data);
            }
        } else {
            // Not timeless data.

            // First check if this data message adds a new timeline…
            let mut new_timelines = TimePoint::default();
            for timeline in time_point.timelines() {
                let is_new_timeline = self.times_per_timeline().get(timeline).is_none();
                if is_new_timeline {
                    re_log::debug!("New timeline added: {timeline:?}");
                    new_timelines.insert(*timeline, TimeInt::BEGINNING);
                }
            }

            // .…then add the data, remembering any new timelines…
            self.obj_db
                .add_data_msg(msg_id, time_point, data_path, data);

            // …finally, if needed, add outstanding timeless data to any newly created timelines.
            if !new_timelines.is_empty() {
                let timeless_data_messages = self
                    .timeless_message_ids
                    .iter()
                    .filter_map(|msg_id| self.log_messages.get(msg_id).cloned())
                    .collect_vec();
                for msg in &timeless_data_messages {
                    if let LogMsg::DataMsg(msg) = msg {
                        self.add_data_msg(msg.msg_id, &new_timelines, &msg.data_path, &msg.data);
                    }
                }
            }
        }
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

        match (
            !self.obj_db.store.is_empty(),
            self.obj_db.arrow_store.total_temporal_index_rows() > 0,
        ) {
            (true, true) => warn_once!("GC not supported in mixed mode"),
            (true, false) => self.purge_fraction_of_ram_classic(fraction_to_purge),
            (false, true) => self.purge_fraction_of_ram_arrow(fraction_to_purge),
            (false, false) => {}
        }
    }

    fn purge_fraction_of_ram_arrow(&mut self, fraction_to_purge: f32) {
        crate::profile_function!();

        assert!((0.0..=1.0).contains(&fraction_to_purge));

        let drop_msg_ids = {
            let msg_id_chunks = self.obj_db.arrow_store.gc(
                GarbageCollectionTarget::DropAtLeastPercentage(fraction_to_purge as _),
                Timeline::new("log_time", TimeType::Time),
                MsgId::name(),
            );

            msg_id_chunks
                .iter()
                .flat_map(|chunk| {
                    arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap()
                })
                .map(Option::unwrap) // MsgId is always present
                .collect::<ahash::HashSet<_>>()
        };

        let Self {
            chronological_message_ids,
            log_messages,
            timeless_message_ids,
            recording_info: _,
            obj_db,
        } = self;

        chronological_message_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));

        {
            crate::profile_scope!("log_messages");
            log_messages.retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
        }
        {
            crate::profile_scope!("timeless_message_ids");
            timeless_message_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
        }

        obj_db.retain(None, Some(&drop_msg_ids));
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    fn purge_fraction_of_ram_classic(&mut self, fraction_to_purge: f32) {
        fn always_keep(msg: &LogMsg) -> bool {
            match msg {
                //TODO(john) allow purging ArrowMsg
                LogMsg::ArrowMsg(_) | LogMsg::BeginRecordingMsg(_) | LogMsg::TypeMsg(_) => true,
                LogMsg::DataMsg(msg) => msg.time_point.is_timeless(),
                LogMsg::PathOpMsg(msg) => msg.time_point.is_timeless(),
            }
        }

        crate::profile_function!();

        assert!((0.0..=1.0).contains(&fraction_to_purge));

        // Start by figuring out what `MsgId`:s to keep:
        let keep_msg_ids = {
            crate::profile_scope!("calc_what_to_keep");
            let mut keep_msg_ids = ahash::HashSet::default();
            for (_, time_points) in self.obj_db.tree.prefix_times.iter() {
                let num_to_purge = (time_points.len() as f32 * fraction_to_purge).round() as usize;
                for (_, msg_id) in time_points.iter().skip(num_to_purge) {
                    keep_msg_ids.extend(msg_id);
                }
            }

            keep_msg_ids.extend(
                self.log_messages
                    .iter()
                    .filter_map(|(msg_id, msg)| always_keep(msg).then_some(*msg_id)),
            );
            keep_msg_ids
        };

        let Self {
            chronological_message_ids,
            log_messages,
            timeless_message_ids,
            recording_info: _,
            obj_db,
        } = self;

        chronological_message_ids.retain(|msg_id| keep_msg_ids.contains(msg_id));

        {
            crate::profile_scope!("log_messages");
            log_messages.retain(|msg_id, _| keep_msg_ids.contains(msg_id));
        }
        {
            crate::profile_scope!("timeless_message_ids");
            timeless_message_ids.retain(|msg_id| keep_msg_ids.contains(msg_id));
        }

        obj_db.retain(Some(&keep_msg_ids), None);
    }
}
