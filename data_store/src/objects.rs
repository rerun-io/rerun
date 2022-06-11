use std::collections::BTreeMap;

use log_types::{DataPath, LogId};

pub use log_types::objects::*;

use crate::{
    storage::{visit_data, visit_data_and_2_children, visit_data_and_3_children},
    TimeQuery, TypePath, TypePathDataStore,
};

#[derive(Copy, Clone, Debug)]
pub struct ObjectProps<'s> {
    pub log_id: &'s LogId,
    pub space: Option<&'s DataPath>,
    pub color: Option<[u8; 4]>,

    /// Use this to test if the object should be visible, etc.
    pub parent_object_path: &'s DataPath,
}

#[derive(Copy, Clone, Debug)]
struct Object<'s, T: Copy + Clone + std::fmt::Debug> {
    pub props: ObjectProps<'s>,
    pub obj: T,
}

#[derive(Clone, Debug)]
pub struct ObjectMap<'s, T: Clone + Copy + std::fmt::Debug>(BTreeMap<TypePath, Vec<Object<'s, T>>>);

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

    pub fn iter(&self) -> impl Iterator<Item = (&TypePath, &ObjectProps<'s>, &T)> {
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
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Image<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Image>(&(object_type_path / "image")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 image: &log_types::Image,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Image { image },
                    });
                },
            );
            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point2D<'s> {
    pub pos: &'s [f32; 2],
    pub radius: Option<f32>,
}

impl<'s> Point2D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Point2D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 2]>(&(object_type_path / "pos")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "radius"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 pos: &[f32; 2],
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Point2D {
                            pos,
                            radius: radius.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point3D<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

impl<'s> Point3D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Point3D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 3]>(&(object_type_path / "pos")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "radius"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 pos: &[f32; 3],
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Point3D {
                            pos,
                            radius: radius.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BBox2D<'s> {
    pub bbox: &'s log_types::BBox2D,
    pub stroke_width: Option<f32>,
}

impl<'s> BBox2D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, BBox2D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::BBox2D>(&(object_type_path / "bbox")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 bbox: &log_types::BBox2D,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: BBox2D {
                            bbox,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Box3D<'s> {
    pub obb: &'s log_types::Box3,
    pub stroke_width: Option<f32>,
}

impl<'s> Box3D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Box3D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Box3>(&(object_type_path / "obb")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 obb: &log_types::Box3,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Box3D {
                            obb,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Path3D<'s> {
    pub points: &'s Vec<[f32; 3]>,
    pub stroke_width: Option<f32>,
}

impl<'s> Path3D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Path3D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<Vec<[f32; 3]>>(&(object_type_path / "points")) {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 points: &Vec<[f32; 3]>,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Path3D {
                            points,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments2D<'s> {
    pub line_segments: &'s Vec<[[f32; 2]; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments2D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, LineSegments2D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) =
            store.get::<Vec<[[f32; 2]; 2]>>(&(object_type_path / "line_segments)"))
        {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 line_segments: &Vec<[[f32; 2]; 2]>,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: LineSegments2D {
                            line_segments,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments3D<'s> {
    pub line_segments: &'s Vec<[[f32; 3]; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments3D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, LineSegments3D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) =
            store.get::<Vec<[[f32; 3]; 2]>>(&(object_type_path / "line_segments)"))
        {
            let mut vec = vec![];
            visit_data_and_3_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color", "stroke_width"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 line_segments: &Vec<[[f32; 3]; 2]>,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: LineSegments3D {
                            line_segments,
                            stroke_width: stroke_width.copied(),
                        },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Mesh3D<'s> {
    pub mesh: &'s log_types::Mesh3D,
}

impl<'s> Mesh3D<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Mesh3D<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Mesh3D>(&(object_type_path / "mesh")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 mesh: &log_types::Mesh3D,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Mesh3D { mesh },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Camera<'s> {
    // TODO: break up in parts
    pub camera: &'s log_types::Camera,
}

impl<'s> Camera<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Vec<Object<'s, Camera<'s>>> {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<log_types::Camera>(&(object_type_path / "camera")) {
            let mut vec = vec![];
            visit_data_and_2_children(
                store,
                time_query,
                object_type_path,
                primary_data,
                ("space", "color"),
                |parent_object_path: &DataPath,
                 log_id: &LogId,
                 camera: &log_types::Camera,
                 space: Option<&DataPath>,
                 color: Option<&[u8; 4]>| {
                    vec.push(Object {
                        props: ObjectProps {
                            log_id,
                            space,
                            color: color.copied(),
                            parent_object_path,
                        },
                        obj: Camera { camera },
                    });
                },
            );

            vec
        } else {
            vec![] // nothing logged yet
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Space<'s> {
    /// The up axis
    pub up: &'s [f32; 3],
}

impl<'s> Space<'s> {
    fn read<Time: 'static + Clone + Ord>(
        object_type_path: &TypePath,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut BTreeMap<&'s DataPath, Space<'s>>,
    ) {
        crate::profile_function!();

        if let Some(primary_data) = store.get::<[f32; 3]>(&(object_type_path / "up")) {
            visit_data(
                time_query,
                primary_data,
                |parent_object_path: &DataPath, _log_id: &LogId, up: &[f32; 3]| {
                    out.insert(parent_object_path, Space { up });
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

    pub space: BTreeMap<&'s DataPath, Space<'s>>, // SPECIAL!
}

impl<'s> Objects<'s> {
    pub fn query_object<Time: 'static + Clone + Ord>(
        &mut self,
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
        object_type_path: &TypePath,
        object_type: ObjectType,
    ) {
        crate::profile_function!();

        match object_type {
            ObjectType::Space => {
                Space::read(object_type_path, store, time_query, &mut self.space);
            }
            ObjectType::Image => {
                self.image.0.insert(
                    object_type_path.clone(),
                    Image::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Point2D => {
                self.point2d.0.insert(
                    object_type_path.clone(),
                    Point2D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::BBox2D => {
                self.bbox2d.0.insert(
                    object_type_path.clone(),
                    BBox2D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::LineSegments2D => {
                self.line_segments2d.0.insert(
                    object_type_path.clone(),
                    LineSegments2D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Point3D => {
                self.point3d.0.insert(
                    object_type_path.clone(),
                    Point3D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Box3D => {
                self.box3d.0.insert(
                    object_type_path.clone(),
                    Box3D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Path3D => {
                self.path3d.0.insert(
                    object_type_path.clone(),
                    Path3D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::LineSegments3D => {
                self.line_segments3d.0.insert(
                    object_type_path.clone(),
                    LineSegments3D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Mesh3D => {
                self.mesh3d.0.insert(
                    object_type_path.clone(),
                    Mesh3D::read(object_type_path, store, time_query),
                );
            }
            ObjectType::Camera => {
                self.camera.0.insert(
                    object_type_path.clone(),
                    Camera::read(object_type_path, store, time_query),
                );
            }
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
