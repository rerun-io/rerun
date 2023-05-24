use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_arrow_store::{LatestAtQuery, TimeInt, Timeline};
use re_data_store::{log_db::EntityDb, EntityPath, EntityTree};
use re_log_types::component_types::{DisconnectedSpace, Pinhole, Transform3D};

use super::UnreachableTransform;

/// Transform connecting two space paths.
#[derive(Clone, Debug)]
pub enum SpaceInfoConnection {
    Connected {
        transform3d: Option<Transform3D>,
        pinhole: Option<Pinhole>,
    },

    /// Explicitly disconnected via a [`DisconnectedSpace`] component.
    Disconnected,
}

/// Information about one "space".
///
/// This is gathered by analyzing the transform hierarchy of the entities.
/// ⚠️ Transforms used for this are latest known, i.e. the "right most location in the timeline" ⚠️
///
/// Expected to be recreated every frame (or whenever new data is available).
pub struct SpaceInfo {
    pub path: EntityPath,

    /// All paths in this space (including self and children connected by the identity transform).
    pub descendants_without_transform: IntSet<EntityPath>,

    /// Nearest ancestor to whom we are not connected via an identity transform.
    /// The transform is from parent to child, i.e. the *same* as in its [`Self::child_spaces`] array.
    parent: Option<(EntityPath, SpaceInfoConnection)>,

    /// Nearest descendants to whom we are not connected with an identity transform.
    pub child_spaces: BTreeMap<EntityPath, SpaceInfoConnection>,
}

impl SpaceInfo {
    pub fn new(path: EntityPath) -> Self {
        Self {
            path,
            descendants_without_transform: Default::default(),
            parent: Default::default(),
            child_spaces: Default::default(),
        }
    }

    /// Invokes visitor for `self` and all descendants that are reachable with a valid transform recursively.
    ///
    /// Keep in mind that transforms are the newest on the currently chosen timeline.
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

            for (child_path, connection) in &space_info.child_spaces {
                let Some(child_space) = space_info_collection.spaces.get(child_path) else {
                    re_log::warn_once!("Child space info {} not part of space info collection", child_path);
                    continue;
                };

                // don't allow nested pinhole
                let has_pinhole = matches!(
                    connection,
                    SpaceInfoConnection::Connected {
                        pinhole: Some(_),
                        ..
                    }
                );
                if encountered_pinhole && has_pinhole {
                    continue;
                }

                visit_descendants_with_reachable_transform_recursively(
                    child_space,
                    space_info_collection,
                    has_pinhole,
                    visitor,
                );
            }
        }

        visit_descendants_with_reachable_transform_recursively(self, spaces_info, false, visitor);
    }
}

/// Information about all spaces.
///
/// This is gathered by analyzing the transform hierarchy of the entities:
/// For every child of the root there is a space info, as well as the root itself.
/// Each of these we walk down recursively, every time a transform is encountered, we create another space info.
///
/// Expected to be recreated every frame (or whenever new data is available).
#[derive(Default)]
pub struct SpaceInfoCollection {
    spaces: BTreeMap<EntityPath, SpaceInfo>,
}

impl SpaceInfoCollection {
    /// Do a graph analysis of the transform hierarchy, and create cuts
    /// wherever we find a non-identity transform.
    pub fn new(entity_db: &EntityDb) -> Self {
        crate::profile_function!();

        fn add_children(
            entity_db: &EntityDb,
            spaces_info: &mut SpaceInfoCollection,
            parent_space: &mut SpaceInfo,
            tree: &EntityTree,
            query: &LatestAtQuery,
        ) {
            // Determine how the paths are connected.
            let store = &entity_db.data_store;
            let transform3d = store.query_latest_component::<Transform3D>(&tree.path, query);
            let pinhole = store.query_latest_component::<Pinhole>(&tree.path, query);

            let connection = if transform3d.is_some() || pinhole.is_some() {
                Some(SpaceInfoConnection::Connected {
                    transform3d,
                    pinhole,
                })
            } else if store
                .query_latest_component::<DisconnectedSpace>(&tree.path, query)
                .is_some()
            {
                Some(SpaceInfoConnection::Disconnected)
            } else {
                None
            };

            if let Some(connection) = connection {
                // A set transform - create a new space.
                parent_space
                    .child_spaces
                    .insert(tree.path.clone(), connection.clone());

                let mut child_space_info = SpaceInfo::new(tree.path.clone());
                child_space_info.parent = Some((parent_space.path.clone(), connection));
                child_space_info
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        entity_db,
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
                // no transform == implicit identity transform.
                parent_space
                    .descendants_without_transform
                    .insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(entity_db, spaces_info, parent_space, child_tree, query);
                }
            }
        }

