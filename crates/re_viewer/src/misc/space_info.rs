use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_data_store::{log_db::ObjDb, query_transform, ObjPath, ObjectTree, TimelineStore};
use re_log_types::{ObjectType, Transform, ViewCoordinates};

use super::TimeControl;

/// Information about one "space".
///
/// This is gathered by analyzing the transform hierarchy of the objects.
#[derive(Default)]
pub struct SpaceInfo {
    /// The latest known coordinate system for this space.
    pub coordinates: Option<ViewCoordinates>,

    /// All paths in this space (including self and children connected by the identity transform).
    pub descendants_without_transform: IntSet<ObjPath>,

    /// Nearest ancestor to whom we are not connected via an identity transform.
    #[allow(unused)]
    pub parent: Option<(ObjPath, Transform)>,

    /// Nearest descendants to whom we are not connected with an identity transform.
    pub child_spaces: BTreeMap<ObjPath, Transform>,
}

impl SpaceInfo {
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
                    if let Some(child_space) = spaces_info.spaces.get(child_path) {
                        gather_rigidly_transformed_children(child_space, spaces_info, objects);
                    }
                }
            }
        }

        let mut objects = IntSet::default();
        gather_rigidly_transformed_children(self, spaces_info, &mut objects);
        objects
    }
}

/// Information about all spaces.
///
/// This is gathered by analyzing the transform hierarchy of the objects:
/// For every child of the root there is a space info.
/// Each of these we walk down recursively, every time a transform is encountered, we create another space info.
#[derive(Default)]
pub struct SpacesInfo {
    pub spaces: BTreeMap<ObjPath, SpaceInfo>,
}

impl SpacesInfo {
    /// Do a graph analysis of the transform hierarchy, and create cuts
    /// wherever we find a non-identity transform.
    pub fn new(obj_db: &ObjDb, time_ctrl: &TimeControl) -> Self {
        crate::profile_function!();

        fn add_children(
            timeline_store: Option<&TimelineStore<i64>>,
            query_time: Option<i64>,
            spaces_info: &mut SpacesInfo,
            parent_space_path: &ObjPath,
            parent_space_info: &mut SpaceInfo,
            tree: &ObjectTree,
        ) {
            if let Some(transform) = query_transform(timeline_store, &tree.path, query_time) {
                // A set transform (likely non-identity) - create a new space.
                parent_space_info
                    .child_spaces
                    .insert(tree.path.clone(), transform.clone());

                let mut child_space_info = SpaceInfo {
                    parent: Some((parent_space_path.clone(), transform)),
                    ..Default::default()
                };
                child_space_info
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        query_time,
                        spaces_info,
                        &tree.path,
                        &mut child_space_info,
                        child_tree,
                    );
                }
                spaces_info
                    .spaces
                    .insert(tree.path.clone(), child_space_info);
            } else {
                // no transform == identity transform.
                parent_space_info
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        query_time,
                        spaces_info,
                        parent_space_path,
                        parent_space_info,
                        child_tree,
                    );
                }
            }
        }

        let timeline = time_ctrl.timeline();
        let query_time = time_ctrl.time().map(|time| time.floor().as_i64());
        let timeline_store = obj_db.store.get(timeline);

        let mut spaces_info = Self::default();

        for tree in obj_db.tree.children.values() {
            // Each root object is its own space (or should be)

            if query_transform(timeline_store, &tree.path, query_time).is_some() {
                re_log::warn_once!(
                    "Root object '{}' has a _transform - this is not allowed!",
                    tree.path
                );
            }

            let mut space_info = SpaceInfo::default();
            add_children(
                timeline_store,
                query_time,
                &mut spaces_info,
                &tree.path,
                &mut space_info,
                tree,
            );
            spaces_info.spaces.insert(tree.path.clone(), space_info);
        }

        // The ClassDescription objects apply to all spaces, collect every
        // object path with this type.
        let spaceless_objects = if let Some(timeline_store) = timeline_store {
            timeline_store
                .iter()
                .filter(|(path, _)| {
                    obj_db.types.get(path.obj_type_path()) == Some(&ObjectType::ClassDescription)
                })
                .map(|(path, _)| path.clone())
                .collect::<IntSet<ObjPath>>()
        } else {
            IntSet::<ObjPath>::default()
        };

        for (obj_path, space_info) in &mut spaces_info.spaces {
            space_info.coordinates = query_view_coordinates(obj_db, time_ctrl, obj_path);
            space_info
                .descendants_without_transform
                .extend(spaceless_objects.clone());
        }

        spaces_info
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
