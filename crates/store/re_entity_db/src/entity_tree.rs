use std::collections::BTreeMap;

use ahash::HashSet;
use nohash_hasher::IntMap;

use re_chunk::RowId;
use re_chunk_store::{ChunkStore, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPath, EntityPathHash, EntityPathPart, TimeInt, Timeline};
use re_types_core::ComponentName;

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

    /// Returns `false` if this entity has no children and no data.
    pub fn is_empty(&self, chunk_store: &ChunkStore) -> bool {
        self.children.is_empty()
            && !chunk_store.entity_has_any_component_on_any_timeline(&self.path)
    }

    /// Updates the [`EntityTree`] by applying a batch of [`ChunkStoreEvent`]s,
    /// adding any new entities to the tree.
    ///
    /// Only reacts to additions (`event.kind == StoreDiffKind::Addition`).
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
    }

    /// Updates the [`EntityTree`] by removing any entities which have no data and no children.
    ///
    /// ⚠ This depends on `data_store` having up-to-date records of which entities have no data,
    /// so it must have already been notified of any chunk events prior to calling this method.
    pub fn on_store_deletions(&mut self, data_store: &ChunkStore) {
        re_tracing::profile_function!();

        self.children.retain(|_, entity| {
            // this is placed first, because we'll only know if the child entity is empty after telling it to clear itself.
            entity.on_store_deletions(data_store);

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
                [first, rest @ ..] => {
                    let child = this.children.get(first)?;
                    subtree_recursive(child, rest)
                }
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
