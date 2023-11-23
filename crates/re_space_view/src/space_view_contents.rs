use std::collections::{BTreeMap, BTreeSet};

use nohash_hasher::IntMap;
use re_data_store::{EntityPath, EntityProperties};
use re_viewer_context::{
    DataBlueprintGroupHandle, DataResult, EntitiesPerSystemPerClass, PerSystemEntities,
    SpaceViewId, StoreContext, ViewSystemName,
};
use slotmap::SlotMap;
use smallvec::{smallvec, SmallVec};

use crate::{
    DataQuery, DataResultHandle, DataResultNode, DataResultTree, EntityOverrides, PropertyResolver,
};

/// A grouping of several data-blueprints.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintGroup {
    pub display_name: String,

    pub group_path: EntityPath,

    /// Parent of this blueprint group. Every data blueprint except the root has a parent.
    pub parent: DataBlueprintGroupHandle,

    pub children: SmallVec<[DataBlueprintGroupHandle; 4]>,

    /// Direct child entities of this blueprint group.
    ///
    /// Musn't be a `HashSet` because we want to preserve order of entity paths.
    pub entities: BTreeSet<EntityPath>,
}

impl Default for DataBlueprintGroup {
    fn default() -> Self {
        DataBlueprintGroup {
            display_name: Default::default(),
            group_path: EntityPath::root(),
            parent: Default::default(),
            children: Default::default(),
            entities: Default::default(),
        }
    }
}

impl DataBlueprintGroup {
    /// Determine whether this `DataBlueprints` has user-edits relative to another `DataBlueprints`
    fn has_edits(&self, other: &Self) -> bool {
        let Self {
            display_name,
            group_path: _,
            parent,
            children,
            entities,
        } = self;

        display_name != &other.display_name
            || parent != &other.parent
            || children != &other.children
            || entities != &other.entities
    }
}

/// Tree of all data blueprint groups for a single space view.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct SpaceViewContents {
    /// The space view these contents belong to.
    pub space_view_id: SpaceViewId,

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
    /// Currently this is reset every frame in `SpaceViewBlueprint::reset_systems_per_entity_path`.
    /// In the future, we may want to keep this around and only add/remove systems
    /// for entities. But at this point we'd likely handle the heuristics a bit differently as well
    /// and don't use serde here for serialization.
    #[serde(skip)]
    per_system_entity_list: PerSystemEntities,

    /// Root group, always exists as a placeholder
    root_group_handle: DataBlueprintGroupHandle,
}

/// Determine whether this `DataBlueprintTree` has user-edits relative to another `DataBlueprintTree`
impl SpaceViewContents {
    pub const INDIVIDUAL_OVERRIDES_PREFIX: &str = "individual_overrides";
    pub const GROUP_OVERRIDES_PREFIX: &str = "group_overrides";

    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            space_view_id: _,
            groups,
            path_to_group,
            per_system_entity_list: _,
            root_group_handle,
        } = self;

        groups.len() != other.groups.len()
            || groups.iter().any(|(key, val)| {
                other
                    .groups
                    .get(key)
                    .map_or(true, |other_val| val.has_edits(other_val))
            })
            || *path_to_group != other.path_to_group
            || *root_group_handle != other.root_group_handle
    }
}

impl SpaceViewContents {
    pub fn new(id: SpaceViewId) -> Self {
        let mut groups = SlotMap::default();
        let root_group = groups.insert(DataBlueprintGroup::default());

        let mut path_to_blueprint = IntMap::default();
        path_to_blueprint.insert(EntityPath::root(), root_group);

        Self {
            space_view_id: id,
            groups,
            path_to_group: path_to_blueprint,
            per_system_entity_list: BTreeMap::default(),
            root_group_handle: root_group,
        }
    }
}

impl SpaceViewContents {
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

    #[inline]
    /// Looks up the group handle for an entity path.
    pub fn group_handle_for_entity_path(
        &self,
        path: &EntityPath,
    ) -> Option<DataBlueprintGroupHandle> {
        self.path_to_group.get(path).cloned()
    }

    pub fn contains_entity(&self, path: &EntityPath) -> bool {
        // If an entity is in path_to_group it is *likely* also an entity in the Space View.
        // However, it could be that the path *only* refers to a group, not also an entity.
        // So once we resolved the group, we need to check if it contains the entity of interest.
        self.path_to_group
            .get(path)
            .and_then(|group| {
                self.groups
                    .get(*group)
                    .and_then(|group| group.entities.get(path))
            })
            .is_some()
    }

