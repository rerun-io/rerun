use std::collections::{BTreeMap, BTreeSet, HashMap};

use itertools::Itertools as _;

use log_types::*;
use rr_string_interner::InternedString;

use crate::misc::TimePoints;

use super::time_axis::TimeRange;

#[derive(Default)]
pub(crate) struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<LogId>,
    log_messages: nohash_hasher::IntMap<LogId, LogMsg>,
    pub object_types: ahash::AHashMap<ObjTypePath, ObjectType>,
    pub time_points: TimePoints,
    pub spaces: BTreeMap<ObjPath, SpaceSummary>,
    pub data_tree: ObjectTree,
    pub data_store: data_store::LogDataStore,
    data_history: ahash::AHashMap<DataPath, DataHistoryLog>,
}

impl LogDb {
    pub fn add(&mut self, msg: LogMsg) {
        match &msg {
            LogMsg::TypeMsg(type_msg) => self.add_type_msg(type_msg),
            LogMsg::DataMsg(data_msg) => self.add_data_msg(data_msg),
        }
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
        if (msg.data.is_2d() || msg.data.is_3d()) && msg.space.is_none() {
            tracing::warn!("Got 2D/3D data message without a space: {:?}", msg);
        }

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

        if let Some(space) = msg.space.clone() {
            if !matches!(msg.data, Data::Batch { .. }) {
                // HACK until we change how spaces are logged
                let space_msg = DataMsg {
                    id: msg.id,
                    time_point: msg.time_point.clone(),
                    data_path: msg.data_path.sibling("space"),
                    data: Data::Space(space),
                    space: None,
                };
                if let Err(err) = self.data_store.insert(&space_msg) {
                    tracing::warn!("Failed to add space data to data_store: {:?}", err);
                }
            }
        }

        self.chronological_message_ids.push(msg.id);
        self.time_points.insert(&msg.time_point);

        if let Some(space) = &msg.space {
            let summary = self.spaces.entry(space.clone()).or_default();
            summary.messages.insert(msg.id);

            match &msg.data {
                Data::Batch { data, .. } => match data {
                    DataBatch::Pos3(pos) => {
                        summary.messages_2d.insert(msg.id);
                        for p in pos {
                            summary.bbox3d.extend((*p).into());
                        }
                    }
                    DataBatch::Color(_) | DataBatch::Space(_) => {}
                },

                Data::Pos2(vec) => {
                    summary.messages_2d.insert(msg.id);
                    summary.bbox2d.extend_with(vec.into());
                }
                Data::BBox2D(bbox) => {
                    summary.messages_2d.insert(msg.id);
                    summary.bbox2d.extend_with(bbox.min.into());
                    summary.bbox2d.extend_with(bbox.max.into());
                }
                Data::LineSegments2D(line_segments) => {
                    summary.messages_2d.insert(msg.id);
                    for [a, b] in line_segments {
                        summary.bbox2d.extend_with(a.into());
                        summary.bbox2d.extend_with(b.into());
                    }
                }
                Data::Image(image) => {
                    summary.messages_2d.insert(msg.id);
                    summary.bbox2d.extend_with(egui::Pos2::ZERO);
                    summary
                        .bbox2d
                        .extend_with(egui::pos2(image.size[0] as _, image.size[1] as _));
                }

                Data::Pos3(pos) => {
                    summary.messages_3d.insert(msg.id);
                    summary.bbox3d.extend((*pos).into());
                }
                Data::Vec3(_) => {
                    // NOTE: vectors aren't positions
                    summary.messages_3d.insert(msg.id);
                }
                Data::Box3(box3) => {
                    summary.messages_3d.insert(msg.id);
                    let Box3 {
                        rotation,
                        translation,
                        half_size,
                    } = box3;
                    let rotation = glam::Quat::from_array(*rotation);
                    let translation = glam::Vec3::from(*translation);
                    let half_size = glam::Vec3::from(*half_size);
                    let transform = glam::Mat4::from_scale_rotation_translation(
                        half_size,
                        rotation,
                        translation,
                    );
                    use glam::vec3;
                    let corners = [
                        transform
                            .transform_point3(vec3(-0.5, -0.5, -0.5))
                            .to_array(),
                        transform.transform_point3(vec3(-0.5, -0.5, 0.5)).to_array(),
                        transform.transform_point3(vec3(-0.5, 0.5, -0.5)).to_array(),
                        transform.transform_point3(vec3(-0.5, 0.5, 0.5)).to_array(),
                        transform.transform_point3(vec3(0.5, -0.5, -0.5)).to_array(),
                        transform.transform_point3(vec3(0.5, -0.5, 0.5)).to_array(),
                        transform.transform_point3(vec3(0.5, 0.5, -0.5)).to_array(),
                        transform.transform_point3(vec3(0.5, 0.5, 0.5)).to_array(),
                    ];
                    for p in corners {
                        summary.bbox3d.extend(p.into());
                    }
                }
                Data::Path3D(points) => {
                    summary.messages_3d.insert(msg.id);
                    for &p in points {
                        summary.bbox3d.extend(p.into());
                    }
                }
                Data::LineSegments3D(segments) => {
                    summary.messages_3d.insert(msg.id);
                    for &[a, b] in segments {
                        summary.bbox3d.extend(a.into());
                        summary.bbox3d.extend(b.into());
                    }
                }
                Data::Mesh3D(mesh) => {
                    summary.messages_3d.insert(msg.id);
                    match mesh {
                        Mesh3D::Encoded(_) => {
                            // TODO: how to we get the bbox of an encoded mesh here?
                        }
                        Mesh3D::Raw(mesh) => {
                            for &pos in &mesh.positions {
                                summary.bbox3d.extend(pos.into());
                            }
                        }
                    }
                }
                Data::Camera(camera) => {
                    summary.messages_3d.insert(msg.id);
                    summary.bbox3d.extend(camera.position.into());
                }

                _ => {
                    debug_assert!(!msg.data.is_2d(), "Missed handling 2D data: {:?}", msg.data);
                    debug_assert!(!msg.data.is_3d(), "Missed handling 3D data: {:?}", msg.data);
                }
            }
        }

        self.data_history
            .entry(msg.data_path.clone())
            .or_default()
            .add(&msg.time_point, msg.id);
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

    /// In the order they arrived
    pub fn chronological_data_messages(&self) -> impl Iterator<Item = &DataMsg> {
        self.chronological_message_ids
            .iter()
            .filter_map(|id| self.get_data_msg(id))
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

    /// Grouped by [`DataPath`], find the latest [`DataMsg`] that matches
    /// the given time source and is not after the given time.
    pub fn latest_of_each_object(
        &self,
        time_source: &TimeSource,
        no_later_than: TimeValue,
        filter: &MessageFilter,
    ) -> Vec<&DataMsg> {
        crate::profile_function!();
        self.data_history
            .iter()
            .filter_map(|(data_path, history)| {
                if filter.allow(data_path) {
                    history
                        .latest(time_source, no_later_than)
                        .and_then(|id| self.get_data_msg(&id))
                } else {
                    None
                }
            })
            .collect()
    }

    /// All messages in the range, plus the last one before the range.
    ///
    /// This last addition is so that we get "static" assets too. We should maybe have a nicer way to accomplish this.
    pub fn data_messages_in_range(
        &self,
        time_source: &TimeSource,
        range: TimeRange,
        filter: &MessageFilter,
    ) -> Vec<&DataMsg> {
        crate::profile_function!();
        let mut ids = vec![];
        for (data_path, history) in &self.data_history {
            if filter.allow(data_path) {
                history.collect_in_range(time_source, range, &mut ids);
            }
        }
        ids.into_iter()
            .filter_map(|id| self.get_data_msg(&id))
            .collect()
    }

    // pub fn latest(
    //     &self,
    //     time_source: &TimeSource,
    //     no_later_than: TimeValue,
    //     data_path: &DataPath,
    // ) -> Option<&DataMsg> {
    //     let id = self
    //         .object_history
    //         .get(data_path)?
    //         .latest(time_source, no_later_than)?;
    //     self.get_msg(&id)
    // }
}

pub enum MessageFilter {
    All,
    /// Only return messages with this path
    DataPath(DataPath),
}

impl MessageFilter {
    pub fn allow(&self, candidate: &DataPath) -> bool {
        match self {
            MessageFilter::All => true,
            MessageFilter::DataPath(needle) => needle == candidate,
        }
    }
}

/// Maps time source to an ordered history of the messages.
///
/// This allows fast lookup of "latest version" of an object.
#[derive(Default)]
pub(crate) struct DataHistoryLog(
    nohash_hasher::IntMap<TimeSource, BTreeMap<TimeValue, Vec<LogId>>>,
);

impl DataHistoryLog {
    fn add(&mut self, time_point: &TimePoint, id: LogId) {
        for (time_source, time_value) in &time_point.0 {
            self.0
                .entry(*time_source)
                .or_default()
                .entry(*time_value)
                .or_default()
                .push(id);
        }
    }

    fn latest(&self, time_source: &TimeSource, no_later_than: TimeValue) -> Option<LogId> {
        self.0
            .get(time_source)?
            .range(..=no_later_than)
            .rev()
            .next()?
            .1
            .last()
            .copied()
    }

    /// All messages in the range, plus the last one before the range.
    ///
    /// This last addition is so that we get "static" assets too. We should maybe have a nicer way to accomplish this.
    fn collect_in_range(&self, time_source: &TimeSource, range: TimeRange, out: &mut Vec<LogId>) {
        if let Some(map) = self.0.get(time_source) {
            for (time, ids) in map.range(..=range.max).rev() {
                if time < &range.min {
                    out.push(*ids.last().unwrap());
                    break;
                } else {
                    out.extend(ids.iter().copied());
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

pub(crate) struct SpaceSummary {
    /// All messages in this space
    pub messages: BTreeSet<LogId>,

    /// messages with 2D data
    pub messages_2d: BTreeSet<LogId>,

    /// messages with 3D data
    pub messages_3d: BTreeSet<LogId>,

    /// bounding box of 2D data
    pub bbox2d: egui::Rect,

    /// bounding box of 3D data
    pub bbox3d: macaw::BoundingBox,
}

impl Default for SpaceSummary {
    fn default() -> Self {
        Self {
            messages: Default::default(),
            messages_2d: Default::default(),
            messages_3d: Default::default(),
            bbox2d: egui::Rect::NOTHING,
            bbox3d: macaw::BoundingBox::nothing(),
        }
    }
}

impl SpaceSummary {
    /// Only set for 2D spaces
    pub fn size_2d(&self) -> Option<egui::Vec2> {
        if self.bbox2d.is_positive() {
            Some(self.bbox2d.size())
        } else {
            None
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
            .entry(msg.data.typ())
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

                DataType::Pos2 => ("2D position", "s"),
                DataType::BBox2D => ("2D bounding box", "es"),
                DataType::LineSegments2D => ("2D line segment list", "s"),
                DataType::Image => ("image", "s"),

                DataType::Pos3 => ("3D position", "s"),
                DataType::Vec3 => ("3D vector", "s"),
                DataType::Box3 => ("3D box", "es"),
                DataType::Path3D => ("3D path", "s"),
                DataType::LineSegments3D => ("3D line segment list", "s"),
                DataType::Mesh3D => ("mesh", "es"),
                DataType::Camera => ("camera", "s"),

                DataType::Vecf32 => ("float vector", "s"),

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
