use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use re_log_types::{
    DataPath, DataType, FieldOrComponent, MsgId, ObjPath, ObjPathComp, PathOp, TimeInt, TimePoint,
    Timeline,
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

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (&Timeline, &mut BTreeMap<TimeInt, BTreeSet<MsgId>>)> {
        self.0.iter_mut()
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

    /// Extra book-keeping used to seed any timelines that include timeless msgs
    pub timeless_msgs: BTreeSet<MsgId>,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<MsgId, TimePoint>,
    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<MsgId, TimePoint>,

    /// Data logged at this object path.
    pub fields: BTreeMap<FieldOrComponent, DataColumns>,
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
            timeless_msgs: Default::default(),
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
    /// As of the arrow-migration, the data argument is now optional. The data
    /// stored in fields is redundant information used for visualizing the
    /// timeline. This concept will be removed from the object-tree all together
    /// once timeline context is populated directly from the arrow store. Until
    /// this happens we only get top-level messages in the timeline, not
    /// individual fields.
    ///
    /// Returns a collection of pending clear operations
    pub fn add_data_msg(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
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

        fields.add(msg_id, time_point);

        pending_clears
    }

    /// Add a path operation into the the object tree
    ///
    /// Returns a collection of data paths to clear as a result of the operation
    /// Additional pending clear operations will be stored in the tree for future
    /// insertion.
    pub fn add_path_op(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        path_op: &PathOp,
    ) -> Vec<(DataPath, DataType)> {
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
                    .map(|(field_name, _fields)| {
                        (
                            DataPath::new_any(obj_path.clone(), *field_name),
                            DataType::Bool, // Doesn't matter what we use here. Arrow clears by field_name.
                        )
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
                    results.extend(next.fields.iter().map(|(field_name, _fields)| {
                        (
                            DataPath::new_any(next.path.clone(), *field_name),
                            DataType::Bool, // Doesn't matter what we use here. Arrow clears by field_name.
                        )
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
        // If the time_point is timeless...
        if time_point.is_timeless() {
            // Save it so that we can duplicate it into future timelines
            self.timeless_msgs.insert(msg_id);
        } else {
            for (timeline, time_value) in time_point.iter() {
                self.prefix_times
                    .0
                    .entry(*timeline)
                    .or_default()
                    .entry(*time_value)
                    .or_default()
                    .insert(msg_id);
            }
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

    pub fn purge(&mut self, drop_msg_ids: &ahash::HashSet<MsgId>) {
        let Self {
            path: _,
            children,
            prefix_times,
            timeless_msgs: _,
            nonrecursive_clears,
            recursive_clears,
            fields,
        } = self;

        for map in prefix_times.0.values_mut() {
            crate::profile_scope!("prefix_times");
            map.retain(|_, msg_ids| {
                msg_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
                !msg_ids.is_empty()
            });
        }
        {
            crate::profile_scope!("nonrecursive_clears");
            nonrecursive_clears.retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
        }
        {
            crate::profile_scope!("recursive_clears");
            recursive_clears.retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
        }

        {
            crate::profile_scope!("fields");
            for columns in fields.values_mut() {
                columns.purge(drop_msg_ids);
            }
        }

        for child in children.values_mut() {
            child.purge(drop_msg_ids);
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

/// Column transform of [`re_log_types::Data`].
#[derive(Default)]
pub struct DataColumns {
    /// When do we have data?
    pub times: BTreeMap<Timeline, BTreeMap<TimeInt, BTreeSet<MsgId>>>,
    /// Extra book-keeping used to seed any timelines that include timeless msgs
    pub timeless_msgs: BTreeSet<MsgId>,
}

impl DataColumns {
    pub fn add(&mut self, msg_id: MsgId, time_point: &TimePoint) {
        // If the `time_point` is timeless...
        if time_point.is_timeless() {
            // Save it so that we can duplicate it into future timelines
            self.timeless_msgs.insert(msg_id);

            // Add it to any existing timelines
            for timeline in &mut self.times.values_mut() {
                timeline
                    .entry(TimeInt::BEGINNING)
                    .or_default()
                    .insert(msg_id);
            }
        } else {
            for (timeline, time_value) in time_point.iter() {
                self.times
                    .entry(*timeline)
                    .or_insert_with(|| {
                        if self.timeless_msgs.is_empty() {
                            Default::default()
                        } else {
                            [(TimeInt::BEGINNING, self.timeless_msgs.clone())].into()
                        }
                    })
                    .entry(*time_value)
                    .or_default()
                    .insert(msg_id);
            }
        }
    }

    pub fn populate_timeless(&mut self, time_point: &TimePoint) {
        // For any timeline in `time_point` populate an initial entry from the current
        // `timeless_msgs` seed.
        for (timeline, _) in time_point.iter() {
            self.times.entry(*timeline).or_insert_with(|| {
                if self.timeless_msgs.is_empty() {
                    Default::default()
                } else {
                    [(TimeInt::BEGINNING, self.timeless_msgs.clone())].into()
                }
            });
        }
    }

    pub fn purge(&mut self, drop_msg_ids: &ahash::HashSet<MsgId>) {
        let Self {
            times,
            timeless_msgs: _,
        } = self;

        for map in times.values_mut() {
            map.retain(|_, msg_ids| {
                msg_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
                !msg_ids.is_empty()
            });
        }
    }
}
