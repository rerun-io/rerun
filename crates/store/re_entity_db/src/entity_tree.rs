use std::collections::BTreeMap;

use ahash::HashSet;
use nohash_hasher::{IntMap, IntSet};
use re_chunk::{ComponentIdentifier, RowId, TimelineName};
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPath, EntityPathHash, EntityPathPart, TimeInt};
use re_query::StorageEngineReadGuard;

// ----------------------------------------------------------------------------

/// A recursive, manually updated [`ChunkStoreSubscriber`] that maintains the entity hierarchy.
///
/// The tree contains a list of subtrees, and so on recursively.
#[derive(Debug, Clone)]
pub struct EntityTree {
    /// Full path prefix to the root of this (sub)tree.
    pub path: EntityPath,

    /// Direct descendants of this (sub)tree.
    pub children: BTreeMap<EntityPathPart, Self>,
}

impl Default for EntityTree {
    fn default() -> Self {
        Self::root()
    }
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

    #[expect(clippy::unimplemented)]
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
    pub temporal:
        IntMap<EntityPathHash, IntMap<TimelineName, IntMap<ComponentIdentifier, Vec<TimeInt>>>>,

    /// For each entity+component, how many static entries were deleted?
    pub static_: IntMap<EntityPathHash, IntMap<ComponentIdentifier, u64>>,
}

