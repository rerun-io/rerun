use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use nohash_hasher::IntMap;

use re_log_types::{
    ComponentPath, EntityPath, EntityPathPart, PathOp, RowId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{ComponentName, Loggable};

// ----------------------------------------------------------------------------

/// Book-keeping required after a GC purge to keep track
/// of what was removed from children, so it can also be removed
/// from the parents.
#[derive(Default)]
pub struct ActuallyDeleted {
    pub timeful: IntMap<Timeline, Vec<TimeInt>>,
    pub timeless: u64,
}

impl ActuallyDeleted {
    fn append(&mut self, other: Self) {
        let Self { timeful, timeless } = other;

        for (timeline, mut times) in timeful {
            self.timeful.entry(timeline).or_default().append(&mut times);
        }
        self.timeless += timeless;
    }
}

// ----------------------------------------------------------------------------

/// Number of messages per time
pub type TimeHistogram = re_int_histogram::Int64Histogram;

/// Number of messages per time per timeline.
///
/// Does NOT include timeless.
#[derive(Default)]
pub struct TimeHistogramPerTimeline(BTreeMap<Timeline, TimeHistogram>);

impl TimeHistogramPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&TimeHistogram> {
        self.0.get(timeline)
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.0.contains_key(timeline)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &TimeHistogram)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&Timeline, &mut TimeHistogram)> {
        self.0.iter_mut()
    }

    pub fn purge(&mut self, deleted: &ActuallyDeleted) {
        re_tracing::profile_function!();

        for (timeline, histogram) in &mut self.0 {
            if let Some(times) = deleted.timeful.get(timeline) {
                for &time in times {
                    histogram.decrement(time.as_i64(), 1);
                }
            }

            // NOTE: we don't include timeless in the histogram.
        }
    }
}

// ----------------------------------------------------------------------------

/// Number of messages per time per timeline.
///
/// Does NOT include timeless.
pub struct TimesPerTimeline(BTreeMap<Timeline, BTreeSet<TimeInt>>);

