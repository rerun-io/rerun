use std::collections::{BTreeMap, BTreeSet, HashMap};

use re_log_types::*;
use re_string_interner::InternedString;

/// Tree of data paths.
#[derive(Default)]
pub struct ObjectTree {
    /// Children of type [`ObjPathComp::Name`].
    pub named_children: BTreeMap<InternedString, ObjectTree>,

    /// Children of type [`ObjPathComp::Index`].
    pub index_children: BTreeMap<Index, ObjectTree>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: BTreeMap<TimeSource, BTreeMap<TimeInt, BTreeSet<MsgId>>>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldName, DataColumns>,
}

impl ObjectTree {
    /// Has no child objects.
    pub fn is_leaf(&self) -> bool {
        self.named_children.is_empty() && self.index_children.is_empty()
    }

    pub fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) {
        crate::profile_function!();
        let obj_path = data_path.obj_path.to_components();
        self.add_path(
            obj_path.as_slice(),
            data_path.field_name,
            msg_id,
            time_point,
            data,
        );
    }

    pub(crate) fn add_path(
        &mut self,
        path: &[ObjPathComp],
        field_name: FieldName,
        msg_id: MsgId,
        time_point: &TimePoint,
        data: &LoggedData,
    ) {
        for (time_source, time_value) in &time_point.0 {
            self.prefix_times
                .entry(*time_source)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        match path {
            [] => {
                self.fields
                    .entry(field_name)
                    .or_default()
                    .add(msg_id, time_point, data);
            }
            [first, rest @ ..] => match first {
                ObjPathComp::Name(name) => {
                    self.named_children
                        .entry(*name)
                        .or_default()
                        .add_path(rest, field_name, msg_id, time_point, data);
                }
                ObjPathComp::Index(index) => {
                    self.index_children
                        .entry(index.clone())
                        .or_default()
                        .add_path(rest, field_name, msg_id, time_point, data);
                }
            },
        }
    }
}

/// Column transform of [`Data`].
#[derive(Default)]
pub struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<TimeSource, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    pub per_type: HashMap<DataType, BTreeSet<MsgId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg_id: MsgId, time_point: &TimePoint, data: &LoggedData) {
        for (time_source, time_value) in &time_point.0 {
            self.times
                .entry(*time_source)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        self.per_type
            .entry(data.data_type())
            .or_default()
            .insert(msg_id);
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

                DataType::Vec3 => ("3D vector", "s"),
                DataType::Box3 => ("3D box", "es"),
                DataType::Mesh3D => ("mesh", "es"),
                DataType::Camera => ("camera", "s"),

                DataType::Tensor => ("tensor", "s"),

                DataType::Space => ("space", "s"),

                DataType::Transform => ("transform", "s"),

                DataType::DataVec => ("vector", "s"),
            };

            summaries.push(plurality(set.len(), stem, plur));
        }

        summaries.join(", ")
    }
}

pub(crate) fn plurality(num: usize, singular: &str, plural_suffix: &str) -> String {
    if num == 1 {
        format!("1 {}", singular)
    } else {
        format!("{} {}{}", num, singular, plural_suffix)
    }
}