        // Use "right most"/latest available data.
        let timeline = Timeline::log_time();
        let query_time = TimeInt::MAX;
        let query = LatestAtQuery::new(timeline, query_time);

        let mut spaces_info = Self::default();

        // Start at the root. The root is always part of the collection!
        if entity_db
            .data_store
            .query_latest_component::<Transform3D>(&EntityPath::root(), &query)
            .is_some()
        {
            re_log::warn_once!("The root entity has a 'transform' component! This will have no effect. Did you mean to apply the transform elsewhere?");
        }
        let mut root_space_info = SpaceInfo::new(EntityPath::root());
        add_children(
            entity_db,
            &mut spaces_info,
            &mut root_space_info,
            &entity_db.tree,
            &query,
        );
        spaces_info
            .spaces
            .insert(EntityPath::root(), root_space_info);

        spaces_info
    }

    pub fn get_first_parent_with_info(&self, path: &EntityPath) -> &SpaceInfo {
        let mut path = path.clone();
        loop {
            if let Some(space_info) = self.spaces.get(&path) {
                return space_info;
            }
            path = path.parent().expect(
                "The root path is part of SpaceInfoCollection, as such it's impossible to not have a space info parent!");
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &SpaceInfo> {
        self.spaces.values()
    }

    /// Answers if an entity path (`from`) is reachable via a transform from some reference space (at `to_reference`)
    ///
    /// For how, you need to check [`crate::misc::TransformCache`]!
    /// Note that in any individual frame, entities may or may not be reachable.
    pub fn is_reachable_by_transform(
        &self,
        from: &EntityPath,
        to_reference: &EntityPath,
    ) -> Result<(), UnreachableTransform> {
        crate::profile_function!();

        // Get closest space infos for the given entity paths.
        let mut from_space = self.get_first_parent_with_info(from);
        let mut to_reference_space = self.get_first_parent_with_info(to_reference);

        // Reachability is (mostly) commutative!
        // This means we can simply walk from both nodes up until we find a common ancestor!
        // If we haven't encountered any obstacles, we're fine!
        let mut encountered_pinhole = false;
        while from_space.path != to_reference_space.path {
            // Decide if we should walk up "from" or "to_reference"
            // If "from" is a descendant of "to_reference", we walk up "from"
            // Otherwise we walk up on "to_reference".
            //
            // If neither is a descendant of the other it doesn't matter which one we walk up, since we eventually going to hit common ancestor!
            let walk_up_from = from_space.path.is_descendant_of(&to_reference_space.path);

            let parent = if walk_up_from {
                &from_space.parent
            } else {
                &to_reference_space.parent
            };

            if let Some((parent_path, connection)) = parent {
                // Matches the connectedness requirements in `inverse_transform_at`/`transform_at` in `transform_cache.rs`
                match connection {
                    SpaceInfoConnection::Disconnected => {
                        Err(UnreachableTransform::DisconnectedSpace)
                    }
                    SpaceInfoConnection::Connected {
                        pinhole: Some(pinhole),
                        ..
                    } => {
                        if encountered_pinhole {
                            Err(UnreachableTransform::NestedPinholeCameras)
                        } else {
                            encountered_pinhole = true;
                            if pinhole.resolution.is_none() && !walk_up_from {
                                Err(UnreachableTransform::InversePinholeCameraWithoutResolution)
                            } else {
                                Ok(())
                            }
                        }
                    }
                    SpaceInfoConnection::Connected { .. } => Ok(()),
                }?;

                let Some(parent_space) = self.spaces.get(parent_path)
                else {
                    re_log::warn_once!("{} not part of space infos", parent_path);
                    return Err(UnreachableTransform::UnknownSpaceInfo);
                };

                if walk_up_from {
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
                return Err(UnreachableTransform::UnknownSpaceInfo);
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
