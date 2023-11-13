use std::collections::BTreeMap;

use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_arrow_store::StoreEvent;
use re_log_types::{
    ComponentPath, EntityPath, EntityPathHash, EntityPathPart, PathOp, RowId, TimeInt, TimePoint,
    Timeline,
};
use re_types_core::{ComponentName, Loggable};

use crate::TimeHistogramPerTimeline;

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

/// Tree of entity paths, plus components at the leaves.
pub struct EntityTree {
    /// Full path to the root of this tree.
    pub path: EntityPath,

    pub children: BTreeMap<EntityPathPart, EntityTree>,

    /// Recursive time histogram for this [`EntityTree`].
    ///
    /// Keeps track of the _number of components logged_ per time per timeline, recursively across
    /// all of the [`EntityTree`]'s children.
    /// A component logged twice at the same timestamp is counted twice.
    ///
    /// ⚠ Auto-generated instance keys are _not_ accounted for. ⚠
    pub recursive_time_histogram: TimeHistogramPerTimeline,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<RowId, TimePoint>,

    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<RowId, TimePoint>,

    /// Flat time histograms for each component of this [`EntityTree`].
    ///
    /// Keeps track of the _number of times a component is logged_ per time per timeline, only for
    /// this specific [`EntityTree`].
    /// A component logged twice at the same timestamp is counted twice.
    ///
    /// ⚠ Auto-generated instance keys are _not_ accounted for. ⚠
    pub time_histograms_per_component: BTreeMap<ComponentName, TimeHistogramPerTimeline>,
}

/// Maintains an optimized representation of a batch of [`StoreEvent`]s specifically designed to
/// accelerate garbage collection of [`EntityTree`]s.
#[derive(Default)]
pub struct CompactedStoreEvents {
    /// What rows were deleted?
    pub row_ids: HashSet<RowId>,

    /// What time points were deleted for each entity+timeline+component?
    pub timeful: IntMap<EntityPathHash, IntMap<Timeline, IntMap<ComponentName, Vec<TimeInt>>>>,

    /// For each entity+component, how many timeless entries were deleted?
    pub timeless: IntMap<EntityPathHash, IntMap<ComponentName, u64>>,
}

impl CompactedStoreEvents {
    pub fn new(store_events: &[StoreEvent]) -> Self {
        let mut this = CompactedStoreEvents {
            row_ids: store_events.iter().map(|event| event.row_id).collect(),
            timeful: Default::default(),
            timeless: Default::default(),
        };

        for event in store_events {
            if event.is_timeless() {
                let per_component = this.timeless.entry(event.entity_path.hash()).or_default();
                for component_name in event.cells.keys() {
                    *per_component.entry(*component_name).or_default() +=
                        event.delta().unsigned_abs();
                }
            } else {
                for (&timeline, &time) in &event.timepoint {
                    let per_timeline = this.timeful.entry(event.entity_path.hash()).or_default();
                    let per_component = per_timeline.entry(timeline).or_default();
                    for component_name in event.cells.keys() {
                        per_component.entry(*component_name).or_default().push(time);
                    }
                }
            }
        }

        this
    }
}

impl EntityTree {
    pub fn root() -> Self {
        Self::new(EntityPath::root(), Default::default())
    }

    pub fn new(path: EntityPath, recursive_clears: BTreeMap<RowId, TimePoint>) -> Self {
        Self {
            path,
            children: Default::default(),
            recursive_time_histogram: Default::default(),
            nonrecursive_clears: recursive_clears.clone(),
            recursive_clears,
            time_histograms_per_component: Default::default(),
        }
    }

    /// Has no child entities.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn num_children_and_fields(&self) -> usize {
        self.children.len() + self.time_histograms_per_component.len()
    }

    pub fn num_timeless_messages(&self) -> u64 {
        self.recursive_time_histogram.num_timeless_messages()
    }

    pub fn time_histogram_for_component(
        &self,
        timeline: &Timeline,
        component_name: impl Into<ComponentName>,
    ) -> Option<&crate::TimeHistogram> {
        self.time_histograms_per_component
            .get(&component_name.into())
            .and_then(|per_timeline| per_timeline.get(timeline))
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
            .time_histograms_per_component
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
                leaf.time_histograms_per_component
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
                        next.time_histograms_per_component
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
        self.recursive_time_histogram.add(time_point);

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
        deleted: &CompactedStoreEvents,
        deleted_by_us_and_children: &mut ActuallyDeleted,
    ) {
        let Self {
            path,
            children,
            recursive_time_histogram,
            nonrecursive_clears,
            recursive_clears,
            time_histograms_per_component,
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
            re_tracing::profile_scope!("components");

            // The `deleted` stats are per component, so start here:

            for (comp_name, times) in time_histograms_per_component {
                for (timeline, histogram) in times.iter_mut() {
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
                    recursive_time_histogram.num_timeless_messages = recursive_time_histogram
                        .num_timeless_messages
                        .saturating_sub(*num_deleted);
                    deleted_by_children.timeless += num_deleted;
                }
            }
        }

        {
            // Apply what was deleted by children and by our components:
            recursive_time_histogram.num_timeless_messages = recursive_time_histogram
                .num_timeless_messages
                .saturating_sub(deleted_by_us_and_children.timeless);

            for (timeline, histogram) in recursive_time_histogram.iter_mut() {
                if let Some(times) = deleted_by_children.timeful.get(timeline) {
                    for &time in times {
                        histogram.decrement(time.as_i64(), 1);
                    }
                }

                // NOTE: we don't include timeless in the histogram.
            }
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
