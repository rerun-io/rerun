use std::collections::BTreeMap;

use log_types::{FieldName, LogId, ObjPath};

pub use log_types::objects::*;

use crate::{
    storage::{visit_data, visit_data_and_2_children, visit_data_and_3_children, ObjStore},
    ObjTypePath, TimeQuery, TypePathDataStore,
};

#[derive(Copy, Clone, Debug)]
pub struct ObjectProps<'s> {
    pub log_id: &'s LogId,
    pub space: Option<&'s ObjPath>,
    pub color: Option<[u8; 4]>,

    /// Use this to test if the object should be visible, etc.
    pub parent_obj_path: &'s ObjPath,
}

#[derive(Copy, Clone, Debug)]
struct Object<'s, T: Copy + Clone + std::fmt::Debug> {
    pub props: ObjectProps<'s>,
    pub obj: T,
}

#[derive(Clone, Debug)]
pub struct ObjectMap<'s, T: Clone + Copy + std::fmt::Debug>(
    BTreeMap<ObjTypePath, Vec<Object<'s, T>>>,
);

impl<'s, T: Clone + Copy + std::fmt::Debug> Default for ObjectMap<'s, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'s, T: Clone + Copy + std::fmt::Debug> ObjectMap<'s, T> {
    /// Total number of objects
    pub fn len(&self) -> usize {
        self.0.values().map(|vec| vec.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ObjTypePath, &ObjectProps<'s>, &T)> {
        self.0.iter().flat_map(|(type_path, vec)| {
            vec.iter().map(move |obj| (type_path, &obj.props, &obj.obj))
        })
    }

    fn filter(&self, keep: &impl Fn(&ObjectProps<'_>) -> bool) -> ObjectMap<'s, T> {
        use itertools::Itertools as _;
        Self(
            self.0
                .iter()
                .filter_map(|(tp, vec)| {
                    let vec = vec.iter().filter(|x| keep(&x.props)).copied().collect_vec();
                    if vec.is_empty() {
                        None
                    } else {
                        Some((tp.clone(), vec))
                    }
                })
                .collect(),
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Image<'s> {
    pub image: &'s log_types::Image,
}

impl<'s> Image<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Image>(&FieldName::from("image")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                primary_data,
                ("space", "color"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 image: &log_types::Image,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Image { image },
                    });
                },
            );
            out.image.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point2D<'s> {
    pub pos: &'s [f32; 2],
    pub radius: Option<f32>,
}

impl<'s> Point2D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 2]>(&FieldName::from("pos")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "radius"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 pos: &[f32; 2],
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Point2D {
                            pos,
                            radius: radius.copied(),
                        },
                    });
                },
            );

            out.point2d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point3D<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

impl<'s> Point3D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 3]>(&FieldName::from("pos")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "radius"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 pos: &[f32; 3],
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Point3D {
                            pos,
                            radius: radius.copied(),
                        },
                    });
                },
            );

            out.point3d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BBox2D<'s> {
    pub bbox: &'s log_types::BBox2D,
    pub stroke_width: Option<f32>,
}

impl<'s> BBox2D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::BBox2D>(&FieldName::from("bbox")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 bbox: &log_types::BBox2D,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: BBox2D {
                            bbox,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            out.bbox2d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Box3D<'s> {
    pub obb: &'s log_types::Box3,
    pub stroke_width: Option<f32>,
}

impl<'s> Box3D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Box3>(&FieldName::from("obb")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 obb: &log_types::Box3,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Box3D {
                            obb,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            out.box3d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Path3D<'s> {
    pub points: &'s Vec<[f32; 3]>,
    pub stroke_width: Option<f32>,
}

impl<'s> Path3D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<Vec<[f32; 3]>>(&FieldName::from("points")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 points: &Vec<[f32; 3]>,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Path3D {
                            points,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            out.path3d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments2D<'s> {
    pub line_segments: &'s Vec<[[f32; 2]; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments2D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) =
            store.get::<Vec<[[f32; 2]; 2]>>(&FieldName::from("line_segments"))
        {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 line_segments: &Vec<[[f32; 2]; 2]>,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: LineSegments2D {
                            line_segments,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            out.line_segments2d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments3D<'s> {
    pub line_segments: &'s Vec<[[f32; 3]; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments3D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) =
            store.get::<Vec<[[f32; 3]; 2]>>(&FieldName::from("line_segments"))
        {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 line_segments: &Vec<[[f32; 3]; 2]>,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: LineSegments3D {
                            line_segments,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            out.line_segments3d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Mesh3D<'s> {
    pub mesh: &'s log_types::Mesh3D,
}

impl<'s> Mesh3D<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Mesh3D>(&FieldName::from("mesh")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                primary_data,
                ("space", "color"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 mesh: &log_types::Mesh3D,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Mesh3D { mesh },
                    });
                },
            );

            out.mesh3d.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Camera<'s> {
    // TODO: break up in parts
    pub camera: &'s log_types::Camera,
}

impl<'s> Camera<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Camera>(&FieldName::from("camera")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                primary_data,
                ("space", "color"),
                |parent_obj_path: &ObjPath,
                 log_id: &LogId,
                 camera: &log_types::Camera,
                 space: Option<&ObjPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_obj_path,
                        },
                        obj: Camera { camera },
                    });
                },
            );

            out.camera.0.insert(obj_type_path.clone(), vec);
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Space<'s> {
    /// The up axis
    pub up: &'s [f32; 3],
}

impl<'s> Space<'s> {
    fn query<Time: 'static + Clone + Ord>(
        store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        _obj_type_path: &ObjTypePath,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 3]>(&FieldName::from("up")) {
            visit_data(
                time_query,
                primary_data,
                |parent_obj_path: &ObjPath, _log_id: &LogId, up: &[f32; 3]| {
                    out.space.insert(parent_obj_path, Space { up });
                },
            );
        }
    }
}

#[derive(Debug, Default)]
pub struct Objects<'s> {
    pub image: ObjectMap<'s, Image<'s>>,
    pub point2d: ObjectMap<'s, Point2D<'s>>,
    pub bbox2d: ObjectMap<'s, BBox2D<'s>>,
    pub line_segments2d: ObjectMap<'s, LineSegments2D<'s>>,

    pub point3d: ObjectMap<'s, Point3D<'s>>,
    pub box3d: ObjectMap<'s, Box3D<'s>>,
    pub path3d: ObjectMap<'s, Path3D<'s>>,
    pub line_segments3d: ObjectMap<'s, LineSegments3D<'s>>,
    pub mesh3d: ObjectMap<'s, Mesh3D<'s>>,
    pub camera: ObjectMap<'s, Camera<'s>>,

    pub space: BTreeMap<&'s ObjPath, Space<'s>>, // SPECIAL!
}

impl<'s> Objects<'s> {
    pub fn query_object<Time: 'static + Clone + Ord>(
        &mut self,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
        obj_type_path: &ObjTypePath,
        object_type: ObjectType,
    ) {
        crate::profile_function!();

        if let Some(obj_store) = store.get(obj_type_path) {
            let query_fn = match object_type {
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

            query_fn(obj_store, time_query, obj_type_path, self);
        }
    }

    pub fn filter(&self, keep: impl Fn(&ObjectProps<'_>) -> bool) -> Self {
        crate::profile_function!();
        Self {
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

            space: self.space.clone(), // SPECIAL don't filter
        }
    }
}
