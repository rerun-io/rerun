use std::collections::{BTreeMap, BTreeSet, HashMap};

use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::*;
use re_string_interner::InternedString;

// ----------------------------------------------------------------------------

/// An aggregate of [`TimePoint`]:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<TimeSource, BTreeSet<TimeInt>>);

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

    pub obj_types: IntMap<ObjTypePath, ObjectType>,

    /// All the points of time when we have some data.
    pub time_points: TimePoints,

    /// A tree-view (split on path components) of the objects.
    pub data_tree: ObjectTree,

    /// The actual store of data.
    pub data_store: crate::DataStore,

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
        let previous_value = self.obj_types.insert(msg.type_path.clone(), msg.obj_type);

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
                for &time_source in self.time_points.0.keys() {
                    time_point.0.insert(time_source, TimeInt::BEGINNING);
                }
                self.add_data_msg(msg_id, &time_point, data_path, data);
            }

            return; // done
        }

        // Validate:
        {
            let obj_type_path = &data_path.obj_path.obj_type_path();
            let field_name = &data_path.field_name;
            if let Some(obj_type) = self.obj_types.get(obj_type_path) {
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

        if let Err(err) = self
            .data_store
            .insert_data(msg_id, time_point, data_path, data)
        {
            re_log::warn!("Failed to add data to data_store: {err:?}");
        }

        self.data_tree
            .add_data_msg(msg_id, time_point, data_path, data);

        self.register_spaces(data);

        {
            let mut new_timelines = TimePoint::default();

            for (time_source, value) in &time_point.0 {
                match self.time_points.0.entry(*time_source) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        re_log::debug!("New timeline added: {time_source:?}");
                        new_timelines.0.insert(*time_source, TimeInt::BEGINNING);
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
        match data {
            LoggedData::Single(Data::Space(space)) | LoggedData::BatchSplat(Data::Space(space)) => {
                register_space(space);
            }
            LoggedData::Batch {
                data: DataVec::Space(spaces),
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

/// Tree of data paths.
#[derive(Default)]
pub struct ObjectTree {
    /// Children of type [`ObjPathComp::Name`].
    pub named_children: BTreeMap<InternedString, ObjectTree>,

    /// Children of type [`ObjPathComp::Index`].
    pub index_children: BTreeMap<Index, ObjectTree>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: BTreeMap<TimeSource, BTreeMap<TimeInt, BTreeSet<MsgId>>>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldName, DataColumns>,
}

impl ObjectTree {
    /// Has no child objects.
    pub fn is_leaf(&self) -> bool {
        self.named_children.is_empty() && self.index_children.is_empty()
    }

    pub fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) {
        crate::profile_function!();
        let obj_path = data_path.obj_path.to_components();
        self.add_path(
            obj_path.as_slice(),
            data_path.field_name,
            msg_id,
            time_point,
            data,
        );
    }

    fn add_path(
        &mut self,
        path: &[ObjPathComp],
        field_name: FieldName,
        msg_id: MsgId,
        time_point: &TimePoint,
        data: &LoggedData,
    ) {
        for (time_source, time_value) in &time_point.0 {
            self.prefix_times
                .entry(*time_source)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        match path {
            [] => {
                self.fields
                    .entry(field_name)
                    .or_default()
                    .add(msg_id, time_point, data);
            }
            [first, rest @ ..] => match first {
                ObjPathComp::Name(name) => {
                    self.named_children
                        .entry(*name)
                        .or_default()
                        .add_path(rest, field_name, msg_id, time_point, data);
                }
                ObjPathComp::Index(index) => {
                    self.index_children
                        .entry(index.clone())
                        .or_default()
                        .add_path(rest, field_name, msg_id, time_point, data);
                }
            },
        }
    }
}

// ----------------------------------------------------------------------------

/// Column transform of [`Data`].
#[derive(Default)]
pub struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<TimeSource, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    pub per_type: HashMap<DataType, BTreeSet<MsgId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg_id: MsgId, time_point: &TimePoint, data: &LoggedData) {
        for (time_source, time_value) in &time_point.0 {
            self.times
                .entry(*time_source)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        self.per_type
            .entry(data.data_type())
            .or_default()
            .insert(msg_id);
    }

    pub fn summary(&self) -> String {
        let mut summaries = vec![];

        for (typ, set) in &self.per_type {
            let (stem, plur) = match typ {
                DataType::I32 => ("integer", "s"),
                DataType::F32 => ("float", "s"),
                DataType::Color => ("color", "s"),
                DataType::String => ("string", "s"),

                DataType::Vec2 => ("2D vector", "s"),
                DataType::BBox2D => ("2D bounding box", "es"),

                DataType::Vec3 => ("3D vector", "s"),
                DataType::Box3 => ("3D box", "es"),
                DataType::Mesh3D => ("mesh", "es"),
                DataType::Camera => ("camera", "s"),

                DataType::Tensor => ("tensor", "s"),

                DataType::Space => ("space", "s"),

                DataType::DataVec => ("vector", "s"),
            };

            summaries.push(plurality(set.len(), stem, plur));
        }

        summaries.join(", ")
    }
}

fn plurality(num: usize, singular: &str, plural_suffix: &str) -> String {
    if num == 1 {
        format!("1 {}", singular)
    } else {
        format!("{} {}{}", num, singular, plural_suffix)
    }
}
