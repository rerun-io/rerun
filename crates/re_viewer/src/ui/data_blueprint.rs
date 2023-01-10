use nohash_hasher::{IntMap, IntSet};
use re_data_store::{ObjPath, ObjectProps, ObjectsProperties};
use slotmap::SlotMap;
use smallvec::{smallvec, SmallVec};

use crate::misc::ViewerContext;

slotmap::new_key_type! { pub struct DataBlueprintGroupHandle; }

/// A grouping of several data-blueprints.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintGroup {
    pub display_name: String,

    /// Individual settings. Mutate & display this.
    pub properties_individual: ObjectProps,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame from [`Self::properties_individual`].
    #[cfg_attr(feature = "serde", serde(skip))]
    pub properties_projected: ObjectProps,

    /// Parent of this blueprint group. Every data blueprint except the root has a parent.
    pub parent: DataBlueprintGroupHandle,

    pub children: SmallVec<[DataBlueprintGroupHandle; 1]>,

    pub objects: IntSet<ObjPath>,
}

impl DataBlueprintGroup {
    pub fn selection_ui(&mut self, _ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        egui::Grid::new("blueprint_group")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.display_name);
            });
    }
}

/// Data blueprints for all object paths in a space view.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
struct DataBlueprints {
    /// Individual settings. Mutate this.
    individual: ObjectsProperties,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame from [`Self::individual`].
    #[cfg_attr(feature = "serde", serde(skip))]
    projected: ObjectsProperties,
}

/// Tree of all data blueprint groups for a single space view.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintTree {
    /// All data blueprint groups.
    groups: SlotMap<DataBlueprintGroupHandle, DataBlueprintGroup>,

    /// Mapping from object paths to blueprints.
    ///
    /// We also use this for building up groups from hierarchy, meaning that some paths in here
    /// may not represent existing objects, i.e. the blueprint groups they are pointing to may not
    /// necessarily have the respective path as a child.
    path_to_blueprint: IntMap<ObjPath, DataBlueprintGroupHandle>,

    /// Root group, always exists as a placeholder
    root_group_handle: DataBlueprintGroupHandle,

    data_blueprints: DataBlueprints,
}

