use std::collections::BTreeMap;

use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_chunk::RowId;
use re_chunk_store::{ChunkStore, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPath, EntityPathHash, EntityPathPart, TimeInt, Timeline};
use re_types_core::ComponentName;

use crate::TimeHistogramPerTimeline;

// ----------------------------------------------------------------------------

/// A recursive, manually updated [`ChunkStoreSubscriber`] that maintains the entity hierarchy.
///
/// The tree contains a list of subtrees, and so on recursively.
pub struct EntityTree {
    /// Full path prefix to the root of this (sub)tree.
    pub path: EntityPath,

    /// Direct descendants of this (sub)tree.
    pub children: BTreeMap<EntityPathPart, EntityTree>,

    /// Info about this subtree, including all children, recursively.
    pub subtree: SubtreeInfo,
}

// NOTE: This is only to let people know that this is in fact a [`ChunkStoreSubscriber`], so they A) don't try
// to implement it on their own and B) don't try to register it.
impl ChunkStoreSubscriber for EntityTree {
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
    fn on_events(&mut self, _events: &[ChunkStoreEvent]) {
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
    fn on_event(&mut self, event: &ChunkStoreEvent) {
        use re_types_core::SizeBytes as _;

        match event.kind {
            ChunkStoreDiffKind::Addition => {
                let times = event
                    .chunk
                    .timelines()
                    .iter()
                    .map(|(&timeline, time_chunk)| (timeline, time_chunk.times_raw()))
                    .collect_vec();
                self.time_histogram.add(&times, event.num_components() as _);

                self.data_bytes += event.chunk.total_size_bytes();
            }
            ChunkStoreDiffKind::Deletion => {
                let times = event
                    .chunk
                    .timelines()
                    .iter()
                    .map(|(&timeline, time_chunk)| (timeline, time_chunk.times_raw()))
                    .collect_vec();
                self.time_histogram
                    .remove(&times, event.num_components() as _);

                let removed_bytes = event.chunk.total_size_bytes();
                self.data_bytes
                    .checked_sub(removed_bytes)
                    .unwrap_or_else(|| {
                        re_log::debug!(
                            store_id = %event.store_id,
                            entity_path = %event.chunk.entity_path(),
                            current = self.data_bytes,
                            removed = removed_bytes,
                            "book keeping underflowed"
                        );
                        u64::MIN
                    });
            }
        }
    }

    /// Number of bytes used by all arrow data in this tree (including their schemas, but otherwise ignoring book-keeping overhead).
    #[inline]
    pub fn data_bytes(&self) -> u64 {
        self.data_bytes
    }
}

/// Maintains an optimized representation of a batch of [`ChunkStoreEvent`]s specifically designed to
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
    pub fn new(store_events: &[&ChunkStoreEvent]) -> Self {
        let mut this = Self {
            row_ids: store_events
                .iter()
                .flat_map(|event| event.chunk.row_ids())
                .collect(),
            temporal: Default::default(),
            timeless: Default::default(),
        };

        for event in store_events {
            if event.is_static() {
                let per_component = this
                    .timeless
                    .entry(event.chunk.entity_path().hash())
                    .or_default();
                for component_name in event.chunk.component_names() {
                    *per_component.entry(component_name).or_default() +=
                        event.delta().unsigned_abs();
                }
            } else {
                for (&timeline, time_chunk) in event.chunk.timelines() {
                    let per_timeline = this
                        .temporal
                        .entry(event.chunk.entity_path().hash())
                        .or_default();
                    for &time in time_chunk.times_raw() {
                        let per_component = per_timeline.entry(timeline).or_default();
                        for component_name in event.chunk.component_names() {
                            per_component
                                .entry(component_name)
                                .or_default()
                                .push(TimeInt::new_temporal(time));
                        }
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
            subtree: Default::default(),
        }
    }

    /// Has no child entities.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Returns `false` if this entity has no children and no data.
    pub fn is_empty(&self, chunk_store: &ChunkStore) -> bool {
        self.children.is_empty()
            && !chunk_store.entity_has_any_component_on_any_timeline(&self.path)
    }

    /// Updates the [`EntityTree`] by applying a batch of [`ChunkStoreEvent`]s.
    ///
    /// Only reacts to deletions (`event.kind == StoreDiffKind::Deletion`).
    pub fn on_store_additions(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();
        for event in events
            .iter()
            .filter(|e| e.kind == ChunkStoreDiffKind::Addition)
        {
            self.on_store_addition(event);
        }
    }

    fn on_store_addition(&mut self, event: &ChunkStoreEvent) {
        re_tracing::profile_function!();

        let entity_path = event.chunk.entity_path();

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
    }

    /// Updates the [`EntityTree`] by applying a batch of [`ChunkStoreEvent`]s.
    ///
    /// Only reacts to deletions (`event.kind == StoreDiffKind::Deletion`).
    pub fn on_store_deletions(
        &mut self,
        data_store: &ChunkStore,
        store_events: &[ChunkStoreEvent],
    ) {
        re_tracing::profile_function!();

        // Only keep events relevant to this branch of the tree.
        let subtree_events = store_events
            .iter()
            .filter(|e| e.kind == ChunkStoreDiffKind::Deletion)
            .filter(|&e| e.diff.chunk.entity_path().starts_with(&self.path))
            .cloned()
            .collect_vec();

        {
            re_tracing::profile_scope!("subtree");
            for event in &subtree_events {
                self.subtree.on_event(event);
            }
        }

        self.children.retain(|_, entity| {
            // this is placed first, because we'll only know if the child entity is empty after telling it to clear itself.
            entity.on_store_deletions(data_store, &subtree_events);

            !entity.is_empty(data_store)
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
    pub fn visit_children_recursively(&self, mut visitor: impl FnMut(&EntityPath)) {
        fn visit(this: &EntityTree, visitor: &mut impl FnMut(&EntityPath)) {
            visitor(&this.path);
            for child in this.children.values() {
                visit(child, visitor);
            }
        }

        visit(self, &mut visitor);
    }

    pub fn find_child_recursive(
        &self,
        mut predicate: impl FnMut(&EntityPath) -> bool,
    ) -> Option<&Self> {
        use std::ops::ControlFlow;

        fn visit<'a>(
            this: &'a EntityTree,
            predicate: &mut impl FnMut(&EntityPath) -> bool,
        ) -> ControlFlow<&'a EntityTree> {
            if predicate(&this.path) {
                return ControlFlow::Break(this);
            };
            for child in this.children.values() {
                visit(child, predicate)?;
            }
            ControlFlow::Continue(())
        }

        let result = visit(self, &mut predicate);
        match result {
            ControlFlow::Continue(()) => None,
            ControlFlow::Break(v) => Some(v),
        }
    }
}
