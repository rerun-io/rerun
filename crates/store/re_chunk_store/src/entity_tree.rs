use std::collections::BTreeMap;

use re_log_types::{EntityPath, EntityPathPart};

// ----------------------------------------------------------------------------

/// A recursive tree structure that maintains the entity hierarchy.
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

    /// Removes leaf entities that have no children and for which `entity_has_data` returns false.
    ///
    /// This is called after store deletions to keep the tree in sync with the actual data.
    pub fn prune_empty_entities(&mut self, entity_has_data: &impl Fn(&EntityPath) -> bool) {
        self.children.retain(|_, child| {
            child.prune_empty_entities(entity_has_data);
            let has_children = !child.children.is_empty();
            let has_data = entity_has_data(&child.path);
            has_children || has_data
        });
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
    use super::*;

    #[test]
    fn prune_removes_empty_leaves() {
        let mut tree = EntityTree::root();
        let parent: EntityPath = "parent".into();
        let child: EntityPath = "parent/child".into();
        let grandchild: EntityPath = "parent/child/grandchild".into();

        tree.on_new_entity(&grandchild);

        assert!(tree.subtree(&parent).is_some());
        assert!(tree.subtree(&child).is_some());
        assert!(tree.subtree(&grandchild).is_some());

        // Only grandchild has data
        tree.prune_empty_entities(&|path| *path == grandchild);
        assert!(tree.subtree(&parent).is_some());
        assert!(tree.subtree(&child).is_some());
        assert!(tree.subtree(&grandchild).is_some());

        // No entity has data, all should be pruned
        tree.prune_empty_entities(&|_| false);
        assert!(tree.subtree(&parent).is_none());
        assert!(tree.subtree(&child).is_none());
        assert!(tree.subtree(&grandchild).is_none());
        assert!(tree.children.is_empty());
    }

    #[test]
    fn prune_keeps_parents_with_children() {
        let mut tree = EntityTree::root();
        let parent: EntityPath = "parent".into();
        let child_a: EntityPath = "parent/a".into();
        let child_b: EntityPath = "parent/b".into();

        tree.on_new_entity(&child_a);
        tree.on_new_entity(&child_b);

        // Only child_b has data -- parent and child_a have no data
        // but parent should stay because child_b is still there
        tree.prune_empty_entities(&|path| *path == child_b);
        assert!(tree.subtree(&parent).is_some());
        assert!(tree.subtree(&child_a).is_none());
        assert!(tree.subtree(&child_b).is_some());
    }
}
