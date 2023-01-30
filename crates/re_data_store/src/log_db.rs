use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, GarbageCollectionTarget};
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    field_types::Instance,
    msg_bundle::{Component as _, ComponentBundle, MsgBundle},
    objects, ArrowMsg, BeginRecordingMsg, DataPath, FieldOrComponent, LogMsg, MsgId, ObjPath,
    ObjPathHash, ObjTypePath, ObjectType, PathOp, PathOpMsg, RecordingId, RecordingInfo, TimePoint,
    Timeline,
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

    /// The arrow store of data.
    pub arrow_store: re_arrow_store::DataStore,
}

impl Default for ObjDb {
    fn default() -> Self {
        Self {
            types: Default::default(),
            obj_path_from_hash: Default::default(),
            tree: crate::ObjectTree::root(),
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
            .entry(obj_path.hash())
            .or_insert_with(|| obj_path.clone());
    }

    fn add_data_msg(&mut self, msg_id: MsgId, time_point: &TimePoint, data_path: &DataPath) {
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

        self.tree.add_data_msg(msg_id, time_point, data_path);
    }

    fn try_add_arrow_data_msg(&mut self, msg: &ArrowMsg) -> Result<(), Error> {
        let msg_bundle = MsgBundle::try_from(msg).map_err(Error::MsgBundleError)?;

        self.types
            .entry(msg_bundle.obj_path.obj_type_path().clone())
            .or_insert(ObjectType::ArrowObject);

        self.register_obj_path(&msg_bundle.obj_path);

        for component in &msg_bundle.components {
            let data_path = DataPath::new_arrow(msg_bundle.obj_path.clone(), component.name);
            if component.name == MsgId::name() {
                continue;
            }
            let pending_clears =
                self.tree
                    .add_data_msg(msg.msg_id, &msg_bundle.time_point, &data_path);

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
                self.tree.add_data_msg(msg_id, &time_point, &data_path);
            }
        }

        self.arrow_store.insert(&msg_bundle).map_err(Into::into)
    }

    fn add_path_op(&mut self, msg_id: MsgId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(msg_id, time_point, path_op);

        for data_path in cleared_paths {
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
                        self.tree.add_data_msg(msg_id, time_point, &data_path);
                    }
                }
            } else if !objects::META_FIELDS.contains(&data_path.field_name.as_str()) {
                self.add_data_msg(msg_id, time_point, &data_path);
            }
        }
    }

    pub fn purge(&mut self, drop_msg_ids: &ahash::HashSet<MsgId>) {
        crate::profile_function!();

        let Self {
            types: _,
            obj_path_from_hash: _,
            tree,
            arrow_store: _, // purged before this function is called
        } = self;

        {
            crate::profile_scope!("tree");
            tree.purge(drop_msg_ids);
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

    pub fn num_timeless_messages(&self) -> usize {
        self.obj_db.tree.num_timeless_messages()
    }

    pub fn is_empty(&self) -> bool {
        self.log_messages.is_empty()
    }

    pub fn add(&mut self, msg: LogMsg) -> Result<(), Error> {
        crate::profile_function!();
        match &msg {
            LogMsg::BeginRecordingMsg(msg) => self.add_begin_recording_msg(msg),
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
            LogMsg::Goodbye(_) => {}
        }
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

        let drop_msg_ids = {
            let msg_id_chunks = self.obj_db.arrow_store.gc(
                GarbageCollectionTarget::DropAtLeastPercentage(fraction_to_purge as _),
                Timeline::log_time(),
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

        obj_db.purge(&drop_msg_ids);
    }
}
