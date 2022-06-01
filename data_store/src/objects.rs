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
    pub pos2d: &'s [f32; 2],
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
                    ("color", "radius", "space"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     pos2d: &[f32; 2],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     space: Option<&DataPath>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Point2D {
                                pos2d,
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
    pub radius: Option<f32>,
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
                    ("color", "radius", "space"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     bbox: &log_types::BBox2D,
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     space: Option<&DataPath>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: BBox2D {
                                bbox,
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
pub struct LineSegments2D<'s> {
    pub line_segments: &'s Vec<[[f32; 2]; 2]>,
    pub color: Option<[u8; 4]>,
    pub radius: Option<f32>,
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
                    ("color", "radius", "space"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     line_segments: &Vec<[[f32; 2]; 2]>,
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     space: Option<&DataPath>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: LineSegments2D {
                                line_segments,
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

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos3d: &'s [f32; 3],
    pub color: Option<[u8; 4]>,
    pub radius: Option<f32>,
}

impl<'s> Point3<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> ObjectMap<'s, Point3<'s>> {
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
                    ("color", "radius", "space"),
                    |parent_object_path: &DataPath,
                     log_id: &LogId,
                     pos3d: &[f32; 3],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     space: Option<&DataPath>| {
                        vec.push(Object {
                            log_id,
                            space,
                            parent_object_path,
                            obj: Point3 {
                                pos3d,
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

#[derive(Default)]
pub struct Objects<'s> {
    pub image: ObjectMap<'s, Image<'s>>,
    pub point2d: ObjectMap<'s, Point2D<'s>>,
    pub bbox2d: ObjectMap<'s, BBox2D<'s>>,
    pub line_segments2d: ObjectMap<'s, LineSegments2D<'s>>,

    pub point3d: ObjectMap<'s, Point3<'s>>,
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

            point3d: Point3::from_store(store, time_query),
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
        }
    }
}

fn parent(type_path: &TypePath) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path
}
