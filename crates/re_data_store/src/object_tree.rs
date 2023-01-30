use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use re_log_types::{
    DataPath, FieldOrComponent, MsgId, ObjPath, ObjPathComp, PathOp, TimeInt, TimePoint, Timeline,
};

// ----------------------------------------------------------------------------

/// Number of messages per time per timeline
#[derive(Default)]
pub struct TimeHistogramPerTimeline(BTreeMap<Timeline, BTreeMap<TimeInt, usize>>);

impl TimeHistogramPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&BTreeMap<TimeInt, usize>> {
        self.0.get(timeline)
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.0.contains_key(timeline)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &BTreeMap<TimeInt, usize>)> {
        self.0.iter()
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (&Timeline, &mut BTreeMap<TimeInt, usize>)> {
        self.0.iter_mut()
    }
}

// ----------------------------------------------------------------------------

/// Number of messages per time per timeline
#[derive(Default)]
pub struct TimesPerTimeline(BTreeMap<Timeline, BTreeSet<TimeInt>>);

impl TimesPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&BTreeSet<TimeInt>> {
        self.0.get(timeline)
    }

    pub fn insert(&mut self, timeline: Timeline, time: TimeInt) {
        self.0.entry(timeline).or_default().insert(time);
    }

    pub fn purge(&mut self, cutoff_times: &std::collections::BTreeMap<Timeline, TimeInt>) {
        for (timeline, time_set) in &mut self.0 {
            if let Some(cutoff_time) = cutoff_times.get(timeline) {
                time_set.retain(|time| cutoff_time <= time);
            }
        }
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.0.contains_key(timeline)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &BTreeSet<TimeInt>)> {
        self.0.iter()
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (&Timeline, &mut BTreeSet<TimeInt>)> {
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
    pub prefix_times: TimeHistogramPerTimeline,

    /// Extra book-keeping used to seed any timelines that include timeless msgs
    num_timeless_messages: usize,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<MsgId, TimePoint>,
    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<MsgId, TimePoint>,

    /// Data logged at this object path.
    pub components: BTreeMap<FieldOrComponent, ComponentStats>,
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
            num_timeless_messages: 0,
            nonrecursive_clears: recursive_clears.clone(),
            recursive_clears,
            components: Default::default(),
        }
    }

    /// Has no child objects.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn num_children_and_fields(&self) -> usize {
        self.children.len() + self.components.len()
    }

    pub fn num_timeless_messages(&self) -> usize {
        self.num_timeless_messages
    }

    /// Returns a collection of pending clear operations
    pub fn add_data_msg(
        &mut self,
        time_point: &TimePoint,
        data_path: &DataPath,
    ) -> Vec<(MsgId, TimePoint)> {
        crate::profile_function!();
        let obj_path = data_path.obj_path.to_components();

        let leaf = self.create_subtrees_recursively(obj_path.as_slice(), 0, time_point);

        let mut pending_clears = vec![];

        let fields = leaf
            .components
            .entry(data_path.field_name)
            .or_insert_with(|| {
                // If we needed to create a new leaf to hold this data, we also want to
                // insert all of the historical pending clear operations
                pending_clears = leaf.nonrecursive_clears.clone().into_iter().collect_vec();

                Default::default()
            });

        fields.add(time_point);

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
    ) -> Vec<DataPath> {
        crate::profile_function!();

        let obj_path = path_op.obj_path().to_components();

        // Look up the leaf at which we will execute the path operation
        let leaf = self.create_subtrees_recursively(obj_path.as_slice(), 0, time_point);

        // TODO(jleibs): Refactor this as separate functions
        match path_op {
            PathOp::ClearFields(obj_path) => {
                // Track that any future fields need a Null at the right
                // time-point when added.
                leaf.nonrecursive_clears
                    .entry(msg_id)
                    .or_insert_with(|| time_point.clone());

                // For every existing field return a clear event
                leaf.components
                    .iter()
                    .map(|(field_name, _fields)| DataPath::new_any(obj_path.clone(), *field_name))
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
                    results.extend(next.components.iter().map(|(field_name, _fields)| {
                        DataPath::new_any(next.path.clone(), *field_name)
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
        time_point: &TimePoint,
    ) -> &mut Self {
        // If the time_point is timeless...
        if time_point.is_timeless() {
            self.num_timeless_messages += 1;
        } else {
            for (timeline, time_value) in time_point.iter() {
                *self
                    .prefix_times
                    .0
                    .entry(*timeline)
                    .or_default()
                    .entry(*time_value)
                    .or_default() += 1;
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
                .create_subtrees_recursively(full_path, depth + 1, time_point),
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

    /// Purge all times before the cutoff, or in the given set
    pub fn purge(
        &mut self,
        cutoff_times: &BTreeMap<Timeline, TimeInt>,
        drop_msg_ids: &ahash::HashSet<MsgId>,
    ) {
        let Self {
            path: _,
            children,
            prefix_times,
            num_timeless_messages: _,
            nonrecursive_clears,
            recursive_clears,
            components: fields,
        } = self;

        for (timeline, map) in &mut prefix_times.0 {
            crate::profile_scope!("prefix_times");
            if let Some(cutoff_time) = cutoff_times.get(timeline) {
                map.retain(|time_int, _count| cutoff_time.is_timeless() || cutoff_time <= time_int);
            }
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
                columns.purge(cutoff_times);
            }
        }

        for child in children.values_mut() {
            child.purge(cutoff_times, drop_msg_ids);
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

#[derive(Default)]
pub struct ComponentStats {
    /// When do we have data? Ignored timeless.
    pub times: TimeHistogramPerTimeline,

    /// Extra book-keeping used to seed any timelines that include timeless msgs
    num_timeless_messages: usize,
}

impl ComponentStats {
    pub fn num_timeless_messages(&self) -> usize {
        self.num_timeless_messages
    }

    pub fn add(&mut self, time_point: &TimePoint) {
        // If the `time_point` is timeless...
        if time_point.is_timeless() {
            self.num_timeless_messages += 1;
        } else {
            for (timeline, time_value) in time_point.iter() {
                *self
                    .times
                    .0
                    .entry(*timeline)
                    .or_default()
                    .entry(*time_value)
                    .or_default() += 1;
            }
        }
    }

    pub fn purge(&mut self, cutoff_times: &BTreeMap<Timeline, TimeInt>) {
        let Self {
            times,
            num_timeless_messages: _,
        } = self;

        for (timeline, time_counts) in &mut times.0 {
            if let Some(cutoff_time) = cutoff_times.get(timeline) {
                time_counts.retain(|time_int, _count| cutoff_time <= time_int);
            }
        }
    }
}
