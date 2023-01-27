use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_arrow_store::{LatestAtQuery, TimeInt, Timeline};
use re_data_store::{log_db::ObjDb, query_transform, ObjPath, ObjectTree};
use re_log_types::{Transform, ViewCoordinates};
use re_query::query_entity_with_primary;

use super::UnreachableTransform;

/// Information about one "space".
///
/// This is gathered by analyzing the transform hierarchy of the objects.
/// ⚠️ Transforms used for this are latest known, i.e. the "right most location in the timeline" ⚠️
///
/// Expected to be recreated every frame (or whenever new data is available).
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

    /// Invokes visitor for `self` and all descendents that are reachable with a valid transform recursively.
    ///
    /// Keep in mind that transforms are the newest on the currently choosen timeline.
    pub fn visit_descendants_with_reachable_transform(
        &self,
        spaces_info: &SpaceInfoCollection,
        visitor: &mut impl FnMut(&SpaceInfo),
    ) {
        fn visit_descendants_with_reachable_transform_recursively(
            space_info: &SpaceInfo,
            space_info_collection: &SpaceInfoCollection,
            encountered_pinhole: bool,
            visitor: &mut impl FnMut(&SpaceInfo),
        ) {
            visitor(space_info);

            for (child_path, transform) in &space_info.child_spaces {
                let Some(child_space) = space_info_collection.get(child_path) else {
                    re_log::warn_once!("Child space info {} not part of space info collection", child_path);
                    continue;
                };

                let is_pinhole = match transform {
                    Transform::Unknown => {
                        continue;
                    }
                    Transform::Rigid3(_) => false,
                    Transform::Pinhole(_) => {
                        // Don't allow nested pinhole
                        if encountered_pinhole {
                            continue;
                        }
                        true
                    }
                };
                visit_descendants_with_reachable_transform_recursively(
                    child_space,
                    space_info_collection,
                    is_pinhole,
                    visitor,
                );
            }
        }

        visit_descendants_with_reachable_transform_recursively(self, spaces_info, false, visitor);
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
///
/// Expected to be recreated every frame (or whenever new data is available).
#[derive(Default)]
pub struct SpaceInfoCollection {
    spaces: BTreeMap<ObjPath, SpaceInfo>,
}

impl SpaceInfoCollection {
    /// Do a graph analysis of the transform hierarchy, and create cuts
    /// wherever we find a non-identity transform.
    pub fn new(obj_db: &ObjDb) -> Self {
        crate::profile_function!();

        fn add_children(
            obj_db: &ObjDb,
            spaces_info: &mut SpaceInfoCollection,
            parent_space: &mut SpaceInfo,
            tree: &ObjectTree,
            query: &LatestAtQuery,
        ) {
            if let Some(transform) = query_transform(obj_db, &tree.path, query) {
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
                        spaces_info,
                        &mut child_space_info,
                        child_tree,
                        query,
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
                    add_children(obj_db, spaces_info, parent_space, child_tree, query);
                }
            }
        }

        // Use "right most"/latest available data.
        let timeline = Timeline::log_time();
        let query_time = TimeInt::MAX;
        let query = LatestAtQuery::new(timeline, query_time);

        let mut spaces_info = Self::default();

        for tree in obj_db.tree.children.values() {
            // Each root object is its own space (or should be)

            if query_transform(obj_db, &tree.path, &query).is_some() {
                re_log::warn_once!(
                    "Root object '{}' has a _transform - this is not allowed!",
                    tree.path
                );
            }

            let mut space_info = SpaceInfo::new(tree.path.clone());
            add_children(obj_db, &mut spaces_info, &mut space_info, tree, &query);
            spaces_info.spaces.insert(tree.path.clone(), space_info);
        }

        for (obj_path, space_info) in &mut spaces_info.spaces {
            space_info.coordinates = query_view_coordinates(obj_db, obj_path, &query);
        }

        spaces_info
    }

    pub fn get(&self, path: &ObjPath) -> Option<&SpaceInfo> {
        self.spaces.get(path)
    }

    pub fn get_first_parent_with_info(&self, path: &ObjPath) -> Option<&SpaceInfo> {
        let mut path = path.clone();
        while let Some(parent) = path.parent() {
            let space_info = self.get(&path);
            if space_info.is_some() {
                return space_info;
            }
            path = parent;
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &SpaceInfo> {
        self.spaces.values()
    }

    /// Answers if an object path (`from`) is reachable via a transform from some reference space (at `to_reference`)
    ///
    /// For how, you need to check [`crate::misc::TransformCache`]!
    /// Note that in any individual frame, objects may or may not be reachable.
    ///
    /// If `from` and `to_reference` are not under the same root branch, they are regarded as [`UnreachableTransform::Unconnected`]
    pub fn is_reachable_by_transform(
        &self,
        from: &ObjPath,
        to_reference: &ObjPath,
    ) -> Result<(), UnreachableTransform> {
        crate::profile_function!();

        // By convention we regard the global hierarchy as a forest - don't allow breaking out of the current tree.
        if from.iter().next() != to_reference.iter().next() {
            return Err(UnreachableTransform::Unconnected);
        }

        // Get closest space infos for the given object paths.
        let Some(mut from_space) = self.get_first_parent_with_info(from) else {
            re_log::warn_once!("{} not part of space infos", from);
            return Err(UnreachableTransform::Unconnected);
        };
        let Some(mut to_reference_space) = self.get_first_parent_with_info(to_reference) else {
            re_log::warn_once!("{} not part of space infos", to_reference);
            return Err(UnreachableTransform::Unconnected);
        };

        // If this is not true, the path we're querying, `from`, is outside of the tree the reference node.
        // Note that this means that all transforms on the way are inversed!
        let from_is_child_of_reference = from.is_descendant_of(to_reference);

        // Reachability is (mostly) commutative!
        // This means we can simply walk from the lower node to the parent until we're on the same node
        // If we haven't encountered any obstacles, we're fine!
        let mut encountered_pinhole = false;
        while from_space.path != to_reference_space.path {
            let parent = if from_is_child_of_reference {
                &from_space.parent
            } else {
                &to_reference_space.parent
            };

            if let Some((parent_path, transform)) = parent {
                // Matches the connectedness requirements in `inverse_transform_at`/`transform_at` in `transform_cache.rs`
                match transform {
                    Transform::Unknown => Err(UnreachableTransform::UnknownTransform),
                    Transform::Rigid3(_) => Ok(()),
                    Transform::Pinhole(pinhole) => {
                        if encountered_pinhole {
                            Err(UnreachableTransform::NestedPinholeCameras)
                        } else {
                            encountered_pinhole = true;
                            if pinhole.resolution.is_none() && !from_is_child_of_reference {
                                Err(UnreachableTransform::InversePinholeCameraWithoutResolution)
                            } else {
                                Ok(())
                            }
                        }
                    }
                }?;

                let Some(parent_space) = self.get(parent_path) else {
                    re_log::warn_once!("{} not part of space infos", parent_path);
                    return Err(UnreachableTransform::Unconnected);
                };

                if from_is_child_of_reference {
                    from_space = parent_space;
                } else {
                    to_reference_space = parent_space;
                };
            } else {
                re_log::warn_once!(
                    "No space info connection between {} and {}",
                    from,
                    to_reference
                );
                return Err(UnreachableTransform::Unconnected);
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------

pub fn query_view_coordinates(
    obj_db: &ObjDb,
    ent_path: &ObjPath,
    query: &LatestAtQuery,
) -> Option<re_log_types::ViewCoordinates> {
    let arrow_store = &obj_db.arrow_store;

    let entity_view =
        query_entity_with_primary::<ViewCoordinates>(arrow_store, query, ent_path, &[]).ok()?;

    let mut iter = entity_view.iter_primary().ok()?;

    let view_coords = iter.next()?;

    if iter.next().is_some() {
        re_log::warn_once!("Unexpected batch for ViewCoordinates at: {}", ent_path);
    }

    view_coords
}
