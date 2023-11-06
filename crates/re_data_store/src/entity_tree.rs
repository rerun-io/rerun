use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use nohash_hasher::{IntMap, IntSet};

use re_arrow_store::{StoreDiff, StoreEvent, StoreView};
use re_log_types::{
    ComponentPath, DataRow, EntityPath, EntityPathPart, PathOp, RowId, TimeInt, TimePoint, Timeline,
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

    /// When do we or a child have data?
    ///
    /// Data logged at this exact path or any child path.
    pub prefix_times: TimeHistogramPerTimeline,

    /// Book-keeping around whether we should clear fields when data is added
    pub nonrecursive_clears: BTreeMap<RowId, TimePoint>,

    /// Book-keeping around whether we should clear recursively when data is added
    pub recursive_clears: BTreeMap<RowId, TimePoint>,

    /// Data logged at this entity path.
    pub components: BTreeMap<ComponentName, TimeHistogramPerTimeline>,
}

// TODO: poor name too
#[derive(Debug, Clone, Default)]
pub struct EntityTreeEvent {
    // TODO: explain the split between the two then?
    pub timepoints_to_clear: BTreeMap<RowId, TimePoint>,
    pub paths_to_clear: BTreeMap<RowId, BTreeSet<ComponentPath>>,
}

impl EntityTreeEvent {
    pub fn is_empty(&self) -> bool {
        let Self {
            timepoints_to_clear: timepoints,
            paths_to_clear: paths,
        } = self;
        timepoints.is_empty() && paths.is_empty()
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
            prefix_times: Default::default(),
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

    // TODO: just remove this though
    pub fn num_timeless_messages(&self) -> usize {
        // TODO: why usize though
        self.prefix_times.num_timeless_messages as _
    }

    pub fn on_additions(&mut self, events: &[StoreEvent]) -> EntityTreeEvent {
        use re_types_core::components::ClearIsRecursive;

        let mut changelog = EntityTreeEvent::default();

        // additions
        for event in events.iter().filter(|e| e.diff.delta > 0) {
            let diff = &event.diff;

            // grow tree
            {
                let full_clears = self.on_added_data(diff);
                for (row_id, timepoint, component_path) in full_clears {
                    for (timeline, time) in timepoint {
                        changelog
                            .timepoints_to_clear
                            .entry(row_id)
                            .or_default()
                            .insert(timeline, time);
                    }
                    changelog
                        .paths_to_clear
                        .entry(row_id)
                        .or_default()
                        .insert(component_path.clone());
                }
            }

            // register pending clear
            if diff.cell.component_name() == ClearIsRecursive::name() {
                let is_recursive = diff
                    .cell
                    .try_to_native_mono::<ClearIsRecursive>()
                    .unwrap()
                    .map_or(false, |settings| settings.0);

                let cleared_paths = self.on_added_clear(diff, is_recursive);

                if let Some((timeline, time)) = diff.timestamp {
                    changelog
                        .timepoints_to_clear
                        .entry(diff.row_id)
                        .or_default()
                        .insert(timeline, time);
                }
                changelog
                    .paths_to_clear
                    .entry(diff.row_id)
                    .or_default()
                    .extend(cleared_paths);
            }
        }

        // deletions
        for event in events.iter().filter(|e| e.diff.delta > 0) {}

        changelog
    }

    // TODO: this guy clears all added components affected by the previously added clears (and so
    // using the old rowid/timepoints).
    //
    /// Returns a collection of pending clear operations
    pub fn on_added_data(&mut self, diff: &StoreDiff) -> Vec<(RowId, TimePoint, ComponentPath)> {
        // pub fn on_added_data(
        //     &mut self,
        //     time_point: &TimePoint,
        //     component_path: &ComponentPath,
        // ) -> Vec<(RowId, TimePoint)> {
        //     re_tracing::profile_function!();
        //
        //     let timepoint = time_point.clone();

        let StoreDiff {
            timestamp,
            entity_path,
            component_name,
            ..
        } = diff;

        let component_path = ComponentPath::new(entity_path.clone(), *component_name);
        let timepoint =
            timestamp.map_or(Default::default(), |timestamp| TimePoint::from([timestamp]));

        // TODO: reminder this not only creates a subtree but also update the prefix_times thingy
        let leaf =
            self.create_subtrees_recursively(component_path.entity_path.as_slice(), 0, &timepoint);

        let mut pending_clears = vec![];

        let stats = leaf
            .components
            .entry(component_path.component_name)
            .or_insert_with(|| {
                // If we needed to create a new leaf to hold this data, we also want to
                // insert all of the historical pending clear operations
                pending_clears = leaf.nonrecursive_clears.clone().into_iter().collect_vec();

                Default::default()
            });

        stats.add(&timepoint);

        pending_clears
            .into_iter()
            .map(|(row_id, timepoint)| (row_id, timepoint, component_path.clone()))
            .collect()
    }

    // TODO: this guy clears all existing components affected by the newly added clear (and so
    // using its rowid/timepoint).
    //
    /// Add a path operation into the entity tree.
    ///
    /// Returns a collection of paths to clear as a result of the operation
    /// Additional pending clear operations will be stored in the tree for future
    /// insertion.
    pub fn on_added_clear(
        &mut self,
        diff: &StoreDiff,
        is_recursive: bool,
    ) -> BTreeSet<ComponentPath> {
        use re_types_core::{archetypes::Clear, components::ClearIsRecursive, Archetype as _};

        re_tracing::profile_function!();

        let StoreDiff {
            row_id,
            timestamp,
            entity_path,
            ..
        } = diff;

        let Some(timestamp) = timestamp else {
            return BTreeSet::new();
        };

        // Look up the leaf at which we will execute the path operation
        let leaf = self.create_subtrees_recursively(
            entity_path.as_slice(),
            0,
            &TimePoint::from([*timestamp]),
        );

        fn filter_out_clear_components(comp_name: &ComponentName) -> bool {
            let is_clear_component = [
                Clear::indicator().name(), //
                ClearIsRecursive::name(),  //
            ]
            .contains(comp_name);
            !is_clear_component
        }

        fn clear_tree(
            tree: &mut EntityTree,
            is_recursive: bool,
            row_id: RowId,
            (timeline, time): (Timeline, TimeInt),
        ) -> impl IntoIterator<Item = ComponentPath> + '_ {
            if is_recursive {
                // Track that any future children need a Null at the right timepoint when added.
                tree.recursive_clears
                    .entry(row_id)
                    .or_default()
                    .insert(timeline, time);
            }

            // Track that any future fields need a Null at the right timepoint when added.
            tree.nonrecursive_clears
                .entry(row_id)
                .or_default()
                .insert(timeline, time);

            // For every existing field return a clear event
            tree.components
                .keys()
                .filter(|comp_name| filter_out_clear_components(comp_name))
                .map(|component_name| ComponentPath::new(tree.path.clone(), *component_name))
        }

        let mut cleared_paths = BTreeSet::new();

        if is_recursive {
            let mut stack = vec![];
            stack.push(leaf);
            while let Some(next) = stack.pop() {
                cleared_paths.extend(clear_tree(next, is_recursive, *row_id, *timestamp));
                stack.extend(next.children.values_mut().collect::<Vec<&mut Self>>());
            }
        } else {
            cleared_paths.extend(clear_tree(leaf, is_recursive, *row_id, *timestamp));
        }

        cleared_paths
    }

