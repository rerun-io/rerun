use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_data_store::{log_db::ObjDb, FieldName, ObjPath, ObjectTree, TimelineStore};
use re_log_types::{Transform, ViewCoordinates};

use super::TimeControl;

/// Information about one "space".
///
/// This is gathered by analyzing the transform hierarchy of the objects.
#[derive(Default)]
pub(crate) struct SpaceInfo {
    /// The latest known coordinate system for this space.
    pub coordinates: Option<ViewCoordinates>,

    /// All paths in this space (including self and children connected by the identity transform).
    pub objects: IntSet<ObjPath>,

    /// Nearest ancestor to whom we are not connected via an identity transform.
    #[allow(unused)] // TODO(emilk): support projecting parent space(s) into this space
    pub parent: Option<(ObjPath, Transform)>,

    /// Nearest descendants to whom we are not connected with an identity transform.
    pub child_spaces: BTreeMap<ObjPath, Transform>,
}

/// Information about all spaces.
///
/// This is gathered by analyzing the transform hierarchy of the objects.
#[derive(Default)]
pub(crate) struct SpacesInfo {
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
            spaceless_objects: &mut IntSet<ObjPath>,
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
                child_space_info.objects.insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        query_time,
                        spaces_info,
                        &tree.path,
                        &mut child_space_info,
                        child_tree,
                        spaceless_objects,
                    );
                }
                spaces_info
                    .spaces
                    .insert(tree.path.clone(), child_space_info);
            } else {
                // no transform == identity transform.
                parent_space_info.objects.insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        query_time,
                        spaces_info,
                        parent_space_path,
                        parent_space_info,
                        child_tree,
                        spaceless_objects,
                    );
                }
            }

            // TODO(jleibs) -- This is a terrible hack, avert your eyes.
            // We want every SpaceView to have access to class_descriptions
            // We don't have type information at this point, but class_descriptions are (currently)
            // the only object with "id" field. We use that to store it off in related_objects
            // as we traverse the tree so we can then append it to the object-sets of all SpaceInfos
            // once we're done with our traversal.
            if tree.fields.contains_key(&FieldName::from("id")) {
                spaceless_objects.insert(tree.path.clone());
            }
        }

        let timeline = time_ctrl.timeline();
        let query_time = time_ctrl.time().map(|time| time.floor().as_i64());
        let timeline_store = obj_db.store.get(timeline);

        let mut spaces_info = Self::default();

        // Add_children can insert paths to spaceless objects which will then
        // be added to all spaces when we are done with the traversal.
        let mut spaceless_objects = IntSet::<ObjPath>::default();

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
                &mut spaceless_objects,
            );

            // Only add the space_info to spaces if it contains a non-spaceless object
            if !space_info.objects.is_subset(&spaceless_objects) {
                spaces_info.spaces.insert(tree.path.clone(), space_info);
            }
        }

        for (obj_path, space_info) in &mut spaces_info.spaces {
            space_info.coordinates = query_view_coordinates(obj_db, time_ctrl, obj_path);
            space_info.objects.extend(spaceless_objects.clone());
        }

        spaces_info
    }
}

// ----------------------------------------------------------------------------

/// Get the latest value of the `_transform` meta-field of the given object.
fn query_transform(
    store: Option<&TimelineStore<i64>>,
    obj_path: &ObjPath,
    query_time: Option<i64>,
) -> Option<re_log_types::Transform> {
    let field_store = store?.get(obj_path)?.get(&FieldName::from("_transform"))?;
    // `_transform` is only allowed to be stored in a mono-field.
    let mono_field_store = field_store.get_mono::<re_log_types::Transform>().ok()?;

    // There is a transform, at least at _some_ time.
    // Is there a transform _now_?
    let latest = query_time
        .and_then(|query_time| mono_field_store.latest_at(&query_time))
        .map(|(_, (_, transform))| transform.clone());

    // If not, return an unknown transform to indicate that there is
    // still a space-split.
    Some(latest.unwrap_or(Transform::Unknown))
}

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
        .map(|(_time, (_msg_id, system))| *system)
}
