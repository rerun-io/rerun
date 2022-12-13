use std::collections::{BTreeMap, BTreeSet, HashMap};

use itertools::Itertools;
use re_log_types::{
    DataPath, DataType, FieldName, LoggedData, MsgId, ObjPath, ObjPathComp, PathOp, TimeInt,
    TimePoint, Timeline,
};

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct TimesPerTimeline(BTreeMap<Timeline, BTreeMap<TimeInt, BTreeSet<MsgId>>>);

impl TimesPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&BTreeMap<TimeInt, BTreeSet<MsgId>>> {
        self.0.get(timeline)
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.0.contains_key(timeline)
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&Timeline, &BTreeMap<TimeInt, BTreeSet<MsgId>>)> {
        self.0.iter()
    }
}

// ----------------------------------------------------------------------------

/// Tree of data paths.
pub struct ObjectTree {
    /// Full path to the root of this tree.
    pub path: ObjPath,

    pub children: BTreeMap<ObjPathComp, ObjectTree>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: TimesPerTimeline,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<MsgId, TimePoint>,
    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<MsgId, TimePoint>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldName, DataColumns>,
}

impl ObjectTree {
    pub fn root() -> Self {
        Self::new(ObjPath::root(), Default::default())
    }

    pub fn new(path: ObjPath, recursive_clears: BTreeMap<MsgId, TimePoint>) -> Self {
        Self {
            path,
            children: Default::default(),
            prefix_times: Default::default(),
            nonrecursive_clears: recursive_clears.clone(),
            recursive_clears,
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

    /// Add a `LoggedData` into the object tree
    ///
    /// Returns a collection of pending clear operations
    pub fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) -> Vec<(MsgId, TimePoint)> {
        crate::profile_function!();
        let obj_path = data_path.obj_path.to_components();

        let leaf = self.create_subtrees_recursively(obj_path.as_slice(), 0, msg_id, time_point);

        let mut pending_clears = vec![];

        let fields = leaf.fields.entry(data_path.field_name).or_insert_with(|| {
            // If we needed to create a new leaf to hold this data, we also want to
            // insert all of the historical pending clear operations
            pending_clears = leaf.nonrecursive_clears.clone().into_iter().collect_vec();

            Default::default()
        });

        fields.add(msg_id, time_point, data);

        pending_clears
    }

    /// Add a path operation into the the object tree
    ///j
    /// Returns a collection of data paths to clear as a result of the operation
    /// Additional pending clear operations will be stored in the tree for future
    /// insertion.
    pub fn add_path_op(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        path_op: &PathOp,
    ) -> Vec<(DataPath, DataType, MonoOrMulti)> {
        crate::profile_function!();

        let obj_path = path_op.obj_path().to_components();

        // Look up the leaf at which we will execute the path operation
        let leaf = self.create_subtrees_recursively(obj_path.as_slice(), 0, msg_id, time_point);

        // TODO(jleibs): Refactor this as separate functions
        match path_op {
            PathOp::ClearFields(obj_path) => {
                // Track that any future fields need a Null at the right
                // time-point when added.
                leaf.nonrecursive_clears
                    .entry(msg_id)
                    .or_insert_with(|| time_point.clone());

                // For every existing field return a clear event
                leaf.fields
                    .iter()
                    .flat_map(|(field_name, fields)| {
                        fields
                            .per_type
                            .iter()
                            .map(|((data_type, multi_or_mono), _)| {
                                (
                                    DataPath::new(obj_path.clone(), *field_name),
                                    *data_type,
                                    *multi_or_mono,
                                )
                            })
                    })
                    .collect_vec()
            }
            PathOp::ClearRecursive(_) => {
                let mut results = vec![];
                let mut trees = vec![];
                trees.push(leaf);
                while !trees.is_empty() {
                    let next = trees.pop().unwrap();
                    trees.extend(next.children.values_mut().collect::<Vec<&mut Self>>());

                    // Track that any future children need a Null at the right
                    // time-point when added.
                    next.recursive_clears
                        .entry(msg_id)
                        .or_insert_with(|| time_point.clone());

                    // Track that any future fields need a Null at the right
                    // time-point when added.
                    next.nonrecursive_clears
                        .entry(msg_id)
                        .or_insert_with(|| time_point.clone());

                    // For every existing field append a clear event into the
                    // results
                    results.extend(next.fields.iter().flat_map(|(field_name, fields)| {
                        fields
                            .per_type
                            .iter()
                            .map(|((data_type, multi_or_mono), _)| {
                                (
                                    DataPath::new(next.path.clone(), *field_name),
                                    *data_type,
                                    *multi_or_mono,
                                )
                            })
                    }));
                }
                results
            }
        }
    }

    fn create_subtrees_recursively(
        &mut self,
        full_path: &[ObjPathComp],
        depth: usize,
        msg_id: MsgId,
        time_point: &TimePoint,
    ) -> &mut Self {
        for (timeline, time_value) in time_point.iter() {
            self.prefix_times
                .0
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
                .or_insert_with(|| {
                    ObjectTree::new(full_path[..depth + 1].into(), self.recursive_clears.clone())
                })
                .create_subtrees_recursively(full_path, depth + 1, msg_id, time_point),
        }
    }

    pub fn subtree(&self, path: &ObjPath) -> Option<&Self> {
        fn subtree_recursive<'tree>(
            this: &'tree ObjectTree,
            path: &[ObjPathComp],
        ) -> Option<&'tree ObjectTree> {
            match path {
                [] => Some(this),
                [first, rest @ ..] => subtree_recursive(this.children.get(first)?, rest),
            }
        }

