use crate::misc::TimePoints;
use eframe::egui;
use log_types::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::time_axis::TimeRange;

#[derive(Default)]
pub(crate) struct LogDb {
    /// Messages in the order they arrived
    chronological_messages: Vec<LogId>,
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

        self.chronological_messages.push(msg.id);
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
    pub fn messages(&self) -> impl Iterator<Item = &LogMsg> {
        self.chronological_messages
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
    ) -> Vec<&LogMsg> {
        crate::profile_function!();
        self.object_history
            .values()
            .filter_map(|history| {
                history
                    .latest(time_source, no_later_than)
                    .and_then(|id| self.get_msg(&id))
            })
            .collect()
    }

    /// All messages in the range, plus the last one before the range.
    ///
    /// This last addition is so that we get "static" assets too. We should maybe have a nicer way to accomplish this.
    pub fn messages_in_range(&self, time_source: &str, range: TimeRange) -> Vec<&LogMsg> {
        crate::profile_function!();
        let mut ids = vec![];
        for history in self.object_history.values() {
            history.collect_in_range(time_source, range, &mut ids);
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
    pub times: BTreeSet<(TimePoint, LogId)>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: BTreeSet<(TimePoint, LogId)>,

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
        self.prefix_times.insert((msg.time_point.clone(), msg.id));

        match path {
            [] => {
                self.times.insert((msg.time_point.clone(), msg.id));
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
    // 1D:
    pub i32: BTreeMap<(TimePoint, LogId), i32>,
    pub f32: BTreeMap<(TimePoint, LogId), f32>,
    pub color: BTreeMap<(TimePoint, LogId), [u8; 4]>,

    // 2D:
    pub pos2: BTreeMap<(TimePoint, LogId), [f32; 2]>,
    pub line_segments_2d: BTreeSet<(TimePoint, LogId)>,
    pub bbox2d: BTreeMap<(TimePoint, LogId), BBox2D>,
    pub image: BTreeMap<(TimePoint, LogId), Image>,

    // 3D:
    pub pos3: BTreeMap<(TimePoint, LogId), [f32; 3]>,
    pub box3: BTreeSet<(TimePoint, LogId)>,
    pub paths_3d: BTreeSet<(TimePoint, LogId)>,
    pub line_segments_3d: BTreeMap<(TimePoint, LogId), Vec<[[f32; 3]; 2]>>,
    pub meshes: BTreeSet<(TimePoint, LogId)>,

    // N-D:
    pub vecf32: BTreeMap<(TimePoint, LogId), Vec<f32>>,
}

impl DataColumns {
    pub fn add(&mut self, msg: &LogMsg) {
        #![allow(clippy::clone_on_copy)] // for symmetry
        let when = (msg.time_point.clone(), msg.id);
        match &msg.data {
            Data::I32(data) => {
                self.i32.insert(when, data.clone());
            }
            Data::F32(data) => {
                self.f32.insert(when, data.clone());
            }
            Data::Color(color) => {
                self.color.insert(when, color.clone());
            }
            Data::Pos2(data) => {
                self.pos2.insert(when, data.clone());
            }
            Data::BBox2D(data) => {
                self.bbox2d.insert(when, data.clone());
            }
            Data::LineSegments2D(_) => {
                self.line_segments_2d.insert(when);
            }
            Data::Image(data) => {
                self.image.insert(when, data.clone());
            }
            Data::Pos3(data) => {
                self.pos3.insert(when, data.clone());
            }
            Data::Box3(_) => {
                self.box3.insert(when);
            }
            Data::Path3D(_) => {
                self.paths_3d.insert(when);
            }
            Data::LineSegments3D(data) => {
                self.line_segments_3d.insert(when, data.clone());
            }
            Data::Mesh3D(_) => {
                self.meshes.insert(when);
            }
            Data::Vecf32(data) => {
                self.vecf32.insert(when, data.clone());
            }
        }
    }

    pub fn summary(&self) -> String {
        let Self {
            i32,
            f32,
            color,
            pos2,
            line_segments_2d,
            bbox2d,
            image,
            pos3,
            box3,
            paths_3d,
            line_segments_3d,
            meshes,
            vecf32,
        } = self;

        let mut summaries = vec![];

        if !i32.is_empty() {
            summaries.push(plurality(i32.len(), "integer", "s"));
        }
        if !f32.is_empty() {
            summaries.push(plurality(f32.len(), "float", "s"));
        }
        if !color.is_empty() {
            summaries.push(plurality(color.len(), "color", "s"));
        }

        if !pos2.is_empty() {
            summaries.push(plurality(pos2.len(), "2D position", "s"));
        }
        if !line_segments_2d.is_empty() {
            summaries.push(plurality(
                line_segments_2d.len(),
                "2D line segment list",
                "s",
            ));
        }
        if !bbox2d.is_empty() {
            summaries.push(plurality(bbox2d.len(), "2D bounding box", "es"));
        }
        if !image.is_empty() {
            summaries.push(plurality(image.len(), "image", "s"));
        }

        if !pos3.is_empty() {
            summaries.push(plurality(pos3.len(), "3D position", "s"));
        }
        if !box3.is_empty() {
            summaries.push(plurality(box3.len(), "3D box", "es"));
        }
        if !paths_3d.is_empty() {
            summaries.push(plurality(paths_3d.len(), "3D path", "s"));
        }
        if !line_segments_3d.is_empty() {
            summaries.push(plurality(
                line_segments_3d.len(),
                "3D line segment list",
                "s",
            ));
        }
        if !meshes.is_empty() {
            summaries.push(plurality(meshes.len(), "mesh", "es"));
        }

        if !vecf32.is_empty() {
            summaries.push(plurality(vecf32.len(), "float vector", "s"));
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
