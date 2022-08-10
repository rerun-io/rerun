use std::collections::{BTreeMap, BTreeSet, HashMap};

use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::*;
use re_string_interner::InternedString;

use crate::misc::TimePoints;

#[derive(Default)]
pub(crate) struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<LogId>,
    log_messages: IntMap<LogId, LogMsg>,
    pub object_types: IntMap<ObjTypePath, ObjectType>,
    pub time_points: TimePoints,
    pub data_tree: ObjectTree,
    pub data_store: re_data_store::LogDataStore,
}

impl LogDb {
    pub fn is_empty(&self) -> bool {
        self.log_messages.is_empty()
    }

    pub fn add(&mut self, msg: LogMsg) {
        crate::profile_function!();
        match &msg {
            LogMsg::TypeMsg(type_msg) => self.add_type_msg(type_msg),
            LogMsg::DataMsg(data_msg) => self.add_data_msg(data_msg),
        }
        self.chronological_message_ids.push(msg.id());
        self.log_messages.insert(msg.id(), msg);
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
            let valid_members = object_type.members();
            if !valid_members.contains(&msg.data_path.field_name.as_str()) {
                log_once::warn_once!(
                    "Logged to {}, but the parent object ({:?}) does not have that field. Expected on of: {}",
                    obj_type_path,
                    object_type,
                    valid_members.iter().format(", ")
                );
            }
        } else {
            log_once::warn_once!(
                "Logging to {}.{} without first registering object type",
                obj_type_path,
                msg.data_path.field_name
            );
        }

        if let Err(err) = self.data_store.insert(msg) {
            tracing::warn!("Failed to add data to data_store: {:?}", err);
        }

        self.time_points.insert(&msg.time_point);

        self.data_tree.add_data_msg(msg);
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

    pub fn get_log_msg(&self, id: &LogId) -> Option<&LogMsg> {
        self.log_messages.get(id)
    }

    pub fn get_data_msg(&self, id: &LogId) -> Option<&DataMsg> {
        match self.log_messages.get(id)? {
            LogMsg::TypeMsg(_) => None,
            LogMsg::DataMsg(msg) => Some(msg),
        }
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
    pub prefix_times: BTreeMap<TimePoint, BTreeSet<LogId>>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldName, DataColumns>,
}

impl ObjectTree {
    /// Has no child objects.
    pub fn is_leaf(&self) -> bool {
        self.string_children.is_empty() && self.index_children.is_empty()
    }

    pub fn add_data_msg(&mut self, msg: &DataMsg) {
        let obj_path = ObjPathBuilder::from(&msg.data_path.obj_path);
        self.add_path(obj_path.as_slice(), msg.data_path.field_name, msg);
    }

    fn add_path(&mut self, path: &[ObjPathComp], field_name: FieldName, msg: &DataMsg) {
        self.prefix_times
            .entry(msg.time_point.clone())
            .or_default()
            .insert(msg.id);

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
                    self.index_children
                        .entry(index.clone())
                        .or_default()
                        .add_path(rest, field_name, msg);
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
    pub times: BTreeMap<TimePoint, BTreeSet<LogId>>,
    pub per_type: HashMap<DataType, BTreeSet<LogId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg: &DataMsg) {
        self.times
            .entry(msg.time_point.clone())
            .or_default()
            .insert(msg.id);

        self.per_type
            .entry(msg.data.data_type())
            .or_default()
            .insert(msg.id);
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
                DataType::LineSegments2D => ("2D line segment list", "s"),

                DataType::Vec3 => ("3D vector", "s"),
                DataType::Box3 => ("3D box", "es"),
                DataType::Path3D => ("3D path", "s"),
                DataType::LineSegments3D => ("3D line segment list", "s"),
                DataType::Mesh3D => ("mesh", "es"),
                DataType::Camera => ("camera", "s"),

                DataType::Tensor => ("tensor", "s"),

                DataType::Space => ("space", "s"),
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
