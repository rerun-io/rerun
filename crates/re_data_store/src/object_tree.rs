use std::collections::{BTreeMap, BTreeSet, HashMap};

use re_log_types::*;

/// Tree of data paths.
pub struct ObjectTree {
    /// Full path to the root of this tree.
    pub path: ObjPath,

    pub children: BTreeMap<ObjPathComp, ObjectTree>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: BTreeMap<Timeline, BTreeMap<TimeInt, BTreeSet<MsgId>>>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldName, DataColumns>,
}

impl ObjectTree {
    pub fn root() -> Self {
        Self::new(ObjPath::root())
    }

    pub fn new(path: ObjPath) -> Self {
        Self {
            path,
            children: Default::default(),
            prefix_times: Default::default(),
            fields: Default::default(),
        }
    }

    /// Has no child objects.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn num_children_and_fields(&self) -> usize {
        self.children.len() + self.fields.len()
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

        let leaf = self.create_subtrees_recursively(obj_path.as_slice(), 0, msg_id, time_point);

        leaf.fields
            .entry(data_path.field_name)
            .or_default()
            .add(msg_id, time_point, data);
    }

    fn create_subtrees_recursively(
        &mut self,
        full_path: &[ObjPathComp],
        depth: usize,
        msg_id: MsgId,
        time_point: &TimePoint,
    ) -> &mut Self {
        for (timeline, time_value) in &time_point.0 {
            self.prefix_times
                .entry(*timeline)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        match full_path.get(depth) {
            None => {
                self // end of path
            }
            Some(component) => self
                .children
                .entry(component.clone())
                .or_insert_with(|| ObjectTree::new(full_path[..depth + 1].into()))
                .create_subtrees_recursively(full_path, depth + 1, msg_id, time_point),
        }
    }
}

/// Column transform of [`Data`].
#[derive(Default)]
pub struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<Timeline, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    pub per_type: HashMap<DataType, BTreeSet<MsgId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg_id: MsgId, time_point: &TimePoint, data: &LoggedData) {
        for (timeline, time_value) in &time_point.0 {
            self.times
                .entry(*timeline)
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
                DataType::Bool => ("bool", "s"),
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