impl CompactedStoreEvents {
    pub fn new(store_events: &[&ChunkStoreEvent]) -> Self {
        let mut this = Self {
            row_ids: store_events
                .iter()
                .flat_map(|event| event.chunk_before_processing.row_ids())
                .collect(),
            temporal: Default::default(),
            static_: Default::default(),
        };

        for event in store_events {
            if event.is_static() {
                let per_component = this
                    .static_
                    .entry(event.chunk_before_processing.entity_path().hash())
                    .or_default();
                for component in event.chunk_before_processing.components_identifiers() {
                    *per_component.entry(component).or_default() += event.delta().unsigned_abs();
                }
            } else {
                for (&timeline, time_column) in event.chunk_before_processing.timelines() {
                    let per_timeline = this
                        .temporal
                        .entry(event.chunk_before_processing.entity_path().hash())
                        .or_default();
                    for &time in time_column.times_raw() {
                        let per_component = per_timeline.entry(timeline).or_default();
                        for component in event.chunk_before_processing.components_identifiers() {
                            per_component
                                .entry(component)
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

    /// Returns `true` if this entity has no children and no data.
    ///
    /// Checking for the absence of data is neither costly nor totally free: do it a few hundreds or
    /// thousands times a frame and it will absolutely kill framerate.
    /// Don't blindly call this on every existing entity every frame: use [`ChunkStoreEvent`]s to make
    /// sure anything changed at all first.
    pub fn check_is_empty(&self, engine: &StorageEngineReadGuard<'_>) -> bool {
        self.children.is_empty() && !engine.store().entity_has_data(&self.path)
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
        self.on_new_entity(event.chunk_before_processing.entity_path());
    }

    pub fn on_new_entity(&mut self, entity_path: &EntityPath) {
        re_tracing::profile_function!();

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
    pub fn on_store_deletions(
        &mut self,
        engine: &StorageEngineReadGuard<'_>,
        entity_paths_with_deletions: &IntSet<EntityPath>,
        events: &[ChunkStoreEvent],
    ) {
        // NOTE: no re_tracing here because this is a recursive function
        if entity_paths_with_deletions.is_empty() {
            return; // early-out
        }

        // We don't actually use the events for anything, we just want to
        // have a direct dependency on the chunk store which must have
        // produced them by the time this function was called.
        let _ = events;

        self.children.retain(|_, entity| {
            // this is placed first, because we'll only know if the child entity is empty after telling it to clear itself.
            entity.on_store_deletions(engine, entity_paths_with_deletions, events);

            let has_children = || !entity.children.is_empty();
            // Checking for lack of data is not free, so make sure there is any reason to believe
            // that any relevant data has changed first.
            let has_recursive_deletion_events = || {
                entity_paths_with_deletions
                    .iter()
                    .any(|removed_entity_path| removed_entity_path.starts_with(&entity.path))
            };
            let has_data = || engine.store().entity_has_data(&entity.path);

            let should_be_removed =
                !has_children() && (has_recursive_deletion_events() && !has_data());

            !should_be_removed
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

    /// Invokes visitor for `self` and all children recursively.
    pub fn visit_children_recursively(&self, mut visitor: impl FnMut(&EntityPath)) {
        fn visit(this: &EntityTree, visitor: &mut impl FnMut(&EntityPath)) {
            visitor(&this.path);
            for child in this.children.values() {
                visit(child, visitor);
            }
        }

        visit(self, &mut visitor);
    }

    /// Invokes the `predicate` for `self` and all children recursively,
    /// returning the _first_ entity for which the `predicate` returns `true`.
    ///
    /// Note that this function has early return semantics, meaning if multiple
    /// entities would return `true`, only the first is returned.
    /// The entities are yielded in order of their entity paths.
    pub fn find_first_child_recursive(
        &self,
        mut predicate: impl FnMut(&EntityPath) -> bool,
    ) -> Option<&Self> {
        fn visit<'a>(
            this: &'a EntityTree,
            predicate: &mut impl FnMut(&EntityPath) -> bool,
        ) -> Option<&'a EntityTree> {
            if predicate(&this.path) {
                return Some(this);
            }

            for child in this.children.values() {
                if let Some(subtree) = visit(child, predicate) {
                    // Early return
                    return Some(subtree);
                }
            }

            None
        }

        visit(self, &mut predicate)
    }
}

impl re_byte_size::SizeBytes for EntityTree {
    fn heap_size_bytes(&self) -> u64 {
        let Self { path, children } = self;
        path.heap_size_bytes() + children.heap_size_bytes()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{EntityPath, StoreId, TimePoint, Timeline};

    use crate::EntityDb;

    #[test]
    fn deleting_descendants() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut db = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));

        let timeline_frame = Timeline::new_sequence("frame");

        let entity_path_parent: EntityPath = "parent".into();
        let entity_path_child: EntityPath = "parent/child1".into();
        let entity_path_grandchild: EntityPath = "parent/child1/grandchild".into();

        assert!(db.tree().check_is_empty(&db.storage_engine()));

        {
            let row_id = RowId::new();
            let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
            let point = MyPoint::new(1.0, 2.0);
            let chunk = Chunk::builder(entity_path_grandchild.clone())
                .with_component_batches(
                    row_id,
                    timepoint,
                    [(MyPoints::descriptor_points(), &[point] as _)],
                )
                .build()?;

            db.add_chunk(&Arc::new(chunk))?;
        }

        {
            let parent = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_parent)
                .unwrap();
            let child = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_child)
                .unwrap();
            let grandchild = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_grandchild)
                .unwrap();

            assert_eq!(1, parent.children.len());
            assert_eq!(1, child.children.len());
            assert_eq!(0, grandchild.children.len());

            assert!(!db.tree().check_is_empty(&db.storage_engine()));
            assert!(!parent.check_is_empty(&db.storage_engine()));
            assert!(!child.check_is_empty(&db.storage_engine()));
            assert!(!grandchild.check_is_empty(&db.storage_engine()));
        }

        let store_events = db.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        db.on_store_events(&store_events);

        {
            let parent = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_parent);
            let child = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_child);
            let grandchild = db
                .tree()
                .find_first_child_recursive(|entity_path| *entity_path == entity_path_grandchild);

            assert!(db.tree().check_is_empty(&db.storage_engine()));
            assert!(parent.is_none());
            assert!(child.is_none());
            assert!(grandchild.is_none());
        }

        Ok(())
    }
}
