use crate::misc::TimePoints;

use log_types::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::time_axis::TimeRange;

#[derive(Default)]
pub(crate) struct LogDb {
    /// Messages in the order they arrived
    chronological_message_ids: Vec<LogId>,
    messages: nohash_hasher::IntMap<LogId, LogMsg>,
    pub time_points: TimePoints,
    pub spaces: BTreeMap<ObjectPath, SpaceSummary>,
    pub object_tree: ObjectTree,
    object_history: HashMap<ObjectPath, ObjectHistory>,
}

impl LogDb {
    pub fn add(&mut self, msg: LogMsg) {
        crate::profile_function!();
        santiy_check(&msg);

        self.chronological_message_ids.push(msg.id);
        self.time_points.insert(&msg.time_point);

        if let Some(space) = &msg.space {
            let summary = self.spaces.entry(space.clone()).or_default();
            summary.messages.insert(msg.id);

            match &msg.data {
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

        self.object_history
            .entry(msg.object_path.clone())
            .or_default()
            .add(&msg.time_point, msg.id);
        self.object_tree.add_log_msg(&msg);
        self.messages.insert(msg.id, msg);
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// In the order they arrived
    pub fn chronological_messages(&self) -> impl Iterator<Item = &LogMsg> {
        self.chronological_message_ids
            .iter()
            .filter_map(|id| self.messages.get(id))
    }

    pub fn get_msg(&self, id: &LogId) -> Option<&LogMsg> {
        self.messages.get(id)
    }

    /// Grouped by [`ObjectPath`], find the latest [`LogMsg`] that matches
    /// the given time source and is not after the given time.
    pub fn latest_of_each_object(
        &self,
        time_source: &str,
        no_later_than: TimeValue,
        filter: &MessageFilter,
    ) -> Vec<&LogMsg> {
        crate::profile_function!();
        self.object_history
            .iter()
            .filter_map(|(object_path, history)| {
                if filter.allow(object_path) {
                    history
                        .latest(time_source, no_later_than)
                        .and_then(|id| self.get_msg(&id))
                } else {
                    None
                }
            })
            .collect()
    }

    /// All messages in the range, plus the last one before the range.
    ///
    /// This last addition is so that we get "static" assets too. We should maybe have a nicer way to accomplish this.
    pub fn messages_in_range(
        &self,
        time_source: &str,
        range: TimeRange,
        filter: &MessageFilter,
    ) -> Vec<&LogMsg> {
        crate::profile_function!();
        let mut ids = vec![];
        for (object_path, history) in &self.object_history {
            if filter.allow(object_path) {
                history.collect_in_range(time_source, range, &mut ids);
            }
        }
        ids.into_iter().filter_map(|id| self.get_msg(&id)).collect()
    }

    pub fn latest(
        &self,
        time_source: &str,
        no_later_than: TimeValue,
        object_path: &ObjectPath,
    ) -> Option<&LogMsg> {
        let id = self
            .object_history
            .get(object_path)?
            .latest(time_source, no_later_than)?;
        self.get_msg(&id)
    }
}

pub enum MessageFilter {
    All,
    /// Only return messages with this path
    ObjectPath(ObjectPath),
}

impl MessageFilter {
    pub fn allow(&self, candidate: &ObjectPath) -> bool {
        match self {
            MessageFilter::All => true,
            MessageFilter::ObjectPath(needle) => needle == candidate,
        }
    }
}

fn santiy_check(msg: &LogMsg) {
    if (msg.data.is_2d() || msg.data.is_3d()) && msg.space.is_none() {
        tracing::warn!("Got 2D/3D log message without a space: {:?}", msg);
    }
}

/// Maps time source to an ordered history of the messages.
///
/// This allows fast lookup of "latest version" of an object.
#[derive(Default)]
pub(crate) struct ObjectHistory(HashMap<String, BTreeMap<TimeValue, Vec<LogId>>>);

impl ObjectHistory {
    fn add(&mut self, time_point: &TimePoint, id: LogId) {
        for (time_source, time_value) in &time_point.0 {
            self.0
                .entry(time_source.clone())
                .or_default()
                .entry(*time_value)
                .or_default()
                .push(id);
        }
    }

    fn latest(&self, time_source: &str, no_later_than: TimeValue) -> Option<LogId> {
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
    fn collect_in_range(&self, time_source: &str, range: TimeRange, out: &mut Vec<LogId>) {
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

/// Tree of object paths.
#[derive(Default)]
pub(crate) struct ObjectTree {
    /// Distinguished only by the string-part (ignores any [`Identifier`] of e.g. [`ObjectPathComponent::PersistId`].
    pub children: BTreeMap<String, ObjectTreeNode>,

    /// When do we have data?
    ///
    /// Data logged at this exact path.
    pub times: BTreeMap<TimePoint, BTreeSet<LogId>>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: BTreeMap<TimePoint, BTreeSet<LogId>>,

    /// Data logged at this exact path.
    pub data: DataColumns,
}

#[derive(Default)]
pub(crate) struct ObjectTreeNode {
    /// Children of type [`ObjectPathComponent::String`].
    pub string_children: ObjectTree,

    /// Children of type [`ObjectPathComponent::PersistId`].
    pub persist_id_children: BTreeMap<Identifier, ObjectTree>,

    /// Children of type [`ObjectPathComponent::TempId`].
    pub temp_id_children: BTreeMap<Identifier, ObjectTree>,
}

impl ObjectTree {
    pub fn add_log_msg(&mut self, msg: &LogMsg) {
        self.add_path(&msg.object_path.0[..], msg);
    }

    fn add_path(&mut self, path: &[ObjectPathComponent], msg: &LogMsg) {
        self.prefix_times
            .entry(msg.time_point.clone())
            .or_default()
            .insert(msg.id);

        match path {
            [] => {
                self.times
                    .entry(msg.time_point.clone())
                    .or_default()
                    .insert(msg.id);
                self.data.add(msg);
            }
            [first, rest @ ..] => match first {
                ObjectPathComponent::String(string) => {
                    self.children
                        .entry(string.clone())
                        .or_default()
                        .string_children
                        .add_path(rest, msg);
                }
                ObjectPathComponent::PersistId(string, id) => {
                    self.children
                        .entry(string.clone())
                        .or_default()
                        .persist_id_children
                        .entry(id.clone())
                        .or_default()
                        .add_path(rest, msg);
                }
                ObjectPathComponent::TempId(string, id) => {
                    self.children
                        .entry(string.clone())
                        .or_default()
                        .temp_id_children
                        .entry(id.clone())
                        .or_default()
                        .add_path(rest, msg);
                }
            },
        }
    }
}

/// Column transform of [`Data`].
#[derive(Default)]
pub(crate) struct DataColumns {
    pub per_type: HashMap<DataType, BTreeSet<LogId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg: &LogMsg) {
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