    fn create_subtrees_recursively(
        &mut self,
        full_path: &[EntityPathPart],
        depth: usize,
        timepoint: &TimePoint,
    ) -> &mut Self {
        self.prefix_times.add(timepoint);
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
                .create_subtrees_recursively(full_path, depth + 1, timepoint),
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
    // TODO: not correct until we count
    pub fn purge2(&mut self, store_events: &[StoreEvent]) {
        let Self {
            path,
            children,
            prefix_times,
            nonrecursive_clears,
            recursive_clears,
            components,
        } = self;

        for event in store_events.iter().filter(|e| e.diff.delta < 0) {
            let StoreDiff {
                row_id,
                timestamp,
                entity_path,
                component_name,
                cell,
                delta,
            } = &event.diff;

            nonrecursive_clears.remove(row_id);
            recursive_clears.remove(row_id);
        }

        prefix_times.on_events(store_events);

        for child in children.values_mut() {
            child.purge2(store_events);
        }
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
            nonrecursive_clears,
            recursive_clears,
            components,
        } = self;

        // TODO: ask the store if the rowid is alive?
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
                let TimeHistogramPerTimeline {
                    times,
                    num_timeless_messages,
                } = stats;

                for (timeline, histogram) in times {
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

        // {
        //     // Apply what was deleted by children and by our components:
        //     *num_timeless_messages =
        //         num_timeless_messages.saturating_sub(deleted_by_us_and_children.timeless as _);
        //
        //     prefix_times.purge(&deleted_by_children);
        // }

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
