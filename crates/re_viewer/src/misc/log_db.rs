use std::collections::{BTreeMap, BTreeSet, HashMap};

use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::*;
use re_string_interner::InternedString;

use crate::misc::TimePoints;

/// A in-memory database built from a stream of [`LogMsg`]es.
#[derive(Default)]
pub(crate) struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<MsgId>,
    log_messages: IntMap<MsgId, LogMsg>,

    recording_info: Option<RecordingInfo>,

    pub object_types: IntMap<ObjTypePath, ObjectType>,
    pub time_points: TimePoints,
    pub data_tree: ObjectTree,
    pub data_store: re_data_store::LogDataStore,

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

    /// All known spacves, in undefined order.
    pub fn spaces(&self) -> impl ExactSizeIterator<Item = &ObjPath> {
        self.spaces.values()
    }

    pub fn add(&mut self, msg: LogMsg) {
        crate::profile_function!();
        match &msg {
            LogMsg::BeginRecordingMsg(msg) => self.add_begin_recording_msg(msg),
            LogMsg::TypeMsg(msg) => self.add_type_msg(msg),
            LogMsg::DataMsg(msg) => self.add_data_msg(msg),
        }
        self.chronological_message_ids.push(msg.id());
        self.log_messages.insert(msg.id(), msg);
    }

    fn add_begin_recording_msg(&mut self, msg: &BeginRecordingMsg) {
        self.recording_info = Some(msg.info.clone());
    }

    fn add_type_msg(&mut self, msg: &TypeMsg) {
        let previous_value = self
            .object_types
            .insert(msg.type_path.clone(), msg.object_type);

        if let Some(previous_value) = previous_value {
            if previous_value != msg.object_type {
                tracing::warn!(
                    "Object {} changed type from {:?} to {:?}",
                    msg.type_path,
                    previous_value,
                    msg.object_type
                );
            }
        } else {
            tracing::debug!(
                "Registered object type {}: {:?}",
                msg.type_path,
                msg.object_type
            );
        }
    }

    fn add_data_msg(&mut self, msg: &DataMsg) {
        crate::profile_function!();

        let obj_type_path = &msg.data_path.obj_path.obj_type_path();
        if let Some(object_type) = self.object_types.get(obj_type_path) {
            let field_name = &msg.data_path.field_name;
            let valid_members = object_type.members();
            if !valid_members.contains(&field_name.as_str()) {
                log_once::warn_once!(
                    "Logged to {obj_type_path}.{field_name}, but the parent object ({object_type:?}) does not have that field. Expected one of: {}",
                    valid_members.iter().format(", ")
                );
            }
        } else {
            log_once::warn_once!(
                "Logging to {obj_type_path}.{field_name} without first registering object type"
            );
        }

        if let Err(err) = self.data_store.insert(msg) {
            tracing::warn!("Failed to add data to data_store: {err:?}");
        }

        self.time_points.insert(&msg.time_point);

        self.data_tree.add_data_msg(msg);

        self.register_spaces(msg);
    }

    fn register_spaces(&mut self, msg: &DataMsg) {
        let mut register_space = |space: &ObjPath| {
            self.spaces
                .entry(*space.hash())
                .or_insert_with(|| space.clone());
        };

        // This is a bit hacky, and I don't like it,
        // but we nned a single place to find all the spaces (ignoring time).
        match &msg.data {
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
pub(crate) struct ObjectTree {
    /// Children of type [`ObjPathComp::String`].
    pub string_children: BTreeMap<InternedString, ObjectTree>,

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
        self.string_children.is_empty() && self.index_children.is_empty()
    }

    pub fn add_data_msg(&mut self, msg: &DataMsg) {
        crate::profile_function!();
        let obj_path = msg.data_path.obj_path.to_components();
        self.add_path(obj_path.as_slice(), msg.data_path.field_name, msg);
    }

    fn add_path(&mut self, path: &[ObjPathComp], field_name: FieldName, msg: &DataMsg) {
        for (time_source, time_value) in &msg.time_point.0 {
            self.prefix_times
                .entry(*time_source)
                .or_default()
                .entry(time_value.as_int())
                .or_default()
                .insert(msg.msg_id);
        }

        match path {
            [] => {
                self.fields.entry(field_name).or_default().add(msg);
            }
            [first, rest @ ..] => match first {
                ObjPathComp::String(string) => {
                    self.string_children
                        .entry(*string)
                        .or_default()
                        .add_path(rest, field_name, msg);
                }
                ObjPathComp::Index(index) => {
                    let slow_but_accurate = false; // TODO(emilk): ths is way too slow, though it is the only way to get toggling of batches to work. We need to do something else.
                    if index == &Index::Placeholder && slow_but_accurate {
                        if let LoggedData::Batch { indices, .. } = &msg.data {
                            crate::profile_scope!("object_tree_batch");
                            for index in indices {
                                self.index_children
                                    .entry(index.clone())
                                    .or_default()
                                    .add_path(rest, field_name, msg);
                            }
                        }
                    } else {
                        self.index_children
                            .entry(index.clone())
                            .or_default()
                            .add_path(rest, field_name, msg);
                    }
                }
            },
        }
    }
}

// ----------------------------------------------------------------------------

/// Column transform of [`Data`].
#[derive(Default)]
pub(crate) struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<TimeSource, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    pub per_type: HashMap<DataType, BTreeSet<MsgId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg: &DataMsg) {
        for (time_source, time_value) in &msg.time_point.0 {
            self.times
                .entry(*time_source)
                .or_default()
                .entry(time_value.as_int())
                .or_default()
                .insert(msg.msg_id);
        }

        self.per_type
            .entry(msg.data.data_type())
            .or_default()
            .insert(msg.msg_id);
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
