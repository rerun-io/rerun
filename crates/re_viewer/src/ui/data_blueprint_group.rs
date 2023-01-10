use nohash_hasher::{IntMap, IntSet};
use re_data_store::ObjPath;
use slotmap::SlotMap;
use smallvec::{smallvec, SmallVec};

slotmap::new_key_type! { pub struct DataBlueprintGroupHandle; }

/// A grouping of several data blueprints.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintGroup {
    pub id: uuid::Uuid,

    pub name: String,
    /// Whether this is expanded in the ui.
    pub expanded: bool,

    // TODO(andreas): We should have the same properties as on data blueprints themselves, see https://github.com/rerun-io/rerun/issues/703
    //                  What to do about things that may or may not apply? Expand ObjectProps?
    //properties: ObjectProps,
    /// Parent of this blueprint group. Every data blueprint except the root has a parent.
    pub parent: DataBlueprintGroupHandle,

    pub children: SmallVec<[DataBlueprintGroupHandle; 1]>,
    pub objects: IntSet<ObjPath>,
}

/// Tree of all data blueprint groups for a single space view.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct DataBlueprintTree {
    /// All data blueprint groups.
    groups: SlotMap<DataBlueprintGroupHandle, DataBlueprintGroup>,

    /// Mapping from object paths to blueprints.
    ///
    /// Note that not every group may map directly to a path.
    /// But every path maps to a group it is in!
    path_to_blueprint: IntMap<ObjPath, DataBlueprintGroupHandle>,

    /// Root group, always exists as a placeholder
    root_group: DataBlueprintGroupHandle,
    // TODO: Requirements
    // * lookup in which group a given obj path is quickly and determine object properties from there
    // * insert object path into a fitting group, potentially creating new groups
    // * walk down the tree to render a ui containing all connected groups and objects
}

impl Default for DataBlueprintTree {
    fn default() -> Self {
        let mut groups = SlotMap::default();
        let root_group = groups.insert(DataBlueprintGroup {
            id: uuid::Uuid::new_v4(),
            name: String::new(),
            parent: slotmap::Key::null(),
            expanded: true,
            children: SmallVec::new(),
            objects: IntSet::default(),
        });

        let mut path_to_blueprint = IntMap::default();
        path_to_blueprint.insert(ObjPath::root(), root_group);

        Self {
            groups,
            path_to_blueprint,
            root_group,
        }
    }
}

impl DataBlueprintTree {
    pub fn root(&self) -> DataBlueprintGroupHandle {
        self.root_group
    }

    pub fn get_group(&self, handle: DataBlueprintGroupHandle) -> Option<&DataBlueprintGroup> {
        self.groups.get(handle)
    }

    pub fn get_group_mut(
        &mut self,
        handle: DataBlueprintGroupHandle,
    ) -> Option<&mut DataBlueprintGroup> {
        self.groups.get_mut(handle)
    }

    /// Adds a list of object paths to the tree, using grouping as dictated by their object path hierarchy.
    ///
    /// `base_path` indicates a path at which we short-circuit to the root group.
    ///
    /// Creates a group at *every* step of every path. Groups directly at the object path will be discarded if they only contain the object itself.
    /// It's up to the ui to not show groups with only a single object.
    pub fn insert_objects_according_to_hierarchy(
        &mut self,
        paths: &IntSet<ObjPath>,
        base_path: &ObjPath,
    ) {
        let mut new_leaf_groups = Vec::new();

        for path in paths.iter() {
            // Is there already a group associated with this exact path? (maybe because a child was logged there earlier)
            // If so, we can simply move it under this existing group.
            let group_handle = if let Some(group_handle) = self.path_to_blueprint.get(path) {
                *group_handle
            } else {
                // Otherwise, create a new group which only contains this object and add the group to the hierarchy.
                let new_group = self.groups.insert(DataBlueprintGroup {
                    id: uuid::Uuid::new_v4(),
                    name: path.to_string(), // TODO:
                    expanded: true,
                    children: SmallVec::new(),
                    objects: IntSet::default(),
                    parent: slotmap::Key::null(), // To be determined.
                });
                self.add_group_to_hierarchy_recursively(new_group, path, base_path);
                new_leaf_groups.push(new_group);
                new_group
            };

            // TODO: collapse if too many
            self.add_path_to_group(group_handle, path);
        }

        // If a leaf group contains only a single element, collapse it.
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

            // Add to its parent.
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
                    id: uuid::Uuid::new_v4(),
                    name: parent_path.to_string(),
                    expanded: true,
                    children: smallvec![new_group],
                    objects: IntSet::default(),
                    parent: slotmap::Key::null(), // To be determined.
                });
                vacant_mapping.insert(parent_group);
                self.add_group_to_hierarchy_recursively(parent_group, &parent_path, base_path);
                parent_group
            }
        };

        self.groups.get_mut(new_group).unwrap().parent = parent_group;
    }

    /// Adds a path to a group.
    ///
    /// If it was already associated with this group, nothing will happen.
    /// If it was already associated with a different group, it will move from there.
    fn add_path_to_group(&mut self, group_handle: DataBlueprintGroupHandle, path: &ObjPath) {
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