impl TimesPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&BTreeSet<TimeInt>> {
        self.0.get(timeline)
    }

    pub fn get_mut(&mut self, timeline: &Timeline) -> Option<&mut BTreeSet<TimeInt>> {
        self.0.get_mut(timeline)
    }

    pub fn insert(&mut self, timeline: Timeline, time: TimeInt) {
        self.0.entry(timeline).or_default().insert(time);
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

// Always ensure we have a default "log_time" timeline.
impl Default for TimesPerTimeline {
    fn default() -> Self {
        Self(BTreeMap::from([(Timeline::log_time(), Default::default())]))
    }
}

// ----------------------------------------------------------------------------

/// Tree of entity paths, plus components at the leaves.
pub struct EntityTree {
    /// Full path to the root of this tree.
    pub path: EntityPath,

    pub children: BTreeMap<EntityPathPart, EntityTree>,

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: TimeHistogramPerTimeline,

    /// Extra book-keeping used to seed any timelines that include timeless msgs
    num_timeless_messages: usize,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<RowId, TimePoint>,

    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<RowId, TimePoint>,

    /// Data logged at this entity path.
    pub components: BTreeMap<ComponentName, ComponentStats>,
}

impl EntityTree {
    pub fn root() -> Self {
        Self::new(EntityPath::root(), Default::default())
    }

    pub fn new(path: EntityPath, recursive_clears: BTreeMap<RowId, TimePoint>) -> Self {
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

    /// Has no child entities.
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
        component_path: &ComponentPath,
    ) -> Vec<(RowId, TimePoint)> {
        re_tracing::profile_function!();

        let leaf =
            self.create_subtrees_recursively(component_path.entity_path.as_slice(), 0, time_point);

        let mut pending_clears = vec![];

        let fields = leaf
            .components
            .entry(component_path.component_name)
            .or_insert_with(|| {
                // If we needed to create a new leaf to hold this data, we also want to
                // insert all of the historical pending clear operations
                pending_clears = leaf.nonrecursive_clears.clone().into_iter().collect_vec();

                Default::default()
            });

        fields.add(time_point);

        pending_clears
    }

    /// Add a path operation into the entity tree.
    ///
    /// Returns a collection of paths to clear as a result of the operation
    /// Additional pending clear operations will be stored in the tree for future
    /// insertion.
    pub fn add_path_op(
        &mut self,
        row_id: RowId,
        time_point: &TimePoint,
        path_op: &PathOp,
    ) -> Vec<ComponentPath> {
        use re_types_core::{archetypes::Clear, components::ClearIsRecursive, Archetype as _};

        re_tracing::profile_function!();

        let entity_path = path_op.entity_path();

        // Look up the leaf at which we will execute the path operation
        let leaf = self.create_subtrees_recursively(entity_path.as_slice(), 0, time_point);

        fn filter_out_clear_components(comp_name: &ComponentName) -> bool {
            let is_clear_component = [
                Clear::indicator().name(), //
                ClearIsRecursive::name(),  //
            ]
            .contains(comp_name);
            !is_clear_component
        }

        // TODO(jleibs): Refactor this as separate functions
        match path_op {
            PathOp::ClearComponents(entity_path) => {
                // Track that any future fields need a Null at the right
                // time-point when added.
                leaf.nonrecursive_clears
                    .entry(row_id)
                    .or_insert_with(|| time_point.clone());

                // For every existing field return a clear event
                leaf.components
                    .keys()
                    .filter(|comp_name| filter_out_clear_components(comp_name))
                    .map(|component_name| ComponentPath::new(entity_path.clone(), *component_name))
                    .collect_vec()
            }
            PathOp::ClearRecursive(_) => {
                let mut results = vec![];
                let mut trees = vec![];
                trees.push(leaf);
                while let Some(next) = trees.pop() {
                    trees.extend(next.children.values_mut().collect::<Vec<&mut Self>>());

                    // Track that any future children need a Null at the right
                    // time-point when added.
                    next.recursive_clears
                        .entry(row_id)
                        .or_insert_with(|| time_point.clone());

                    // Track that any future fields need a Null at the right
                    // time-point when added.
                    next.nonrecursive_clears
                        .entry(row_id)
                        .or_insert_with(|| time_point.clone());

                    // For every existing field append a clear event into the
                    // results
                    results.extend(
                        next.components
                            .keys()
                            .filter(|comp_name| filter_out_clear_components(comp_name))
                            .map(|component_name| {
                                ComponentPath::new(next.path.clone(), *component_name)
                            }),
                    );
                }
                results
            }
        }
    }

    fn create_subtrees_recursively(
        &mut self,
        full_path: &[EntityPathPart],
        depth: usize,
        time_point: &TimePoint,
    ) -> &mut Self {
        // If the time_point is timeless…
        if time_point.is_timeless() {
            self.num_timeless_messages += 1;
        } else {
            for (timeline, time_value) in time_point.iter() {
                self.prefix_times
                    .0
                    .entry(*timeline)
                    .or_default()
                    .increment(time_value.as_i64(), 1);
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
                    EntityTree::new(full_path[..depth + 1].into(), self.recursive_clears.clone())
                })
                .create_subtrees_recursively(full_path, depth + 1, time_point),
        }
    }

    pub fn subtree(&self, path: &EntityPath) -> Option<&Self> {
        fn subtree_recursive<'tree>(
            this: &'tree EntityTree,
            path: &[EntityPathPart],
        ) -> Option<&'tree EntityTree> {
            match path {
                [] => Some(this),
                [first, rest @ ..] => subtree_recursive(this.children.get(first)?, rest),
            }
        }

        subtree_recursive(self, path.as_slice())
    }

    /// Purge all times before the cutoff, or in the given set
    pub fn purge(
        &mut self,
        deleted: &re_arrow_store::Deleted,
        deleted_by_us_and_children: &mut ActuallyDeleted,
    ) {
        let Self {
            path,
            children,
            prefix_times,
            num_timeless_messages,
            nonrecursive_clears,
            recursive_clears,
            components,
        } = self;

        {
            re_tracing::profile_scope!("nonrecursive_clears");
            nonrecursive_clears.retain(|row_id, _| !deleted.row_ids.contains(row_id));
        }
        {
            re_tracing::profile_scope!("recursive_clears");
            recursive_clears.retain(|row_id, _| !deleted.row_ids.contains(row_id));
        }

        let mut deleted_by_children = ActuallyDeleted::default();

        for child in children.values_mut() {
            child.purge(deleted, &mut deleted_by_children);
        }

        {
            re_tracing::profile_scope!("ComponentStats");

            // The `deleted` stats are per component, so start here:

            for (comp_name, stats) in components {
                let ComponentStats {
                    times,
                    num_timeless_messages,
                } = stats;

                for (timeline, histogram) in &mut times.0 {
                    if let Some(times) = deleted
                        .timeful
                        .get(&path.hash())
                        .and_then(|map| map.get(timeline))
                        .and_then(|map| map.get(comp_name))
                    {
                        for &time in times {
                            histogram.decrement(time.as_i64(), 1);

                            deleted_by_children
                                .timeful
                                .entry(*timeline)
                                .or_default()
                                .push(time);
                        }
                    }

                    // NOTE: we don't include timeless in the histogram.
                }

                if let Some(num_deleted) = deleted
                    .timeless
                    .get(&path.hash())
                    .and_then(|map| map.get(comp_name))
                {
                    *num_timeless_messages =
                        num_timeless_messages.saturating_sub(*num_deleted as _);
                    deleted_by_children.timeless += num_deleted;
                }
            }
        }

        {
            // Apply what was deleted by children and by our components:
            *num_timeless_messages =
                num_timeless_messages.saturating_sub(deleted_by_us_and_children.timeless as _);

            prefix_times.purge(&deleted_by_children);
        }

        deleted_by_us_and_children.append(deleted_by_children);
    }

    // Invokes visitor for `self` all children recursively.
    pub fn visit_children_recursively(&self, visitor: &mut impl FnMut(&EntityPath)) {
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
        // If the `time_point` is timeless…
        if time_point.is_timeless() {
            self.num_timeless_messages += 1;
        } else {
            for (timeline, time_value) in time_point.iter() {
                self.times
                    .0
                    .entry(*timeline)
                    .or_default()
                    .increment(time_value.as_i64(), 1);
            }
        }
    }
}
