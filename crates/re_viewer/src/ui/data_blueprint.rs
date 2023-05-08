use std::collections::BTreeSet;

use nohash_hasher::{IntMap, IntSet};
use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap};
use slotmap::SlotMap;
use smallvec::{smallvec, SmallVec};

slotmap::new_key_type! { pub struct DataBlueprintGroupHandle; }

/// A grouping of several data-blueprints.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintGroup {
    pub display_name: String,

    /// Individual settings. Mutate & display this.
    pub properties_individual: EntityProperties,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame from [`Self::properties_individual`].
    #[cfg_attr(feature = "serde", serde(skip))]
    pub properties_projected: EntityProperties,

    /// Parent of this blueprint group. Every data blueprint except the root has a parent.
    pub parent: DataBlueprintGroupHandle,

    pub children: SmallVec<[DataBlueprintGroupHandle; 4]>,

    /// Direct child entities of this blueprint group.
    ///
    /// Musn't be a `HashSet` because we want to preserve order of entity paths.
    pub entities: BTreeSet<EntityPath>,
}

/// Data blueprints for all entity paths in a space view.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
struct DataBlueprints {
    /// Individual settings. Mutate this.
    individual: EntityPropertyMap,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame from [`Self::individual`].
    #[cfg_attr(feature = "serde", serde(skip))]
    projected: EntityPropertyMap,
}

/// Tree of all data blueprint groups for a single space view.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintTree {
    /// All data blueprint groups.
    groups: SlotMap<DataBlueprintGroupHandle, DataBlueprintGroup>,

    /// Mapping from entity paths to blueprints.
    ///
    /// We also use this for building up groups from hierarchy, meaning that some paths in here
    /// may not represent existing entities, i.e. the blueprint groups they are pointing to may not
    /// necessarily have the respective path as a child.
    path_to_group: IntMap<EntityPath, DataBlueprintGroupHandle>,

    /// List of all entities that we query via this data blueprint collection.
    ///
    /// Two things to keep in sync:
    /// * children on [`DataBlueprintGroup`] this is on
    /// * elements in [`Self::path_to_group`]
    /// TODO(andreas): Can we reduce the amount of these dependencies?
    entity_paths: IntSet<EntityPath>,

    /// Root group, always exists as a placeholder
    root_group_handle: DataBlueprintGroupHandle,

    data_blueprints: DataBlueprints,
}

impl Default for DataBlueprintTree {
    fn default() -> Self {
        let mut groups = SlotMap::default();
        let root_group = groups.insert(DataBlueprintGroup::default());

        let mut path_to_blueprint = IntMap::default();
        path_to_blueprint.insert(EntityPath::root(), root_group);

        Self {
            groups,
            path_to_group: path_to_blueprint,
            entity_paths: IntSet::default(),
            root_group_handle: root_group,
            data_blueprints: DataBlueprints::default(),
        }
    }
}

impl DataBlueprintTree {
    /// Returns a handle to the root data blueprint.
    ///
    /// Even if there are no other groups, we always have a root group at the top.
    /// Typically, we don't show the root group in the ui.
    pub fn root_handle(&self) -> DataBlueprintGroupHandle {
        self.root_group_handle
    }

    /// Returns the root data blueprint.
    ///
    /// Even if there are no other groups, we always have a root group at the top.
    /// Typically, we don't show the root group in the ui.
    pub fn root_group(&self) -> &DataBlueprintGroup {
        self.groups.get(self.root_group_handle).unwrap()
    }

    /// Resolves a data blueprint group handle.
    pub fn group(&self, handle: DataBlueprintGroupHandle) -> Option<&DataBlueprintGroup> {
        self.groups.get(handle)
    }

    /// Resolves a data blueprint group handle.
    pub fn group_mut(
        &mut self,
        handle: DataBlueprintGroupHandle,
    ) -> Option<&mut DataBlueprintGroup> {
        self.groups.get_mut(handle)
    }

    /// Calls the visitor function on every entity path in the given group and its descending groups.
    pub fn visit_group_entities_recursively(
        &self,
        handle: DataBlueprintGroupHandle,
        visitor: &mut impl FnMut(&EntityPath),
    ) {
        let Some(group) = self.groups.get(handle) else {
            return;
        };

        for entity in &group.entities {
            visitor(entity);
        }

        for child in &group.children {
            self.visit_group_entities_recursively(*child, visitor);
        }
    }

