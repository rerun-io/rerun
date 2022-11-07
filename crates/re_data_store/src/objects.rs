use std::collections::BTreeMap;

use ahash::HashMap;
use nohash_hasher::IntMap;
use re_log_types::{objects::*, DataVec, FieldName, IndexHash, MsgId, ObjPath, ObjTypePath};

use crate::{query::*, ObjStore, TimeQuery, TimelineStore};

/// Common properties of an object instance.
#[derive(Copy, Clone, Debug)]
pub struct InstanceProps<'s> {
    // NOTE: While we would normally make InstanceProps generic over time
    // (`InstanceProps<'s, Time`>), doing so leads to a gigantic template-leak that
    // propagates all over the codebase.
    // So for now we will constrain ourselves to an i64 here, which is the only unit
    // of time we currently use in practice anyway.
    pub time: i64,
    pub msg_id: &'s MsgId,
    pub color: Option<[u8; 4]>,

    /// Use this to test if the object should be visible, etc.
    pub obj_path: &'s ObjPath,

    /// If it is a multi-object, this is the instance index,
    /// else it is [`IndexHash::NONE`].
    pub instance_index: IndexHash,

    /// Whether or not the object is visible
    pub visible: bool,
}

#[derive(Copy, Clone, Debug)]
struct Object<'s, T: Copy + Clone + std::fmt::Debug> {
    pub props: InstanceProps<'s>,
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

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&InstanceProps<'s>, &T)> {
        self.0.iter().map(|obj| (&obj.props, &obj.data))
    }

    pub fn first(&self) -> Option<(&InstanceProps<'s>, &T)> {
        self.0.first().map(|obj| (&obj.props, &obj.data))
    }

    pub fn last(&self) -> Option<(&InstanceProps<'s>, &T)> {
        self.0.last().map(|obj| (&obj.props, &obj.data))
    }

    pub fn get(&self, idx: usize) -> Option<(&InstanceProps<'s>, &T)> {
        self.0.get(idx).map(|obj| (&obj.props, &obj.data))
    }

    pub fn filter(&self, keep: &impl Fn(&InstanceProps<'_>) -> bool) -> Self {
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
pub struct LineSegments2D<'s> {
    /// Connected pair-wise even-odd.
    pub points: &'s Vec<[f32; 2]>,
    pub stroke_width: Option<f32>,
}

impl<'s> LineSegments2D<'s> {
    fn query<Time: 'static + Copy + Ord + Into<i64>>(
        obj_path: &'s ObjPath,
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_3(
            obj_store,
            &FieldName::from("points"),
            time_query,
            ("_visible", "color", "stroke_width"),
            |instance_index: Option<&IndexHash>,
             time: Time,
             msg_id: &MsgId,
             points: &DataVec,
             visible: Option<&bool>,
             color: Option<&[u8; 4]>,
             stroke_width: Option<&f32>| {
                if let Some(points) = points.as_vec_of_vec2("LineSegments2D::points") {
                    out.line_segments2d.0.push(Object {
                        props: InstanceProps {
                            time: time.into(),
                            msg_id,
                            color: color.copied(),
                            obj_path,
                            instance_index: instance_index.copied().unwrap_or(IndexHash::NONE),
                            visible: *visible.unwrap_or(&true),
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

#[derive(Clone, Debug)]
pub struct ClassDescriptionMap<'s> {
    pub msg_id: &'s MsgId,
    pub map: HashMap<i32, ClassDescription<'s>>,
}

#[derive(Copy, Clone, Debug)]
pub struct ClassDescription<'s> {
    pub label: Option<&'s str>,
    pub color: Option<[u8; 4]>,
}

impl<'s> ClassDescription<'s> {
    fn query<Time: 'static + Copy + Ord + Into<i64>>(
        obj_path: &'s ObjPath,
        obj_store: &'s ObjStore<Time>,
        time_query: &TimeQuery<Time>,
        out: &mut Objects<'s>,
    ) {
        crate::profile_function!();

        visit_type_data_2(
            obj_store,
            &FieldName::from("id"),
            time_query,
            ("label", "color"),
            |_instance_index: Option<&IndexHash>,
             _time,
             msg_id: &MsgId,
             id: &i32,
             label: Option<&String>,
             color: Option<&[u8; 4]>| {
                let class_description_map = out
                    .class_description_map
                    .entry(obj_path)
                    .or_insert_with(|| ClassDescriptionMap {
                        msg_id,
                        map: HashMap::<i32, ClassDescription<'s>>::default(),
                    });

                class_description_map.map.insert(
                    *id,
                    ClassDescription {
                        label: label.map(|s| s.as_str()),
                        color: color.cloned(),
                    },
                );
            },
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct Objects<'s> {
    pub class_description_map: BTreeMap<&'s ObjPath, ClassDescriptionMap<'s>>,

    pub line_segments2d: ObjectVec<'s, LineSegments2D<'s>>,
}

impl<'s> Objects<'s> {
    pub fn query<Time: 'static + Copy + Ord + Into<i64>>(
        &mut self,
        store: &'s TimelineStore<Time>,
        time_query: &'_ TimeQuery<Time>,
        obj_types: &IntMap<ObjTypePath, ObjectType>,
    ) {
        crate::profile_function!();

        for (obj_path, obj_store) in store.iter() {
            if let Some(obj_type) = obj_types.get(obj_path.obj_type_path()) {
                self.query_object(obj_store, time_query, obj_path, obj_type);
            } else {
                // Not every path is an object, and that's fine.
                // Some paths just contains a `_transform`, for instance.
            }
        }
    }

    pub fn query_object<Time: 'static + Copy + Ord + Into<i64>>(
        &mut self,
        obj_store: &'s ObjStore<Time>,
        time_query: &'_ TimeQuery<Time>,
        obj_path: &'s ObjPath,
        obj_type: &ObjectType,
    ) {
        let query_fn = match obj_type {
            ObjectType::ClassDescription => ClassDescription::query,
            ObjectType::LineSegments2D => LineSegments2D::query,
            ObjectType::Point3D
            | ObjectType::TextEntry
            | ObjectType::Box3D
            | ObjectType::Path3D
            | ObjectType::LineSegments3D
            | ObjectType::Mesh3D
            | ObjectType::Arrow3D
            | ObjectType::Image
            | ObjectType::BBox2D
            | ObjectType::Point2D => return, // TODO
        };

        query_fn(obj_path, obj_store, time_query, self);
    }

    pub fn filter(&self, keep: impl Fn(&InstanceProps<'_>) -> bool) -> Self {
        crate::profile_function!();

        Self {
            class_description_map: self.class_description_map.clone(), // SPECIAL - can't filter

            line_segments2d: self.line_segments2d.filter(&keep),
        }
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            class_description_map,
            line_segments2d,
        } = self;
        class_description_map.is_empty() && line_segments2d.is_empty()
    }

    pub fn len(&self) -> usize {
        let Self {
            class_description_map,
            line_segments2d,
        } = self;
        class_description_map.len() + line_segments2d.len()
    }

    pub fn has_any_2d(&self) -> bool {
        !self.line_segments2d.is_empty()
    }

    pub fn has_any_3d(&self) -> bool {
        false
    }
}
