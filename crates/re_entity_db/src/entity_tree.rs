use std::collections::BTreeMap;

use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_data_store::{StoreDiff, StoreDiffKind, StoreEvent, StoreSubscriber};
use re_log_types::{
    ComponentPath, EntityPath, EntityPathHash, EntityPathPart, RowId, TimeInt, Timeline,
};
use re_types_core::ComponentName;

// Used all over in docstrings.
#[allow(unused_imports)]
use re_data_store::DataStore;

use crate::TimeHistogramPerTimeline;

// ----------------------------------------------------------------------------

/// A recursive, manually updated [`re_data_store::StoreSubscriber`] that maintains the entity hierarchy.
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
                    let removed_bytes = cell.total_size_bytes();
                    self.data_bytes =
                        self.data_bytes
                            .checked_sub(removed_bytes)
                            .unwrap_or_else(|| {
                                re_log::debug!(
                                    store_id = %event.store_id,
                                    entity_path = %event.diff.entity_path,
                                    current = self.data_bytes,
                                    removed = removed_bytes,
                                    "book keeping underflowed"
                                );
                                u64::MIN
                            });
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
    pub temporal: IntMap<EntityPathHash, IntMap<Timeline, IntMap<ComponentName, Vec<TimeInt>>>>,

    /// For each entity+component, how many timeless entries were deleted?
    pub timeless: IntMap<EntityPathHash, IntMap<ComponentName, u64>>,
}

impl CompactedStoreEvents {
    pub fn new(store_events: &[&StoreEvent]) -> Self {
        let mut this = Self {
            row_ids: store_events.iter().map(|event| event.row_id).collect(),
            temporal: Default::default(),
            timeless: Default::default(),
        };

        for event in store_events {
            if event.is_static() {
                let per_component = this.timeless.entry(event.entity_path.hash()).or_default();
                for component_name in event.cells.keys() {
                    *per_component.entry(*component_name).or_default() +=
                        event.delta().unsigned_abs();
                }
            } else {
                for &(timeline, time) in &event.times {
                    let per_timeline = this.temporal.entry(event.entity_path.hash()).or_default();
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
        Self::new(EntityPath::root())
    }

    pub fn new(path: EntityPath) -> Self {
        Self {
            path,
            children: Default::default(),
            entity: Default::default(),
            subtree: Default::default(),
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
    pub fn num_static_messages_recursive(&self) -> u64 {
        self.subtree.time_histogram.num_static_messages()
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
    /// Only reacts to additions (`event.kind == StoreDiffKind::Addition`).
    pub fn on_store_additions(&mut self, events: &[StoreEvent]) {
        re_tracing::profile_function!();

        for event in events.iter().filter(|e| e.kind == StoreDiffKind::Addition) {
            self.on_store_addition(event);
        }
    }

    fn on_store_addition(&mut self, event: &StoreEvent) {
        re_tracing::profile_function!();

        let entity_path = &event.diff.entity_path;

        // Book-keeping for each level in the hierarchy:
        let mut tree = self;
        tree.subtree.on_event(event);

        for (i, part) in entity_path.iter().enumerate() {
            tree = tree
                .children
                .entry(part.clone())
                .or_insert_with(|| Self::new(entity_path.as_slice()[..=i].into()));
            tree.subtree.on_event(event);
        }

        // Finally book-keeping for the entity where data was actually added:
        tree.on_added_data(&event.diff);
    }

    /// Handles the addition of new data into the tree.
    fn on_added_data(&mut self, store_diff: &StoreDiff) {
        for component_name in store_diff.cells.keys() {
            let component_path =
                ComponentPath::new(store_diff.entity_path.clone(), *component_name);

            let per_component = self
                .entity
                .components
                .entry(component_path.component_name)
                .or_default();
            per_component.add(&store_diff.times, 1);
        }
    }

    /// Updates the [`EntityTree`] by applying a batch of [`StoreEvent`]s.
    ///
    /// Only reacts to deletions (`event.kind == StoreDiffKind::Deletion`).
    pub fn on_store_deletions(&mut self, store_events: &[&StoreEvent]) {
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
            for &event in &subtree_events {
                subtree.on_event(event);
            }
        }

        children.retain(|_, child| {
            child.on_store_deletions(&subtree_events);
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

    // Invokes visitor for `self` and all children recursively.
    pub fn visit_children_recursively(&self, visitor: &mut impl FnMut(&EntityPath, &EntityInfo)) {
        visitor(&self.path, &self.entity);
        for child in self.children.values() {
            child.visit_children_recursively(visitor);
        }
    }
}