    /// List of all entities that we query via this data blueprint collection.
    pub fn entity_paths(&self) -> impl Iterator<Item = &EntityPath> {
        // Each entity is only ever in one group, therefore collecting all entities from all groups, gives us all entities.
        self.groups.values().flat_map(|group| group.entities.iter())
    }

    pub fn per_system_entities(&self) -> &PerSystemEntities {
        &self.per_system_entity_list
    }

    pub fn per_system_entities_mut(&mut self) -> &mut PerSystemEntities {
        &mut self.per_system_entity_list
    }

    pub fn contains_all_entities_from(&self, other: &SpaceViewContents) -> bool {
        for (system, entities) in &other.per_system_entity_list {
            let Some(self_entities) = self.per_system_entity_list.get(system) else {
                if entities.is_empty() {
                    continue;
                } else {
                    return false;
                }
            };
            if !entities.is_subset(self_entities) {
                return false;
            }
        }
        true
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
        re_tracing::profile_function!();

        let mut new_leaf_groups = Vec::new();

        for path in paths {
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
                    group_path: path.clone(),
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
                    group_path: parent_path.clone(),
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
        re_tracing::profile_function!();

        if let Some(group_handle) = self.path_to_group.get(path) {
            if let Some(group) = self.groups.get_mut(*group_handle) {
                group.entities.remove(path);
                self.remove_group_if_empty(*group_handle);
            }
        }
        self.path_to_group.remove(path);

        for per_system_list in self.per_system_entity_list.values_mut() {
            per_system_list.remove(path);
        }
    }

    /// Removes a group and all its entities and subgroups from the blueprint tree
    pub fn remove_group(&mut self, group_handle: DataBlueprintGroupHandle) {
        re_tracing::profile_function!();

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
            for per_system_list in self.per_system_entity_list.values_mut() {
                per_system_list.remove(entity_path);
            }
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

    #[inline]
    pub fn entity_path(&self) -> EntityPath {
        self.space_view_id.as_entity_path()
    }

    /// Find all `ViewParts` that this [`SpaceViewContents`] thinks are relevant for the given entity path.
    // TODO(jleibs): This inversion of data-structure is not great, but I believe this goes away as we
    // implement a more direct heuristic evaluation in the future.
    pub fn view_parts_for_entity_path(
        &self,
        entity_path: &EntityPath,
    ) -> SmallVec<[ViewSystemName; 4]> {
        re_tracing::profile_function!();
        self.per_system_entities()
            .iter()
            .filter_map(|(part, ents)| {
                if ents.contains(entity_path) {
                    Some(*part)
                } else {
                    None
                }
            })
            .collect()
    }
}

fn path_to_group_name(path: &EntityPath) -> String {
    path.iter().last().map_or("/".to_owned(), |c| c.to_string())
}

// ----------------------------------------------------------------------------
// Implement the `DataQuery` interface for `SpaceViewContents`

impl DataQuery for SpaceViewContents {
    fn execute_query(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &StoreContext<'_>,
        _entities_per_system_per_class: &EntitiesPerSystemPerClass,
    ) -> DataResultTree {
        re_tracing::profile_function!();
        let overrides = property_resolver.resolve_entity_overrides(ctx);
        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();
        let root_handle = Some(self.root_group().add_to_data_results_recursive(
            self,
            &overrides,
            None,
            &overrides.root,
            &mut data_results,
        ));
        DataResultTree {
            data_results,
            root_handle,
        }
    }

    fn resolve(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &StoreContext<'_>,
        _entities_per_system_per_class: &EntitiesPerSystemPerClass,
        entity_path: &EntityPath,
        as_group: bool,
    ) -> DataResult {
        re_tracing::profile_function!();
        let overrides = property_resolver.resolve_entity_overrides(ctx);

        let view_parts = self
            .per_system_entities()
            .iter()
            .filter_map(|(part, ents)| {
                if ents.contains(entity_path) {
                    Some(*part)
                } else {
                    None
                }
            })
            .collect();

        // Start with the root override
        let mut resolved_properties = overrides.root;

        // Merge in any group overrides
        for prefix in EntityPath::incremental_walk(None, entity_path) {
            if let Some(props) = overrides.group.get_opt(&prefix) {
                resolved_properties = resolved_properties.with_child(props);
            }
        }

        if as_group {
            DataResult {
                entity_path: entity_path.clone(),
                view_parts,
                is_group: true,
                resolved_properties,
                individual_properties: overrides.group.get_opt(entity_path).cloned(),
                override_path: self
                    .entity_path()
                    .join(&SpaceViewContents::GROUP_OVERRIDES_PREFIX.into())
                    .join(entity_path),
            }
        } else {
            // Finally apply the individual overrides
            if let Some(props) = overrides.individual.get_opt(entity_path) {
                resolved_properties = resolved_properties.with_child(props);
            }

            DataResult {
                entity_path: entity_path.clone(),
                view_parts,
                is_group: false,
                resolved_properties,
                individual_properties: overrides.individual.get_opt(entity_path).cloned(),
                override_path: self
                    .entity_path()
                    .join(&SpaceViewContents::INDIVIDUAL_OVERRIDES_PREFIX.into())
                    .join(entity_path),
            }
        }
    }
}

impl DataBlueprintGroup {
    /// This recursively walks a `DataBlueprintGroup` and adds every entity / group to the tree.
    ///
    /// Properties are resolved hierarchically from an `EntityPropertyMap` containing all the
    /// overrides. As we walk down the tree.
    fn add_to_data_results_recursive(
        &self,
        contents: &SpaceViewContents,
        overrides: &EntityOverrides,
        inherited_base: Option<&EntityPath>,
        inherited: &EntityProperties,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
    ) -> DataResultHandle {
        let group_path = self.group_path.clone();

        // The group in a SpaceViewContents should never be displayed
        // there will always be a leaf that is the actual entity.
        let group_view_parts = Default::default();

        let mut group_resolved_properties = inherited.clone();

        for prefix in EntityPath::incremental_walk(inherited_base, &group_path) {
            if let Some(props) = overrides.group.get_opt(&prefix) {
                group_resolved_properties = group_resolved_properties.with_child(props);
            }
        }

        let base_entity_path = contents.entity_path();
        let individual_prefix = EntityPath::from(SpaceViewContents::INDIVIDUAL_OVERRIDES_PREFIX);
        let group_prefix = EntityPath::from(SpaceViewContents::GROUP_OVERRIDES_PREFIX);

        // First build up the direct children
        let mut children: SmallVec<_> = self
            .entities
            .iter()
            .cloned()
            .map(|entity_path| {
                let view_parts = contents.view_parts_for_entity_path(&entity_path);

                let mut resolved_properties = group_resolved_properties.clone();

                // Only need to do the incremental walk up from the group
                for prefix in EntityPath::incremental_walk(Some(&group_path), &entity_path) {
                    if let Some(props) = overrides.group.get_opt(&prefix) {
                        resolved_properties = resolved_properties.with_child(props);
                    }
                }

                let individual_properties = overrides.individual.get_opt(&entity_path).cloned();

                if let Some(props) = &individual_properties {
                    resolved_properties = resolved_properties.with_child(props);
                }

                let override_path = base_entity_path.join(&individual_prefix).join(&entity_path);

                data_results.insert(DataResultNode {
                    data_result: DataResult {
                        entity_path,
                        view_parts,
                        is_group: false,
                        individual_properties,
                        resolved_properties,
                        override_path,
                    },
                    children: Default::default(),
                })
            })
            .collect();

        // And then append the recursive children
        let mut recursive_children: SmallVec<[DataResultHandle; 4]> = self
            .children
            .iter()
            .filter_map(|handle| {
                contents.group(*handle).map(|group| {
                    group.add_to_data_results_recursive(
                        contents,
                        overrides,
                        Some(&group_path),
                        &group_resolved_properties,
                        data_results,
                    )
                })
            })
            .collect();

        children.append(&mut recursive_children);

        // The 'individual' properties of a group are the group overrides
        let individual_properties = overrides.group.get_opt(&group_path).cloned();

        let group_override_path = base_entity_path.join(&group_prefix).join(&group_path);

        data_results.insert(DataResultNode {
            data_result: DataResult {
                entity_path: group_path,
                view_parts: group_view_parts,
                is_group: true,
                individual_properties,
                resolved_properties: group_resolved_properties,
                override_path: group_override_path,
            },
            children,
        })
    }
}

// ----------------------------------------------------------------------------
