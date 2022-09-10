use std::collections::BTreeMap;

use re_log_types::{DataVec, FieldName, MsgId, ObjPath};

pub use re_log_types::objects::*;

use crate::{storage::ObjStore, type_path_query::*, ObjTypePath, TimeQuery, TypePathDataStore};

#[derive(Copy, Clone, Debug)]
pub struct ObjectProps<'s> {
    pub msg_id: &'s MsgId,
    pub space: Option<&'s ObjPath>,
    pub color: Option<[u8; 4]>,

    /// Use this to test if the object should be visible, etc.
    pub obj_path: &'s ObjPath,
}

#[derive(Copy, Clone, Debug)]
struct Object<'s, T: Copy + Clone + std::fmt::Debug> {
    pub props: ObjectProps<'s>,
    pub data: T,
}

#[derive(Clone, Debug)]
pub struct ObjectVec<'s, T: Copy + Clone + std::fmt::Debug>(Vec<Object<'s, T>>);

impl<'s, T: Clone + Copy + std::fmt::Debug> Default for ObjectVec<'s, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'s, T: Clone + Copy + std::fmt::Debug> ObjectVec<'s, T> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&ObjectProps<'s>, &T)> {
        self.0.iter().map(|obj| (&obj.props, &obj.data))
    }

    pub fn first(&self) -> Option<(&ObjectProps<'s>, &T)> {
        self.0.first().map(|obj| (&obj.props, &obj.data))
    }

    pub fn last(&self) -> Option<(&ObjectProps<'s>, &T)> {
        self.0.last().map(|obj| (&obj.props, &obj.data))
    }

    pub fn filter(&self, keep: &impl Fn(&ObjectProps<'_>) -> bool) -> Self {
        crate::profile_function!();
        Self(
            self.0
                .iter()
                .filter(|obj| keep(&obj.props))
                .copied()
                .collect(),
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Image<'s> {
    pub tensor: &'s re_log_types::Tensor,

    /// If this is a depth map, how long is a meter?
    ///
    /// For instance, with a `u16` dtype one might have
    /// `meter == 1000.0` for millimeter precision
    /// up to a ~65m range.
    pub meter: Option<f32>,
}

impl<'s> Image<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("tensor"),
            time_query,
            ("space", "color", "meter"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             tensor: &re_log_types::Tensor,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             meter: Option<&f32>| {
                out.image.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Image {
                        tensor,
                        meter: meter.copied(),
                    },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point2D<'s> {
    pub pos: &'s [f32; 2],
    pub radius: Option<f32>,
}

impl<'s> Point2D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("pos"),
            time_query,
            ("space", "color", "radius"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             pos: &[f32; 2],
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             radius: Option<&f32>| {
                out.point2d.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Point2D {
                        pos,
                        radius: radius.copied(),
                    },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point3D<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

impl<'s> Point3D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("pos"),
            time_query,
            ("space", "color", "radius"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             pos: &[f32; 3],
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             radius: Option<&f32>| {
                out.point3d.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Point3D {
                        pos,
                        radius: radius.copied(),
                    },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BBox2D<'s> {
    pub bbox: &'s re_log_types::BBox2D,
    pub stroke_width: Option<f32>,
    pub label: Option<&'s str>,
}

impl<'s> BBox2D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_4(
            obj_store,
            &FieldName::from("bbox"),
            time_query,
            ("space", "color", "stroke_width", "label"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             bbox: &re_log_types::BBox2D,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>,
             label: Option<&String>| {
                out.bbox2d.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: BBox2D {
                        bbox,
                        stroke_width: stroke_width.copied(),
                        label: label.map(|s| s.as_str()),
                    },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Box3D<'s> {
    pub obb: &'s re_log_types::Box3,
    pub stroke_width: Option<f32>,
}

impl<'s> Box3D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("obb"),
            time_query,
            ("space", "color", "stroke_width"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             obb: &re_log_types::Box3,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>| {
                out.box3d.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Box3D {
                        obb,
                        stroke_width: stroke_width.copied(),
                    },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Path3D<'s> {
    pub points: &'s Vec<[f32; 3]>,
    pub stroke_width: Option<f32>,
}

impl<'s> Path3D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("points"),
            time_query,
            ("space", "color", "stroke_width"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             points: &DataVec,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>| {
                if let Some(points) = as_vec_of_vec3("Path3D::points", points) {
                    out.path3d.0.push(Object {
                        props: ObjectProps {
                            msg_id,
                            space,
                            color: color.copied(),
                            obj_path,
                        },
                        data: Path3D {
                            points,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments2D<'s> {
    /// Connected pair-wise even-odd.
    pub points: &'s Vec<[f32; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments2D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("points"),
            time_query,
            ("space", "color", "stroke_width"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             points: &DataVec,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>| {
                if let Some(points) = as_vec_of_vec2("LineSegments2D::points", points) {
                    out.line_segments2d.0.push(Object {
                        props: ObjectProps {
                            msg_id,
                            space,
                            color: color.copied(),
                            obj_path,
                        },
                        data: LineSegments2D {
                            points,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments3D<'s> {
    /// Connected pair-wise even-odd.
    pub points: &'s Vec<[f32; 3]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments3D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("points"),
            time_query,
            ("space", "color", "stroke_width"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             points: &DataVec,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>| {
                if let Some(points) = as_vec_of_vec3("LineSegments3D::points", points) {
                    out.line_segments3d.0.push(Object {
                        props: ObjectProps {
                            msg_id,
                            space,
                            color: color.copied(),
                            obj_path,
                        },
                        data: LineSegments3D {
                            points,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Mesh3D<'s> {
    pub mesh: &'s re_log_types::Mesh3D,
}

impl<'s> Mesh3D<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_2(
            obj_store,
            &FieldName::from("mesh"),
            time_query,
            ("space", "color"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             mesh: &re_log_types::Mesh3D,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>| {
                out.mesh3d.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Mesh3D { mesh },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Camera<'s> {
    // TODO(emilk): break up in parts
    pub camera: &'s re_log_types::Camera,
}

impl<'s> Camera<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_2(
            obj_store,
            &FieldName::from("camera"),
            time_query,
            ("space", "color"),
            |obj_path: &ObjPath,
             msg_id: &MsgId,
             camera: &re_log_types::Camera,
             space: Option<&ObjPath>,
             color: Option<&[u8; 4]>| {
                out.camera.0.push(Object {
                    props: ObjectProps {
                        msg_id,
                        space,
                        color: color.copied(),
                        obj_path,
                    },
                    data: Camera { camera },
                });
            },
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Space<'s> {
    /// The up axis
    pub up: &'s [f32; 3],
}

impl<'s> Space<'s> {
    fn query<Time: 'static + Copy + Ord>(
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data(
            obj_store,
            &FieldName::from("up"),
            time_query,
            |obj_path: &ObjPath, _msg_id: &MsgId, up: &[f32; 3]| {
                out.space.insert(obj_path, Space { up });
            },
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct Objects<'s> {
    pub space: BTreeMap<&'s ObjPath, Space<'s>>, // SPECIAL!

    pub image: ObjectVec<'s, Image<'s>>,
    pub point2d: ObjectVec<'s, Point2D<'s>>,
    pub bbox2d: ObjectVec<'s, BBox2D<'s>>,
    pub line_segments2d: ObjectVec<'s, LineSegments2D<'s>>,

    pub point3d: ObjectVec<'s, Point3D<'s>>,
    pub box3d: ObjectVec<'s, Box3D<'s>>,
    pub path3d: ObjectVec<'s, Path3D<'s>>,
    pub line_segments3d: ObjectVec<'s, LineSegments3D<'s>>,
    pub mesh3d: ObjectVec<'s, Mesh3D<'s>>,
    pub camera: ObjectVec<'s, Camera<'s>>,
    // be very careful when adding to this to update everything, including `viwer::misc::calc_bbox_3d`.
}

impl<'s> Objects<'s> {
    pub fn query_object<Time: 'static + Copy + Ord>(
        &mut self,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        obj_type: ObjectType,
    ) {
        crate::profile_function!();

        if let Some(obj_store) = store.get(obj_type_path) {
            let query_fn = match obj_type {
                ObjectType::Space => Space::query,
                ObjectType::Image => Image::query,
                ObjectType::Point2D => Point2D::query,
                ObjectType::BBox2D => BBox2D::query,
                ObjectType::LineSegments2D => LineSegments2D::query,
                ObjectType::Point3D => Point3D::query,
                ObjectType::Box3D => Box3D::query,
                ObjectType::Path3D => Path3D::query,
                ObjectType::LineSegments3D => LineSegments3D::query,
                ObjectType::Mesh3D => Mesh3D::query,
                ObjectType::Camera => Camera::query,
            };

            query_fn(obj_store, time_query, self);
        }
    }

    pub fn filter(&self, keep: impl Fn(&ObjectProps<'_>) -> bool) -> Self {
        crate::profile_function!();

        Self {
            space: self.space.clone(), // SPECIAL - can't filter

            image: self.image.filter(&keep),
            point2d: self.point2d.filter(&keep),
            bbox2d: self.bbox2d.filter(&keep),
            line_segments2d: self.line_segments2d.filter(&keep),

            point3d: self.point3d.filter(&keep),
            box3d: self.box3d.filter(&keep),
            path3d: self.path3d.filter(&keep),
            line_segments3d: self.line_segments3d.filter(&keep),
            mesh3d: self.mesh3d.filter(&keep),
            camera: self.camera.filter(&keep),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.space.is_empty() && !self.has_any_2d() && !self.has_any_3d()
    }

    pub fn has_any_2d(&self) -> bool {
        !self.image.is_empty()
            || !self.point2d.is_empty()
            || !self.bbox2d.is_empty()
            || !self.line_segments2d.is_empty()
    }

    pub fn has_any_3d(&self) -> bool {
        !self.point3d.is_empty()
            || !self.box3d.is_empty()
            || !self.path3d.is_empty()
            || !self.line_segments3d.is_empty()
            || !self.mesh3d.is_empty()
            || !self.camera.is_empty()
    }

    pub fn partition_on_space(self) -> ObjectsBySpace<'s> {
        crate::profile_function!();

        let mut partitioner = SpacePartitioner::default();

        for obj in self.image.0 {
            partitioner.slot(obj.props.space).image.0.push(obj);
        }
        for obj in self.point2d.0 {
            partitioner.slot(obj.props.space).point2d.0.push(obj);
        }
        for obj in self.bbox2d.0 {
            partitioner.slot(obj.props.space).bbox2d.0.push(obj);
        }
        for obj in self.line_segments2d.0 {
            partitioner
                .slot(obj.props.space)
                .line_segments2d
                .0
                .push(obj);
        }

        for obj in self.point3d.0 {
            partitioner.slot(obj.props.space).point3d.0.push(obj);
        }
        for obj in self.box3d.0 {
            partitioner.slot(obj.props.space).box3d.0.push(obj);
        }
        for obj in self.path3d.0 {
            partitioner.slot(obj.props.space).path3d.0.push(obj);
        }
        for obj in self.line_segments3d.0 {
            partitioner
                .slot(obj.props.space)
                .line_segments3d
                .0
                .push(obj);
        }
        for obj in self.mesh3d.0 {
            partitioner.slot(obj.props.space).mesh3d.0.push(obj);
        }
        for obj in self.camera.0 {
            partitioner.slot(obj.props.space).camera.0.push(obj);
        }

        let mut partitioned = partitioner.finish();

        for part in partitioned.values_mut() {
            part.space = self.space.clone(); // TODO(emilk): probably only extract the relevant space
        }

        partitioned
    }
}

/// Partitioned on space.
pub type ObjectsBySpace<'s> = ahash::HashMap<Option<&'s ObjPath>, Objects<'s>>; // TODO(emilk): nohash_hasher

#[derive(Default)]
struct SpacePartitioner<'s> {
    current_space: Option<&'s ObjPath>,
    current_objects: Objects<'s>,
    partitioned: ObjectsBySpace<'s>,
}

impl<'s> SpacePartitioner<'s> {
    fn slot(&mut self, new_space: Option<&'s ObjPath>) -> &mut Objects<'s> {
        // we often have runs of the same space, so optimize of that:
        if new_space != self.current_space {
            let new_objects = self.partitioned.remove(&new_space).unwrap_or_default();

            let prev_objects = std::mem::replace(&mut self.current_objects, new_objects);
            let prev_space = std::mem::replace(&mut self.current_space, new_space);

            self.partitioned.insert(prev_space, prev_objects);
        }
        &mut self.current_objects
    }

    fn finish(self) -> ObjectsBySpace<'s> {
        let Self {
            current_space,
            current_objects,
            mut partitioned,
        } = self;
        partitioned.insert(current_space, current_objects);
        partitioned.retain(|_, objects| !objects.is_empty());
        partitioned
    }
}

// ----------------------------------------------------------------------------

fn as_vec_of_vec2<'s>(what: &str, data_vec: &'s DataVec) -> Option<&'s Vec<[f32; 2]>> {
    if let DataVec::Vec2(vec) = data_vec {
        Some(vec)
    } else {
        log_once::warn_once!(
            "Expected {} to be Vec<Vec2>, got Vec<{:?}>",
            what,
            data_vec.element_data_type()
        );
        None
    }
}

fn as_vec_of_vec3<'s>(what: &str, data_vec: &'s DataVec) -> Option<&'s Vec<[f32; 3]>> {
    if let DataVec::Vec3(vec) = data_vec {
        Some(vec)
    } else {
        log_once::warn_once!(
            "Expected {} to be Vec<Vec3>, got Vec<{:?}>",
            what,
            data_vec.element_data_type()
        );
        None
    }
}