    /// Returns entity properties with the hierarchy applied.
    pub fn data_blueprints_projected(&self) -> &EntityPropertyMap {
        &self.data_blueprints.projected
    }

    /// Returns mutable individual entity properties, the hierarchy was not applied to this.
    pub fn data_blueprints_individual(&mut self) -> &mut EntityPropertyMap {
        &mut self.data_blueprints.individual
    }

    pub fn contains_entity(&self, path: &EntityPath) -> bool {
        self.entity_paths.contains(path)
    }

    /// List of all entities that we query via this data blueprint collection.
    pub fn entity_paths(&self) -> &IntSet<EntityPath> {
        &self.entity_paths
    }

    /// Should be called on frame start.
    ///
    /// Propagates any data blueprint changes along the tree.
    pub fn on_frame_start(&mut self) {
        crate::profile_function!();

        // NOTE: We could do this projection only when the entity properties changes
        // and/or when new entity paths are added, but such memoization would add complexity.

        fn project_tree(
            tree: &mut DataBlueprintTree,
            parent_properties: &EntityProperties,
            group_handle: DataBlueprintGroupHandle,
        ) {
            let Some(group) = tree.groups.get_mut(group_handle) else {
                debug_assert!(false, "Invalid group handle in blueprint group tree");
                return;
            };

            let group_properties_projected =
                parent_properties.with_child(&group.properties_individual);
            group.properties_projected = group_properties_projected.clone();

            for entity_path in &group.entities {
                let projected_properties = group_properties_projected
                    .with_child(&tree.data_blueprints.individual.get(entity_path));
                tree.data_blueprints
                    .projected
                    .set(entity_path.clone(), projected_properties);
            }

            let children = group.children.clone(); // TODO(andreas): How to avoid this clone?
            for child in &children {
                project_tree(tree, &group_properties_projected, *child);
            }
        }

        project_tree(self, &EntityProperties::default(), self.root_group_handle);
    }

