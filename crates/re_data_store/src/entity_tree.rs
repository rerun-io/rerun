use std::collections::{BTreeMap, BTreeSet};

use ahash::HashSet;
use itertools::Itertools;

use nohash_hasher::IntMap;
use re_arrow_store::{StoreDiff, StoreDiffKind, StoreEvent, StoreSubscriber};
use re_log_types::{
    ComponentPath, EntityPath, EntityPathHash, EntityPathPart, RowId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{ComponentName, Loggable};

// Used all over in docstrings.
#[allow(unused_imports)]
use re_arrow_store::DataStore;

use crate::TimeHistogramPerTimeline;

// ----------------------------------------------------------------------------

/// A recursive, manually updated [`re_arrow_store::StoreSubscriber`] that maintains the entity hierarchy.
///
/// The tree contains a list of subtrees, and so on recursively.
pub struct EntityTree {
    /// Full path prefix to the root of this (sub)tree.
    pub path: EntityPath,

    /// Direct descendants of this (sub)tree.
    pub children: BTreeMap<EntityPathPart, EntityTree>,

    /// Information about this specific entity (excluding children).
    pub entity: EntityInfo,

    /// Info about this subtree, including all children, recursively.
    pub subtree: SubtreeInfo,
}

// NOTE: This is only to let people know that this is in fact a [`StoreSubscriber`], so they A) don't try
// to implement it on their own and B) don't try to register it.
impl StoreSubscriber for EntityTree {
    fn name(&self) -> String {
        "rerun.store_subscribers.EntityTree".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    #[allow(clippy::unimplemented)]
    fn on_events(&mut self, _events: &[StoreEvent]) {
        unimplemented!(
            r"EntityTree view is maintained manually, see `EntityTree::on_store_{{additions|deletions}}`"
        );
    }
}

/// Information about this specific entity (excluding children).
#[derive(Default)]
pub struct EntityInfo {
    /// Book-keeping around whether we should clear fields when data is added.
    clears: BTreeMap<RowId, TimePoint>,

    /// Flat time histograms for each component of this [`EntityTree`].
    ///
    /// Keeps track of the _number of times a component is logged_ per time per timeline, only for
    /// this specific [`EntityTree`].
    /// A component logged twice at the same timestamp is counted twice.
    ///
    /// ⚠ Auto-generated instance keys are _not_ accounted for. ⚠
    pub components: BTreeMap<ComponentName, TimeHistogramPerTimeline>,
}

/// Info about stuff at a given [`EntityPath`], including all of its children, recursively.
#[derive(Default)]
pub struct SubtreeInfo {
    /// Book-keeping around whether we should clear recursively when data is added.
    clears: BTreeMap<RowId, TimePoint>,

    /// Recursive time histogram for this [`EntityTree`].
    ///
    /// Keeps track of the _number of components logged_ per time per timeline, recursively across
    /// all of the [`EntityTree`]'s children.
    /// A component logged twice at the same timestamp is counted twice.
    ///
    /// ⚠ Auto-generated instance keys are _not_ accounted for. ⚠
    pub time_histogram: TimeHistogramPerTimeline,

    /// Number of bytes used by all arrow data
    data_bytes: u64,
}

impl SubtreeInfo {
    /// Assumes the event has been filtered to be part of this subtree.
    fn on_event(&mut self, event: &StoreEvent) {
        use re_types_core::SizeBytes as _;

        match event.kind {
            StoreDiffKind::Addition => {
                self.time_histogram
                    .add(&event.times, event.num_components() as _);

                for cell in event.cells.values() {
                    self.data_bytes += cell.total_size_bytes();
                }
            }
            StoreDiffKind::Deletion => {
                self.time_histogram
                    .remove(&event.timepoint(), event.num_components() as _);

                for cell in event.cells.values() {
                    if let Some(bytes_left) = self.data_bytes.checked_sub(cell.total_size_bytes()) {
                        self.data_bytes = bytes_left;
                    } else if cfg!(debug_assertions) {
                        re_log::warn_once!(
                            "Error in book-keeping: we've removed more bytes then we've added"
                        );
                    }
                }
            }
        }
    }

    /// Number of bytes used by all arrow data in this tree (including their schemas, but otherwise ignoring book-keeping overhead).
    #[inline]
    pub fn data_bytes(&self) -> u64 {
        self.data_bytes
    }
}

/// Maintains an optimized representation of a batch of [`StoreEvent`]s specifically designed to
/// accelerate garbage collection of [`EntityTree`]s.
///
/// See [`EntityTree::on_store_deletions`].
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
    pub fn new(store_events: &[&StoreEvent]) -> Self {
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
                for &(timeline, time) in &event.times {
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

/// Cascaded clears that need be to applied to the [`DataStore`] as a result of modifying the [`EntityTree`].
///
/// When an [`EntityTree`] gets updated with new data, two cascading effects might happen:
///
/// 1. If the data contains a `Clear` component, then inserting it will trigger an immediate clear
///    at this specific timepoint, that can affect an arbitrary number of components and, if the `Clear`
///    is recursive, even an arbitrary number of entity paths.
///    That `Clear` then lives on and might affect data added later on, which leads us to
///    side-effect #2 described below.
///
/// 2. If data is inserted at an entity path that is under the influence of a previously logged
///    `Clear` component, then the insertion will trigger a pending clear for all components at
///    that path.
///
/// `Clear` components themselves are not affected by clears.
#[derive(Debug, Clone, Default)]
pub struct ClearCascade {
    /// [`ComponentPath`]s that should be cleared as a result of the cascade.
    ///
    /// Keep in mind: these are the [`RowId`]s of the `Clear` components that triggered the
    /// cascades, they are therefore not unique and, by definition, illegal!
    pub to_be_cleared: BTreeMap<RowId, BTreeMap<EntityPath, (TimePoint, BTreeSet<ComponentPath>)>>,
}

impl ClearCascade {
    pub fn is_empty(&self) -> bool {
        let Self { to_be_cleared } = self;
        to_be_cleared.is_empty()
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
            entity: EntityInfo {
                clears: recursive_clears.clone(),
                ..Default::default()
            },
            subtree: SubtreeInfo {
                clears: recursive_clears,
                ..Default::default()
            },
        }
    }

    /// Has no child entities.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn num_children_and_fields(&self) -> usize {
        self.children.len() + self.entity.components.len()
    }

    /// Number of timeless messages in this tree, or any child, recursively.
    pub fn num_timeless_messages_recursive(&self) -> u64 {
        self.subtree.time_histogram.num_timeless_messages()
    }

    pub fn time_histogram_for_component(
        &self,
        timeline: &Timeline,
        component_name: impl Into<ComponentName>,
    ) -> Option<&crate::TimeHistogram> {
        self.entity
            .components
            .get(&component_name.into())
            .and_then(|per_timeline| per_timeline.get(timeline))
    }

    /// Updates the [`EntityTree`] by applying a batch of [`StoreEvent`]s.
    ///
    /// Returns an [`ClearCascade`] that describes the cascading side-effects to be applied to the
    /// [`DataStore`] as a result, if any.
    /// See [`ClearCascade`]'s documentation for more information.
    ///
    /// Only reacts to additions (`event.kind == StoreDiffKind::Addition`).
    pub fn on_store_additions(&mut self, events: &[StoreEvent]) -> ClearCascade {
        re_tracing::profile_function!();

        let mut clear_cascade = ClearCascade::default();
        for event in events.iter().filter(|e| e.kind == StoreDiffKind::Addition) {
            self.on_store_addition(event, &mut clear_cascade);
        }
        clear_cascade
    }

    fn on_store_addition(&mut self, event: &StoreEvent, clear_cascade: &mut ClearCascade) {
        re_tracing::profile_function!();

        let entity_path = &event.diff.entity_path;

        // Book-keeping for each level in the hierarchy:
        let mut tree = self;
        tree.subtree.on_event(event);

        for (i, part) in entity_path.iter().enumerate() {
            tree = tree.children.entry(part.clone()).or_insert_with(|| {
                EntityTree::new(
                    entity_path.as_slice()[..=i].into(),
                    tree.subtree.clears.clone(),
                )
            });
            tree.subtree.on_event(event);
        }

        // Finally book-keeping for the entity where data was actually added:
        tree.on_added_data(clear_cascade, &event.diff);
    }

    /// Handles the addition of new data into the tree.
    ///
    /// Updates the given [`ClearCascade`] with immediate and pending clears as a
    /// result of the operation.
    fn on_added_data(&mut self, clear_cascade: &mut ClearCascade, store_diff: &StoreDiff) {
        for (component_name, cell) in &store_diff.cells {
            let component_path =
                ComponentPath::new(store_diff.entity_path.clone(), *component_name);

            let mut pending_clears = vec![];

            let per_component = self
                .entity
                .components
                .entry(component_path.component_name)
                .or_insert_with(|| {
                    // If we needed to create a new leaf to hold this data, we also want to
                    // insert all of the historical pending clear operations.
                    pending_clears = self.entity.clears.clone().into_iter().collect_vec();
                    Default::default()
                });
            per_component.add(&store_diff.times, 1);

            // Is the newly added component under the influence of previously logged `Clear`
            // component?
            //
            // If so, this is one of two cascading side-effects that happen when updating the entity
            // tree: a pending clear.
            //
            // We need to inform the [`DataStore`] that it should insert a cleared batch for the
            // current component, _using the Timepoint and RowId of the previously logged clear_.
            //
            // ## RowId duplication
            //
            // We want to insert new data (empty cells) using an old RowId (specifically, the RowId
            // of the original insertion that was used to register the pending clear in the first
            // place).
            // By definition, this is illegal: RowIds are unique.
            //
            // On the other hand, the GC process is driven by RowId order, which means we must make
            // sure that the empty cell we're inserting uses a RowId with a similar timestamp as the
            // one used in the original `Clear` component cell, so they roughly get GC'd at the same time.
            //
            // This is fine, the insertion retry mechanism will make sure we get a unique RowId
            // that is still close to this one.

            for (pending_row_id, pending_timepoint) in pending_clears {
                let per_entity = clear_cascade
                    .to_be_cleared
                    .entry(pending_row_id)
                    .or_default();
                let (timepoint, component_paths) = per_entity
                    .entry(store_diff.entity_path.clone())
                    .or_default();
                *timepoint = pending_timepoint.union_max(timepoint);
                component_paths.insert(component_path.clone());
            }

            use re_types_core::components::ClearIsRecursive;
            if cell.component_name() == ClearIsRecursive::name() {
                let is_recursive = cell
                    .try_to_native_mono::<ClearIsRecursive>()
                    .unwrap()
                    .map_or(false, |settings| settings.0);

                self.on_added_clear(clear_cascade, store_diff, is_recursive);
            }
        }
    }

    /// Handles the addition of new `Clear` component into the tree.
    ///
    /// Updates the given [`ClearCascade`] as a result of the operation.
    ///
    /// Additional pending clear operations will be stored in the tree for future
    /// insertion.
    fn on_added_clear(
        &mut self,
        clear_cascade: &mut ClearCascade,
        store_diff: &StoreDiff,
        is_recursive: bool,
    ) {
        use re_types_core::{archetypes::Clear, components::ClearIsRecursive, Archetype as _};

        re_tracing::profile_function!();

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
            timepoint: TimePoint,
        ) -> impl IntoIterator<Item = ComponentPath> + '_ {
            if is_recursive {
                // Track that any future children need a Null at the right timepoint when added.
                let cur_timepoint = tree.subtree.clears.entry(row_id).or_default();
                *cur_timepoint = timepoint.clone().union_max(cur_timepoint);
            }

            // Track that any future fields need a Null at the right timepoint when added.
            let cur_timepoint = tree.entity.clears.entry(row_id).or_default();
            *cur_timepoint = timepoint.union_max(cur_timepoint);

            // For every existing field return a clear event.
            tree.entity
                .components
                .keys()
                // Don't clear `Clear` components, or we'd end up with recursive cascades!
                .filter(|comp_name| filter_out_clear_components(comp_name))
                .map(|component_name| ComponentPath::new(tree.path.clone(), *component_name))
        }

        let mut cleared_paths = BTreeSet::new();

        if is_recursive {
            let mut stack = vec![];
            stack.push(self);
            while let Some(next) = stack.pop() {
                cleared_paths.extend(clear_tree(
                    next,
                    is_recursive,
                    store_diff.row_id,
                    store_diff.timepoint(),
                ));
                stack.extend(next.children.values_mut().collect::<Vec<&mut Self>>());
            }
        } else {
            cleared_paths.extend(clear_tree(
                self,
                is_recursive,
                store_diff.row_id,
                store_diff.timepoint(),
            ));
        }

        // Are there previous logged components under the influence of the newly logged `Clear`
        // component?
        //
        // If so, this is one of two cascading side-effects that happen when updating the entity
        // tree: an immediate clear.
        //
        // We need to inform the [`DataStore`] that it should insert a cleared batch for each of
        // these components, _using the Timepoint and RowId of the newly logged clear_.
        //
        // ## RowId duplication
        //
        // We want to insert new data (empty cells) using a single RowId (specifically, the RowId
        // that was used to log this new `Clear` component.
        // By definition, this is illegal: RowIds are unique.
        //
        // On the other hand, the GC process is driven by RowId order, which means we must make
        // sure that the empty cell we're inserting uses a RowId with a similar timestamp as the
        // one used by the `Clear` component cell, so they roughly get GC'd at the same time.
        //
        // This is fine, the insertion retry mechanism will make sure we get a unique RowId
        // that is still close to this one.

        for component_path in cleared_paths {
            let per_entity = clear_cascade
                .to_be_cleared
                .entry(store_diff.row_id)
                .or_default();
            let (timepoint, component_paths) = per_entity
                .entry(component_path.entity_path().clone())
                .or_default();

            *timepoint = store_diff.timepoint().union_max(timepoint);
            component_paths.insert(component_path.clone());
        }
    }

    /// Updates the [`EntityTree`] by applying a batch of [`StoreEvent`]s.
    ///
    /// Returns an [`ClearCascade`] that describes a list of deletions that should be applied
    /// to the store as a result.
    ///
    /// Only reacts to additions (`event.kind == StoreDiffKind::Addition`).
    pub fn on_store_deletions(
        &mut self,
        store_events: &[&StoreEvent],
        compacted: &CompactedStoreEvents,
    ) {
        re_tracing::profile_function!();

        let Self {
            path,
            children,
            entity,
            subtree,
        } = self;

        // Only keep events relevant to this branch of the tree.
        let subtree_events = store_events
            .iter()
            .filter(|e| e.entity_path.starts_with(path))
            .copied() // NOTE: not actually copying, just removing the superfluous ref layer
            .collect_vec();

        {
            re_tracing::profile_scope!("entity");

            {
                re_tracing::profile_scope!("clears");
                entity
                    .clears
                    .retain(|row_id, _| !compacted.row_ids.contains(row_id));
            }

            re_tracing::profile_scope!("components");
            for event in subtree_events.iter().filter(|e| &e.entity_path == path) {
                for component_name in event.cells.keys() {
                    if let Some(histo) = entity.components.get_mut(component_name) {
                        histo.remove(&event.timepoint(), 1);
                        if histo.is_empty() {
                            entity.components.remove(component_name);
                        }
                    }
                }
            }
        }

        {
            re_tracing::profile_scope!("subtree");
            {
                re_tracing::profile_scope!("clears");
                subtree
                    .clears
                    .retain(|row_id, _| !compacted.row_ids.contains(row_id));
            }
            re_tracing::profile_scope!("on_event");
            for &event in &subtree_events {
                subtree.on_event(event);
            }
        }

        children.retain(|_, child| {
            child.on_store_deletions(&subtree_events, compacted);
            child.num_children_and_fields() > 0
        });
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

    // Invokes visitor for `self` all children recursively.
    pub fn visit_children_recursively(&self, visitor: &mut impl FnMut(&EntityPath)) {
        visitor(&self.path);
        for child in self.children.values() {
            child.visit_children_recursively(visitor);
        }
    }
}