impl Default for DataBlueprintTree {
    fn default() -> Self {
        let mut groups = SlotMap::default();
        let root_group = groups.insert(DataBlueprintGroup::default());

        let mut path_to_blueprint = IntMap::default();
        path_to_blueprint.insert(ObjPath::root(), root_group);

        Self {
            groups,
            path_to_blueprint,
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

    pub fn root_group(&self) -> &DataBlueprintGroup {
        self.groups.get(self.root_group_handle).unwrap()
    }

    /// Resolves a data blueprint group handle.
    pub fn get_group(&self, handle: DataBlueprintGroupHandle) -> Option<&DataBlueprintGroup> {
        self.groups.get(handle)
    }

    /// Resolves a data blueprint group handle.
    pub fn get_group_mut(
        &mut self,
        handle: DataBlueprintGroupHandle,
    ) -> Option<&mut DataBlueprintGroup> {
        self.groups.get_mut(handle)
    }

    /// Returns object properties with the hierarchy applied.
    pub fn data_blueprints_projected(&self) -> &ObjectsProperties {
        &self.data_blueprints.projected
    }

    /// Returns mutable individual object properties, the hierarchy was not applied to this.
    pub fn data_blueprints_individual(&mut self) -> &mut ObjectsProperties {
        &mut self.data_blueprints.individual
    }

    /// Should be called on frame start.
    ///
    /// Propagates any data blueprint changes along the tree.
    pub fn on_frame_start(&mut self) {
        crate::profile_function!();

        // NOTE: We could do this projection only when the object properties changes
        // and/or when new object paths are added, but such memoization would add complexity.

        fn project_tree(
            tree: &mut DataBlueprintTree,
            parent_properties: &ObjectProps,
            group_handle: DataBlueprintGroupHandle,
        ) {
            let Some(group) = tree.groups.get_mut(group_handle) else {
                return; // should never happen.
            };

            let group_properties_projected =
                parent_properties.with_child(&group.properties_individual);
            group.properties_projected = group_properties_projected;

            for obj_path in &group.objects {
                let projected_properties = group_properties_projected
                    .with_child(&tree.data_blueprints.individual.get(obj_path));
                tree.data_blueprints
                    .projected
                    .set(obj_path.clone(), projected_properties);
            }

            let children = group.children.clone(); // TODO(andreas): How to avoid this clone?
            for child in &children {
                project_tree(tree, &group_properties_projected, *child);
            }
        }

        project_tree(self, &ObjectProps::default(), self.root_group_handle);
    }

    /// Adds a list of object paths to the tree, using grouping as dictated by their object path hierarchy.
    ///
    /// `base_path` indicates a path at which we short-circuit to the root group.
    ///
    /// Creates a group at *every* step of every path, unless a new group would only contain the object itself.
    pub fn insert_objects_according_to_hierarchy(
        &mut self,
        paths: &IntSet<ObjPath>,
        base_path: &ObjPath,
    ) {
        crate::profile_function!();

        let mut new_leaf_groups = Vec::new();

        for path in paths.iter() {
            // Is there already a group associated with this exact path? (maybe because a child was logged there earlier)
            // If so, we can simply move it under this existing group.
            let group_handle = if let Some(group_handle) = self.path_to_blueprint.get(path) {
                *group_handle
            } else if path == base_path {
                // Object might have directly been logged on the base_path. We map then to the root!
                self.root_group_handle
            } else {
                // Otherwise, create a new group which only contains this object and add the group to the hierarchy.
                let new_group = self.groups.insert(DataBlueprintGroup {
                    display_name: path_to_group_name(path),
                    ..Default::default()
                });
                self.add_group_to_hierarchy_recursively(new_group, path, base_path);
                new_leaf_groups.push(new_group);
                new_group
            };

            self.add_object_to_group(group_handle, path);
        }

        // If a leaf group contains only a single element, move that element to the parent and remove the leaf again.
        // (we can't do this as we iterate initially on `paths`, as we don't know if we're data on non-leaf paths until we touched all of them)
        for leaf_group_handle in new_leaf_groups {
            let Some(leaf_group) = self.groups.get_mut(leaf_group_handle) else {
                continue;
            };
            if !leaf_group.children.is_empty() || leaf_group.objects.len() != 1 {
                continue;
            }

            // Remove group.
            let single_object = leaf_group.objects.drain().next().unwrap();
            let parent_group_handle = leaf_group.parent;
            self.groups.remove(leaf_group_handle);

            // Add object to its parent and remove the now deleted child.
            let parent_group = self.groups.get_mut(parent_group_handle).unwrap();
            parent_group
                .children
                .retain(|child_group| *child_group != leaf_group_handle);
            parent_group.objects.insert(single_object.clone());
            self.path_to_blueprint
                .insert(single_object, parent_group_handle);
        }
    }

    fn add_group_to_hierarchy_recursively(
        &mut self,
        new_group: DataBlueprintGroupHandle,
        associated_path: &ObjPath,
        base_path: &ObjPath,
    ) {
        let Some(mut parent_path) = associated_path.parent() else {
            // Already the root, nothing to do.
            return;
        };

        // Short circuit to the root group at base_path.
        // If the object is outside of the base path we would walk up all the way to the root
        // That's ok but we want to stop one element short (since a space view can only show elements under a shared path)
        if &parent_path == base_path || parent_path.iter().count() == 1 {
            parent_path = ObjPath::root();
        }

        let parent_group = match self.path_to_blueprint.entry(parent_path.clone()) {
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

    /// Adds an objectpath to a group.
    ///
    /// If it was already associated with this group, nothing will happen.
    /// If it was already associated with a different group, it will move from there.
    pub fn add_object_to_group(&mut self, group_handle: DataBlueprintGroupHandle, path: &ObjPath) {
        if let Some(group) = self.groups.get_mut(group_handle) {
            if !group.objects.insert(path.clone()) {
                // If the object was already in here it won't be in another group previously.
                return;
            }
        } else {
            return;
        }

        if let Some(previous_group) = self.path_to_blueprint.insert(path.clone(), group_handle) {
            if previous_group != group_handle {
                if let Some(previous_group) = self.groups.get_mut(previous_group) {
                    previous_group.objects.retain(|obj| obj != path);
                }
            }
        }
    }
}

fn path_to_group_name(path: &ObjPath) -> String {
    path.iter().last().map_or(String::new(), |c| c.to_string())
}
