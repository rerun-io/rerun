use std::collections::BTreeMap;

use log_types::{DataPath, LogId};

use crate::{
    storage::{visit_data_and_2_siblings, visit_data_and_3_siblings},
    TimeQuery, TypePath, TypePathDataStore,
};

#[derive(Copy, Clone, Debug)]
pub struct Object<'s, T: Copy + Clone + std::fmt::Debug> {
    pub log_id: &'s LogId,
    pub space: Option<&'s DataPath>,
    /// Use this to test if the object should be visible, etc.
    pub parent_object_path: &'s DataPath,
    pub obj: T,
}

#[derive(Clone)]
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

    pub fn iter(&self) -> impl Iterator<Item = (&TypePath, &Object<'s, T>)> {
        self.0
            .iter()
            .flat_map(|(type_path, vec)| vec.iter().map(move |obj| (type_path, obj)))
    }

    fn filter_space(&self, space: &DataPath) -> ObjectMap<'s, T> {
        use itertools::Itertools as _;
        Self(
            self.0
                .iter()
                .filter_map(|(tp, vec)| {
                    let vec = vec
                        .iter()
                        .filter(|x| x.space == Some(space))
                        .copied()
                        .collect_vec();
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
    pub color: Option<[u8; 4]>,
}

impl<'s> Image<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Image<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<log_types::Image>() {
                let mut vec = vec![];
                visit_data_and_2_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     image: &log_types::Image,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Image {
                                image,
                                color: color.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point2D<'s> {
    pub pos: &'s [f32; 2],
    pub color: Option<[u8; 4]>,
    pub radius: Option<f32>,
}

impl<'s> Point2D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Point2D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<[f32; 2]>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "radius"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     pos: &[f32; 2],
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Point2D {
                                pos,
                                color: color.copied(),
                                radius: radius.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point3D<'s> {
    pub pos: &'s [f32; 3],
    pub color: Option<[u8; 4]>,
    pub radius: Option<f32>,
}

impl<'s> Point3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Point3D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<[f32; 3]>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "radius"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     pos: &[f32; 3],
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Point3D {
                                pos,
                                color: color.copied(),
                                radius: radius.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BBox2D<'s> {
    pub bbox: &'s log_types::BBox2D,
    pub color: Option<[u8; 4]>,
    pub stroke_width: Option<f32>,
}

impl<'s> BBox2D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, BBox2D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<log_types::BBox2D>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "stroke_width"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     bbox: &log_types::BBox2D,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: BBox2D {
                                bbox,
                                color: color.copied(),
                                stroke_width: stroke_width.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Box3D<'s> {
    pub bbox: &'s log_types::Box3,
    pub color: Option<[u8; 4]>,
    pub stroke_width: Option<f32>,
}

impl<'s> Box3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Box3D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<log_types::Box3>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "stroke_width"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     bbox: &log_types::Box3,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Box3D {
                                bbox,
                                color: color.copied(),
                                stroke_width: stroke_width.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Path3D<'s> {
    pub points: &'s Vec<[f32; 3]>,
    pub color: Option<[u8; 4]>,
    pub stroke_width: Option<f32>,
}

impl<'s> Path3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Path3D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<Vec<[f32; 3]>>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "stroke_width"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     points: &Vec<[f32; 3]>,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Path3D {
                                points,
                                color: color.copied(),
                                stroke_width: stroke_width.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments2D<'s> {
    pub line_segments: &'s Vec<[[f32; 2]; 2]>,
    pub color: Option<[u8; 4]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments2D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, LineSegments2D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<Vec<[[f32; 2]; 2]>>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "stroke_width"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     line_segments: &Vec<[[f32; 2]; 2]>,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: LineSegments2D {
                                line_segments,
                                color: color.copied(),
                                stroke_width: stroke_width.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LineSegments3D<'s> {
    pub line_segments: &'s Vec<[[f32; 3]; 2]>,
    pub color: Option<[u8; 4]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, LineSegments3D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<Vec<[[f32; 3]; 2]>>() {
                let mut vec = vec![];
                visit_data_and_3_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color", "stroke_width"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     line_segments: &Vec<[[f32; 3]; 2]>,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: LineSegments3D {
                                line_segments,
                                color: color.copied(),
                                stroke_width: stroke_width.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Mesh3D<'s> {
    pub mesh: &'s log_types::Mesh3D,
    pub color: Option<[u8; 4]>,
}

impl<'s> Mesh3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Mesh3D<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<log_types::Mesh3D>() {
                let mut vec = vec![];
                visit_data_and_2_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     mesh: &log_types::Mesh3D,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Mesh3D {
                                mesh,
                                color: color.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Camera<'s> {
    // TODO: break up in parts
    pub camera: &'s log_types::Camera,
    pub color: Option<[u8; 4]>,
}

impl<'s> Camera<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Camera<'s>> {
        crate::profile_function!();

        let mut all = BTreeMap::default();

        for (type_path, data_store) in store.iter() {
            if let Some(primary_data) = data_store.read_no_warn::<log_types::Camera>() {
                let mut vec = vec![];
                visit_data_and_2_siblings(
                    store,
                    time_query,
                    type_path,
                    primary_data,
                    ("space", "color"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     camera: &log_types::Camera,
                     space: Option<&DataPath>,
                     color: Option<&[u8; 4]>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Camera {
                                camera,
                                color: color.copied(),
                            },
                        });
                    },
                );
                all.insert(parent(type_path), vec);
            }
        }

        ObjectMap(all)
    }
}

#[derive(Default)]
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
}

impl<'s> Objects<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Self {
        Self {
            image: Image::from_store(store, time_query),
            point2d: Point2D::from_store(store, time_query),
            bbox2d: BBox2D::from_store(store, time_query),
            line_segments2d: LineSegments2D::from_store(store, time_query),

            point3d: Point3D::from_store(store, time_query),
            box3d: Box3D::from_store(store, time_query),
            path3d: Path3D::from_store(store, time_query),
            line_segments3d: LineSegments3D::from_store(store, time_query),
            mesh3d: Mesh3D::from_store(store, time_query),
            camera: Camera::from_store(store, time_query),
        }
    }

    /// Only keep data that has this space
    pub fn filter_space(&self, space: &DataPath) -> Self {
        crate::profile_function!();
        Self {
            image: self.image.filter_space(space),
            point2d: self.point2d.filter_space(space),
            bbox2d: self.bbox2d.filter_space(space),
            line_segments2d: self.line_segments2d.filter_space(space),

            point3d: self.point3d.filter_space(space),
            box3d: self.box3d.filter_space(space),
            path3d: self.path3d.filter_space(space),
            line_segments3d: self.line_segments3d.filter_space(space),
            mesh3d: self.mesh3d.filter_space(space),
            camera: self.camera.filter_space(space),
        }
    }
}

fn parent(type_path: &TypePath) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path
}