        subtree_recursive(self, &path.to_components())
    }

    pub fn purge_everything_but(&mut self, keep_msg_ids: &ahash::HashSet<MsgId>) {
        let Self {
            path: _,
            children,
            prefix_times,
            nonrecursive_clears,
            recursive_clears,
            fields,
        } = self;

        for map in prefix_times.0.values_mut() {
            crate::profile_scope!("prefix_times");
            map.retain(|_, msg_ids| {
                msg_ids.retain(|msg_id| keep_msg_ids.contains(msg_id));
                !msg_ids.is_empty()
            });
        }
        {
            crate::profile_scope!("nonrecursive_clears");
            nonrecursive_clears.retain(|msg_id, _| keep_msg_ids.contains(msg_id));
        }
        {
            crate::profile_scope!("recursive_clears");
            recursive_clears.retain(|msg_id, _| keep_msg_ids.contains(msg_id));
        }

        {
            crate::profile_scope!("fields");
            for columns in fields.values_mut() {
                columns.purge_everything_but(keep_msg_ids);
            }
        }

        for child in children.values_mut() {
            child.purge_everything_but(keep_msg_ids);
        }
    }

    // Invokes visitor for `self` all children recursively.
    pub fn visit_children_recursively(&self, visitor: &mut impl FnMut(&ObjPath)) {
        visitor(&self.path);
        for child in self.children.values() {
            child.visit_children_recursively(visitor);
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MonoOrMulti {
    Mono,
    Multi,
}

/// Column transform of [`re_log_types::Data`].
#[derive(Default)]
pub struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<Timeline, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    pub per_type: HashMap<(DataType, MonoOrMulti), BTreeSet<MsgId>>,
}

impl DataColumns {
    pub fn add(&mut self, msg_id: MsgId, time_point: &TimePoint, data: &LoggedData) {
        for (timeline, time_value) in time_point.iter() {
            self.times
                .entry(*timeline)
                .or_default()
                .entry(*time_value)
                .or_default()
                .insert(msg_id);
        }

        let mono_or_multi = match data {
            LoggedData::Null(_) | LoggedData::Single(_) => MonoOrMulti::Mono,
            LoggedData::Batch { .. } | LoggedData::BatchSplat(_) => MonoOrMulti::Multi,
        };

        self.per_type
            .entry((data.data_type(), mono_or_multi))
            .or_default()
            .insert(msg_id);
    }

    pub fn summary(&self) -> String {
        let mut summaries = vec![];

        for ((typ, _), set) in &self.per_type {
            let (stem, plur) = match typ {
                DataType::Bool => ("bool", "s"),
                DataType::I32 => ("integer", "s"),
                DataType::F32 | DataType::F64 => ("scalar", "s"),
                DataType::Color => ("color", "s"),
                DataType::String => ("string", "s"),

                DataType::Vec2 => ("2D vector", "s"),
                DataType::BBox2D => ("2D bounding box", "es"),

                DataType::Vec3 => ("3D vector", "s"),
                DataType::Box3 => ("3D box", "es"),
                DataType::Mesh3D => ("mesh", "es"),
                DataType::Arrow3D => ("3D arrow", "s"),

                DataType::Tensor => ("tensor", "s"),

                DataType::ObjPath => ("path", "s"),

                DataType::Transform => ("transform", "s"),
                DataType::ViewCoordinates => ("coordinate system", "s"),
                DataType::AnnotationContext => ("annotation context", "s"),

                DataType::DataVec => ("vector", "s"),
            };

            summaries.push(plurality(set.len(), stem, plur));
        }

        summaries.join(", ")
    }

    pub fn purge_everything_but(&mut self, keep_msg_ids: &ahash::HashSet<MsgId>) {
        let Self { times, per_type } = self;

        for map in times.values_mut() {
            map.retain(|_, msg_ids| {
                msg_ids.retain(|msg_id| keep_msg_ids.contains(msg_id));
                !msg_ids.is_empty()
            });
        }
        for msg_set in per_type.values_mut() {
            msg_set.retain(|msg_id| keep_msg_ids.contains(msg_id));
        }
    }
}

pub(crate) fn plurality(num: usize, singular: &str, plural_suffix: &str) -> String {
    if num == 1 {
        format!("1 {}", singular)
    } else {
        format!("{} {}{}", num, singular, plural_suffix)
    }
}
