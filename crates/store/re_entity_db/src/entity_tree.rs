use std::collections::BTreeMap;

use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_chunk::RowId;
use re_chunk_store::{ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPath, EntityPathHash, EntityPathPart, TimeInt, Timeline};
use re_types_core::ComponentName;

// Used all over in docstrings.
#[allow(unused_imports)]
use re_chunk_store::ChunkStore;

// ----------------------------------------------------------------------------

/// A recursive, manually updated [`ChunkStoreSubscriber`] that maintains the entity hierarchy.
///
/// The tree contains a list of subtrees, and so on recursively.
pub struct EntityTree {
    /// Full path prefix to the root of this (sub)tree.
    pub path: EntityPath,

    /// Direct descendants of this (sub)tree.
    pub children: BTreeMap<EntityPathPart, EntityTree>,
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
        }
    }

    /// Has no child entities.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn num_children(&self) -> usize {
        self.children.len()
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

        for (i, part) in entity_path.iter().enumerate() {
            tree = tree
                .children
                .entry(part.clone())
                .or_insert_with(|| Self::new(entity_path.as_slice()[..=i].into()));
        }

        // Finally book-keeping for the entity where data was actually added:
        tree.on_added_data(&event.diff);
    }

    /// Handles the addition of new data into the tree.
    fn on_added_data(&mut self, store_diff: &ChunkStoreDiff) {
        let _ = (self, store_diff);
    }

    /// Updates the [`EntityTree`] by applying a batch of [`ChunkStoreEvent`]s.
    ///
    /// Only reacts to deletions (`event.kind == StoreDiffKind::Deletion`).
    pub fn on_store_deletions(&mut self, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let Self { path, children } = self;

        // Only keep events relevant to this branch of the tree.
        let subtree_events = store_events
            .iter()
            .filter(|e| e.kind == ChunkStoreDiffKind::Deletion)
            .filter(|&e| e.diff.chunk.entity_path().starts_with(path))
            .cloned()
            .collect_vec();

        children.retain(|_, child| {
            child.on_store_deletions(&subtree_events);
            child.num_children() > 0
        });
    }

    pub fn subtree(&self, path: &EntityPath) -> Option<&Self> {
        let mut this = self;
        let mut path = path.as_slice();
        loop {
            match path {
                [] => return Some(this),
                [first, rest @ ..] => {
                    this = this.children.get(first)?;
                    path = rest;
                }
            }
        }
    }

    // Invokes visitor for `self` and all children recursively.
    pub fn visit_children_recursively(&self, visitor: &mut impl FnMut(&EntityPath)) {
        visitor(&self.path);
        for child in self.children.values() {
            child.visit_children_recursively(visitor);
        }
    }
}
