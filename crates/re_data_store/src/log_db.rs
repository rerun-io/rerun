use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::*;

// ----------------------------------------------------------------------------

/// An aggregate of [`TimePoint`]:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<Timeline, BTreeSet<TimeInt>>);

// ----------------------------------------------------------------------------

/// Stored objects and their types, with easy indexing of the paths.
pub struct ObjDb {
    /// The types of all the objects.
    /// Must be registered before adding them.
    pub types: IntMap<ObjTypePath, ObjectType>,

    /// A tree-view (split on path components) of the objects.
    pub tree: crate::ObjectTree,

    /// The actual store of data.
    pub store: crate::DataStore,
}

impl Default for ObjDb {
    fn default() -> Self {
        Self {
            types: Default::default(),
            tree: crate::ObjectTree::root(),
            store: Default::default(),
        }
    }
}

impl ObjDb {
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

        if let Err(err) = self.store.insert_data(msg_id, time_point, data_path, data) {
            re_log::warn!("Failed to add data to data_store: {err:?}");
        }

        self.tree.add_data_msg(msg_id, time_point, data_path, data);
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
#[derive(Default)]
pub struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<MsgId>,
    log_messages: IntMap<MsgId, LogMsg>,

    /// Data that was logged with [`TimePoint::timeless`].
    /// We need to re-insert those in any new timelines
    /// that are created after they were logged.
    timeless_message_ids: Vec<MsgId>,

    recording_info: Option<RecordingInfo>,

    /// All the points of time when we have some data.
    pub time_points: TimePoints,

    /// Where we store the objects.
    pub obj_db: ObjDb,

    /// All known spaces
    spaces: IntMap<ObjPathHash, ObjPath>,
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

    pub fn is_empty(&self) -> bool {
        self.log_messages.is_empty()
    }

    /// All known spaces, in undefined order.
    pub fn spaces(&self) -> impl ExactSizeIterator<Item = &ObjPath> {
        self.spaces.values()
    }

    pub fn add(&mut self, msg: LogMsg) {
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
        }
        self.chronological_message_ids.push(msg.id());
        self.log_messages.insert(msg.id(), msg);
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
            // as well to all future timelines,
            // so we special-case it here.
            // See https://linear.app/rerun/issue/PRO-97

            // Remember to add it to future timelines:
            self.timeless_message_ids.push(msg_id);

            if !self.time_points.0.is_empty() {
                // Add to existing timelines (if any):
                let mut time_point = TimePoint::default();
                for &timeline in self.time_points.0.keys() {
                    time_point.0.insert(timeline, TimeInt::BEGINNING);
                }
                self.add_data_msg(msg_id, &time_point, data_path, data);
            }

            return; // done
        }

        self.obj_db
            .add_data_msg(msg_id, time_point, data_path, data);

        if data_path.field_name == "space" {
            self.register_spaces(data);
        }

        {
            let mut new_timelines = TimePoint::default();

            for (timeline, value) in &time_point.0 {
                match self.time_points.0.entry(*timeline) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        re_log::debug!("New timeline added: {timeline:?}");
                        new_timelines.0.insert(*timeline, TimeInt::BEGINNING);
                        entry.insert(Default::default())
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                }
                .insert(*value);
            }

            if !new_timelines.0.is_empty() {
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

    fn register_spaces(&mut self, data: &LoggedData) {
        let mut register_space = |space: &ObjPath| {
            self.spaces
                .entry(*space.hash())
                .or_insert_with(|| space.clone());
        };

        // This is a bit hacky, and I don't like it,
        // but we need a single place to find all the spaces (ignoring time).
        // This will be properly fixed when https://linear.app/rerun/issue/PRO-98/refactor-spaces is done
        match data {
            LoggedData::Single(Data::ObjPath(space))
            | LoggedData::BatchSplat(Data::ObjPath(space)) => {
                register_space(space);
            }
            LoggedData::Batch {
                data: DataVec::ObjPath(spaces),
                ..
            } => {
                for space in spaces {
                    register_space(space);
                }
            }
            _ => {}
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
}

// ----------------------------------------------------------------------------

#[cfg(feature = "re_memory")]
impl re_memory::GenNode for LogDb {
    fn node(&self, global: &mut re_memory::Global) -> re_memory::Node {
        if true {
            self.obj_db.node(global)
        } else {
            re_memory::Node::Struct(re_memory::Struct {
                type_name: std::any::type_name::<Self>(),
                fields: vec![("obj_db", self.obj_db.node(global))],
            })
        }
    }
}

#[cfg(feature = "re_memory")]
impl re_memory::GenNode for ObjDb {
    fn node(&self, global: &mut re_memory::Global) -> re_memory::Node {
        if true {
            self.store.node(global)
        } else {
            re_memory::Node::Struct(re_memory::Struct {
                type_name: std::any::type_name::<Self>(),
                fields: vec![("store", self.store.node(global))],
            })
        }
    }
}