    /// Adds a list of entity paths to the tree, using grouping as dictated by their entity path hierarchy.
    ///
    /// `base_path` indicates a path at which we short-circuit to the root group.
    ///
    /// Creates a group at *every* step of every path, unless a new group would only contain the entity itself.
    pub fn insert_entities_according_to_hierarchy<'a>(
        &mut self,
        paths: impl Iterator<Item = &'a EntityPath>,
        base_path: &EntityPath,
    ) {
        crate::profile_function!();

        self.entity_paths.clear();
        let mut new_leaf_groups = Vec::new();
        for path in paths {
            self.entity_paths.insert(path.clone());

            // Is there already a group associated with this exact path? (maybe because a child was logged there earlier)
            // If so, we can simply move it under this existing group.
            let group_handle = if let Some(group_handle) = self.path_to_group.get(path) {
                *group_handle
            } else if path == base_path {
                // An entity might have directly been logged on the base_path. We map then to the root!
                self.root_group_handle
            } else {
                // Otherwise, create a new group which only contains this entity and add the group to the hierarchy.
                let new_group = self.groups.insert(DataBlueprintGroup {
                    display_name: path_to_group_name(path),
                    ..Default::default()
                });
                self.add_group_to_hierarchy_recursively(new_group, path, base_path);
                new_leaf_groups.push(new_group);
                new_group
            };

            self.add_entity_to_group(group_handle, path);
        }

        // If a leaf group contains only a single element, move that element to the parent and remove the leaf again.
        // (we can't do this as we iterate initially on `paths`, as we don't know if we're data on non-leaf paths until we touched all of them)
        for leaf_group_handle in new_leaf_groups {
            let Some(leaf_group) = self.groups.get_mut(leaf_group_handle) else {
                continue;
            };
            if !leaf_group.children.is_empty() || leaf_group.entities.len() != 1 {
                continue;
            }

            // Remove group.
            let single_entity = leaf_group.entities.iter().next().unwrap().clone();
            let parent_group_handle = leaf_group.parent;
            self.groups.remove(leaf_group_handle);

            // Add entity to its parent and remove the now deleted child.
            let parent_group = self.groups.get_mut(parent_group_handle).unwrap();
            parent_group
                .children
                .retain(|child_group| *child_group != leaf_group_handle);
            parent_group.entities.insert(single_entity.clone());
            self.path_to_group
                .insert(single_entity, parent_group_handle);
        }
    }

    fn add_group_to_hierarchy_recursively(
        &mut self,
        new_group: DataBlueprintGroupHandle,
        associated_path: &EntityPath,
        base_path: &EntityPath,
    ) {
        let Some(mut parent_path) = associated_path.parent() else {
            // Already the root, nothing to do.
            return;
        };

        // Short circuit to the root group at base_path.
        // If the entity is outside of the base path we would walk up all the way to the root
        if &parent_path == base_path {
            parent_path = EntityPath::root();
        }

        let parent_group = match self.path_to_group.entry(parent_path.clone()) {
            std::collections::hash_map::Entry::Occupied(parent_group) => {
                let parent_group = *parent_group.get();
                self.groups
                    .get_mut(parent_group)
                    .unwrap()
                    .children
                    .push(new_group);
                parent_group
            }

            std::collections::hash_map::Entry::Vacant(vacant_mapping) => {
                let parent_group = self.groups.insert(DataBlueprintGroup {
                    display_name: path_to_group_name(&parent_path),
                    children: smallvec![new_group],
                    ..Default::default()
                });
                vacant_mapping.insert(parent_group);
                self.add_group_to_hierarchy_recursively(parent_group, &parent_path, base_path);
                parent_group
            }
        };

        self.groups.get_mut(new_group).unwrap().parent = parent_group;
    }

    /// Adds an entity path to a group.
    ///
    /// If it was already associated with this group, nothing will happen.
    /// If it was already associated with a different group, it will move from there.
    pub fn add_entity_to_group(
        &mut self,
        group_handle: DataBlueprintGroupHandle,
        path: &EntityPath,
    ) {
        if let Some(group) = self.groups.get_mut(group_handle) {
            if !group.entities.insert(path.clone()) {
                // If the entity was already in here it won't be in another group previously.
                return;
            }
        } else {
            return;
        }

        if let Some(previous_group) = self.path_to_group.insert(path.clone(), group_handle) {
            if previous_group != group_handle {
                if let Some(previous_group) = self.groups.get_mut(previous_group) {
                    previous_group.entities.retain(|ent| ent != path);
                }
            }
        }
    }

    /// Removes an entity from the data blueprint collection.
    ///
    /// If the entity was not known by this data blueprint tree nothing happens.
    pub fn remove_entity(&mut self, path: &EntityPath) {
        if let Some(group_handle) = self.path_to_group.get(path) {
            if let Some(group) = self.groups.get_mut(*group_handle) {
                group.entities.remove(path);
                self.remove_group_if_empty(*group_handle);
            }
        }
        self.path_to_group.remove(path);
        self.entity_paths.remove(path);
    }

    /// Removes a group and all its entities and subgroups from the blueprint tree
    pub fn remove_group(&mut self, group_handle: DataBlueprintGroupHandle) {
        crate::profile_function!();

        let Some(group) = self.groups.get(group_handle) else {
            return;
        };

        // Clone group to work around borrow checker issues.
        let group = group.clone();

        // Remove all child groups.
        for child_group in &group.children {
            self.remove_group(*child_group);
        }

        // Remove all child entities.
        for entity_path in &group.entities {
            self.entity_paths.remove(entity_path);
        }

        // Remove from `path_to_group` map.
        // `path_to_group` may map arbitrary paths to this group, some of which aren't in the entity_paths list!
        self.path_to_group
            .retain(|_, group_mapping| *group_mapping != group_handle);

        // Remove group from parent group
        if let Some(parent_group) = self.groups.get_mut(group.parent) {
            parent_group
                .children
                .retain(|child_group| *child_group != group_handle);
        }

        // Never completely remove the root group.
        if group_handle != self.root_group_handle {
            self.groups.remove(group_handle);
        }
    }

    fn remove_group_if_empty(&mut self, group_handle: DataBlueprintGroupHandle) {
        let Some(group) = self.groups.get(group_handle) else {
            return;
        };
        if group.entities.is_empty() && group.children.is_empty() {
            let parent_group_handle = group.parent;
            if let Some(parent_group) = self.groups.get_mut(parent_group_handle) {
                parent_group
                    .children
                    .retain(|child_group| *child_group != group_handle);
                self.remove_group_if_empty(parent_group_handle);
            }
        }
    }
}

fn path_to_group_name(path: &EntityPath) -> String {
    path.iter().last().map_or("/".to_owned(), |c| c.to_string())
}
