use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_arrow_store::Timeline;
use re_data_store::{log_db::ObjDb, query_transform, ObjPath, ObjectTree};
use re_log_types::{Transform, ViewCoordinates};

use super::TimeControl;

/// Information about one "space".
///
/// This is gathered by analyzing the transform hierarchy of the objects.
pub struct SpaceInfo {
    pub path: ObjPath,

    /// The latest known coordinate system for this space.
    pub coordinates: Option<ViewCoordinates>,

    /// All paths in this space (including self and children connected by the identity transform).
    pub descendants_without_transform: IntSet<ObjPath>,

    /// Nearest ancestor to whom we are not connected via an identity transform.
    /// The transform is from parent to child, i.e. the *same* as in its [`Self::child_spaces`] array.
    parent: Option<(ObjPath, Transform)>,

    /// Nearest descendants to whom we are not connected with an identity transform.
    pub child_spaces: BTreeMap<ObjPath, Transform>,
}

impl SpaceInfo {
    pub fn new(path: ObjPath) -> Self {
        Self {
            path,
            coordinates: Default::default(),
            descendants_without_transform: Default::default(),
            parent: Default::default(),
            child_spaces: Default::default(),
        }
    }

    /// Invokes visitor for `self` and all descendents recursively.
    pub fn visit_descendants(
        &self,
        spaces_info: &SpacesInfo,
        visitor: &mut impl FnMut(&SpaceInfo),
    ) {
        visitor(self);
        for child_path in self.child_spaces.keys() {
            if let Some(child_space) = spaces_info.get(child_path) {
                child_space.visit_descendants(spaces_info, visitor);
            }
        }
    }

    /// Invokes visitor for `self` and all connected nodes that are not descendants.
    ///
    /// I.e. all parents and their children in turn, except the children of `self`.
    /// In other words, everything that [`Self::visit_descendants`] doesn't visit plus `self`.
    pub fn visit_non_descendants(
        &self,
        spaces_info: &SpacesInfo,
        visitor: &mut impl FnMut(&SpaceInfo),
    ) {
        visitor(self);

        if let Some((parent_space, _)) = &self.parent(spaces_info) {
            for sibling_path in parent_space.child_spaces.keys() {
                if *sibling_path == self.path {
                    continue;
                }
                if let Some(child_space) = spaces_info.get(sibling_path) {
                    child_space.visit_descendants(spaces_info, visitor);
                }

                parent_space.visit_non_descendants(spaces_info, visitor);
            }
        }
    }

    /// Recursively gather all descendants that have no or only a rigid transform.
    pub fn descendants_with_rigid_or_no_transform(
        &self,
        spaces_info: &SpacesInfo,
    ) -> IntSet<ObjPath> {
        fn gather_rigidly_transformed_children(
            space: &SpaceInfo,
            spaces_info: &SpacesInfo,
            objects: &mut IntSet<ObjPath>,
        ) {
            objects.extend(space.descendants_without_transform.iter().cloned());

            for (child_path, transform) in &space.child_spaces {
                if let re_log_types::Transform::Rigid3(_) = transform {
                    if let Some(child_space) = spaces_info.get(child_path) {
                        gather_rigidly_transformed_children(child_space, spaces_info, objects);
                    }
                }
            }
        }

        let mut objects = IntSet::default();
        gather_rigidly_transformed_children(self, spaces_info, &mut objects);
        objects
    }

    pub fn parent<'a>(&self, spaces_info: &'a SpacesInfo) -> Option<(&'a SpaceInfo, &Transform)> {
        self.parent.as_ref().and_then(|(parent_path, transform)| {
            spaces_info.get(parent_path).map(|space| (space, transform))
        })
    }

    pub fn parent_transform(&self) -> Option<&Transform> {
        self.parent.as_ref().map(|(_, transform)| transform)
    }
}

/// Information about all spaces.
///
/// This is gathered by analyzing the transform hierarchy of the objects:
/// For every child of the root there is a space info.
/// Each of these we walk down recursively, every time a transform is encountered, we create another space info.
#[derive(Default)]
pub struct SpacesInfo {
    spaces: BTreeMap<ObjPath, SpaceInfo>,
}

impl SpacesInfo {
    /// Do a graph analysis of the transform hierarchy, and create cuts
    /// wherever we find a non-identity transform.
    pub fn new(obj_db: &ObjDb, time_ctrl: &TimeControl) -> Self {
        crate::profile_function!();

        fn add_children(
            obj_db: &ObjDb,
            timeline: &Timeline,
            query_time: Option<i64>,
            spaces_info: &mut SpacesInfo,
            parent_space: &mut SpaceInfo,
            tree: &ObjectTree,
        ) {
            if let Some(transform) = query_transform(obj_db, timeline, &tree.path, query_time) {
                // A set transform (likely non-identity) - create a new space.
                parent_space
                    .child_spaces
                    .insert(tree.path.clone(), transform.clone());

                let mut child_space_info = SpaceInfo::new(tree.path.clone());
                child_space_info.parent = Some((parent_space.path.clone(), transform));
                child_space_info
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        obj_db,
                        timeline,
                        query_time,
                        spaces_info,
                        &mut child_space_info,
                        child_tree,
                    );
                }
                spaces_info
                    .spaces
                    .insert(tree.path.clone(), child_space_info);
            } else {
                // no transform == identity transform.
                parent_space
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        obj_db,
                        timeline,
                        query_time,
                        spaces_info,
                        parent_space,
                        child_tree,
                    );
                }
            }
        }

        let timeline = time_ctrl.timeline();
        let query_time = time_ctrl.time().map(|time| time.floor().as_i64());

        let mut spaces_info = Self::default();

        for tree in obj_db.tree.children.values() {
            // Each root object is its own space (or should be)

            if query_transform(obj_db, timeline, &tree.path, query_time).is_some() {
                re_log::warn_once!(
                    "Root object '{}' has a _transform - this is not allowed!",
                    tree.path
                );
            }

            let mut space_info = SpaceInfo::new(tree.path.clone());
            add_children(
                obj_db,
                timeline,
                query_time,
                &mut spaces_info,
                &mut space_info,
                tree,
            );
            spaces_info.spaces.insert(tree.path.clone(), space_info);
        }

        for (obj_path, space_info) in &mut spaces_info.spaces {
            space_info.coordinates = query_view_coordinates(obj_db, time_ctrl, obj_path);
        }

        spaces_info
    }

    pub fn get(&self, path: &ObjPath) -> Option<&SpaceInfo> {
        self.spaces.get(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SpaceInfo> {
        self.spaces.values()
    }
}

// ----------------------------------------------------------------------------

/// Get the latest value of the `_view_coordinates` meta-field of the given object.
fn query_view_coordinates(
    obj_db: &ObjDb,
    time_ctrl: &TimeControl,
    obj_path: &ObjPath,
) -> Option<re_log_types::ViewCoordinates> {
    let query_time = time_ctrl.time()?;
    let timeline = time_ctrl.timeline();

    let store = obj_db.store.get(timeline)?;

    let field_store = store
        .get(obj_path)?
        .get(&re_data_store::FieldName::from("_view_coordinates"))?;

    // `_view_coordinates` is only allowed to be stored in a mono-field.
    let mono_field_store = field_store
        .get_mono::<re_log_types::ViewCoordinates>()
        .ok()?;

    mono_field_store
        .latest_at(&query_time.floor().as_i64())
        .map(|(_time, _msg_id, system)| *system)
}
