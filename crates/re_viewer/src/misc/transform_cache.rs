use nohash_hasher::IntMap;
use re_arrow_store::LatestAtQuery;
use re_data_store::{log_db::EntityDb, EntityPath, EntityPropertyMap, EntityTree};
use re_log_types::component_types::{DisconnectedSpace, Pinhole, Transform3D};
use re_log_types::EntityPathHash;
use re_viewer_context::TimeControl;

#[derive(Clone)]
struct TransformInfo {
    /// The transform from the entity to the reference space.
    pub reference_from_entity: glam::Affine3A,

    /// The pinhole camera ancestor of this entity if any.
    ///
    /// None indicates that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the space view.
    pub parent_pinhole: Option<EntityPathHash>,
}

/// Provides transforms from an entity to a chosen reference space for all elements in the scene
/// for the currently selected time & timeline.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
///
/// Should be recomputed every frame.
#[derive(Clone)]
pub struct TransformCache {
    /// All transforms provided are relative to this reference path.
    reference_path: EntityPath,

    /// All reachable entities.
    transform_per_entity: IntMap<EntityPath, TransformInfo>,

    /// All unreachable descendant paths of `reference_path`.
    unreachable_descendants: Vec<(EntityPath, UnreachableTransform)>,

    /// The first parent of reference_path that is no longer reachable.
    first_unreachable_parent: Option<(EntityPath, UnreachableTransform)>,
}

#[derive(Clone, Copy)]
pub enum UnreachableTransform {
    /// [`super::space_info::SpaceInfoCollection`] is outdated and can't find a corresponding space info for the given path.
    ///
    /// If at all, this should only happen for a single frame until space infos are rebuilt.
    UnknownSpaceInfo,

    /// More than one pinhole camera between this and the reference space.
    NestedPinholeCameras,

    /// Exiting out of a space with a pinhole camera that doesn't have a resolution is not supported.
    InversePinholeCameraWithoutResolution,

    /// Unknown transform between this and the reference space.
    DisconnectedSpace,
}

impl std::fmt::Display for UnreachableTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownSpaceInfo =>
                "Can't determine transform because internal data structures are not in a valid state. Please file an issue on https://github.com/rerun-io/rerun/",
            Self::NestedPinholeCameras =>
                "Can't display entities under nested pinhole cameras.",
            Self::DisconnectedSpace =>
                "Can't display entities that are in an explicitly disconnected space.",
            Self::InversePinholeCameraWithoutResolution =>
                "Can't display entities that would require inverting a pinhole camera without a specified resolution.",
        })
    }
}

impl TransformCache {
    /// Determines transforms for all entities relative to a space path which serves as the "reference".
    /// I.e. the resulting transforms are "reference from scene"
    ///
    /// This means that the entities in `reference_space` get the identity transform and all other
    /// entities are transformed relative to it.
    pub fn determine_transforms(
        entity_db: &EntityDb,
        time_ctrl: &TimeControl,
        space_path: &EntityPath,
        entity_prop_map: &EntityPropertyMap,
    ) -> Self {
        crate::profile_function!();

        let mut transforms = TransformCache {
            reference_path: space_path.clone(),
            transform_per_entity: Default::default(),
            unreachable_descendants: Default::default(),
            first_unreachable_parent: None,
        };

        // Find the entity path tree for the root.
        let Some(mut current_tree) = &entity_db.tree.subtree(space_path) else {
            // It seems the space path is not part of the object tree!
            // This happens frequently when the viewer remembers space views from a previous run that weren't shown yet.
            // Naturally, in this case we don't have any transforms yet.
            return transforms;
        };

        let query = time_ctrl.current_query();

        // Child transforms of this space
        transforms.gather_descendants_transforms(
            current_tree,
            &entity_db.data_store,
            &query,
            entity_prop_map,
            glam::Affine3A::IDENTITY,
            None, // Ignore potential pinhole camera at the root of the space view, since it regarded as being "above" this root.
        );

        // Walk up from the reference to the highest reachable parent.
        let mut encountered_pinhole = None;
        let mut reference_from_ancestor = glam::Affine3A::IDENTITY;
        while let Some(parent_path) = current_tree.path.parent() {
            let Some(parent_tree) = &entity_db.tree.subtree(&parent_path) else {
                // Unlike not having the space path in the hierarchy, this should be impossible.
                re_log::error_once!(
                    "Path {} is not part of the global Entity tree whereas its child {} is",
                    parent_path, space_path
                );
                return transforms;
            };

            // Note that the transform at the reference is the first that needs to be inverted to "break out" of its hierarchy.
            // Generally, the transform _at_ a node isn't relevant to it's children, but only to get to its parent in turn!
            match transform_at(
                &current_tree.path,
                &entity_db.data_store,
                &query,
                // TODO(#1988): See comment in transform_at. This is a workaround for precision issues
                // and the fact that there is no meaningful image plane distance for 3D->2D views.
                |_| 500.0,
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    transforms.first_unreachable_parent =
                        Some((parent_tree.path.clone(), unreachable_reason));
                    break;
                }
                Ok(None) => {}
                Ok(Some(parent_from_child)) => {
                    reference_from_ancestor = reference_from_ancestor * parent_from_child.inverse();
                }
            }

            // (skip over everything at and under `current_tree` automatically)
            transforms.gather_descendants_transforms(
                parent_tree,
                &entity_db.data_store,
                &query,
                entity_prop_map,
                reference_from_ancestor,
                encountered_pinhole,
            );

            current_tree = parent_tree;
        }

        transforms
    }

    fn gather_descendants_transforms(
        &mut self,
        tree: &EntityTree,
        data_store: &re_arrow_store::DataStore,
        query: &LatestAtQuery,
        entity_properties: &EntityPropertyMap,
        reference_from_entity: glam::Affine3A,
        encountered_pinhole: Option<EntityPathHash>,
    ) {
        match self.transform_per_entity.entry(tree.path.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(TransformInfo {
                    reference_from_entity,
                    parent_pinhole: encountered_pinhole,
                });
            }
        }

        for child_tree in tree.children.values() {
            let mut encountered_pinhole = encountered_pinhole;
            let reference_from_child = match transform_at(
                &child_tree.path,
                data_store,
                query,
                |p| *entity_properties.get(p).pinhole_image_plane_distance.get(),
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    self.unreachable_descendants
                        .push((child_tree.path.clone(), unreachable_reason));
                    continue;
                }
                Ok(None) => reference_from_entity,
                Ok(Some(child_from_parent)) => reference_from_entity * child_from_parent,
            };
            self.gather_descendants_transforms(
                child_tree,
                data_store,
                query,
                entity_properties,
                reference_from_child,
                encountered_pinhole,
            );
        }
    }

    pub fn reference_path(&self) -> &EntityPath {
        &self.reference_path
    }

    /// Retrieves the transform of on entity from its local system to the space of the reference.
    ///
    /// Returns None if the path is not reachable.
    pub fn reference_from_entity(&self, ent_path: &EntityPath) -> Option<macaw::Affine3A> {
        self.transform_per_entity
            .get(ent_path)
            .map(|i| i.reference_from_entity)
    }

    /// Retrieves the ancestor (or self) pinhole under which this entity sits.
    ///
    /// None indicates either that the entity does not exist in this hierarchy or that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the space view.
    pub fn parent_pinhole(&self, ent_path: &EntityPath) -> Option<EntityPathHash> {
        self.transform_per_entity
            .get(ent_path)
            .and_then(|i| i.parent_pinhole)
    }

    // This method isn't currently implemented, but we might need it in the future.
    // All the necessary data on why a subtree isn't reachable is already stored.
    //
    // Returns why (if actually) a path isn't reachable.
    // pub fn unreachable_reason(&self, _entity_path: &EntityPath) -> Option<UnreachableTransformReason> {
    //     None
    // }
}

fn transform_at(
    entity_path: &EntityPath,
    store: &re_arrow_store::DataStore,
    query: &LatestAtQuery,
    pinhole_image_plane_distance: impl Fn(&EntityPath) -> f32,
    encountered_pinhole: &mut Option<EntityPathHash>,
) -> Result<Option<glam::Affine3A>, UnreachableTransform> {
    let pinhole = store.query_latest_component::<Pinhole>(entity_path, query);
    if pinhole.is_some() {
        if encountered_pinhole.is_some() {
            return Err(UnreachableTransform::NestedPinholeCameras);
        } else {
            *encountered_pinhole = Some(entity_path.hash());
        }
    }

    let transform3d = store
        .query_latest_component::<Transform3D>(entity_path, query)
        .map(|transform| transform.to_parent_from_child_transform());

    let pinhole = pinhole.map(|pinhole| {
        // A pinhole camera means that we're looking at some 2D image plane from a single point (the pinhole).
        // Center the image plane and move it along z, scaling the further the image plane is.
        let distance = pinhole_image_plane_distance(entity_path);

        let focal_length = pinhole.focal_length_in_pixels();
        let focal_length = glam::vec2(focal_length.x(), focal_length.y());
        let scale = distance / focal_length;
        let translation = (-pinhole.principal_point() * scale).extend(distance);

        glam::Affine3A::from_translation(translation)
        // We want to preserve any depth that might be on the pinhole image.
        // Use harmonic mean of x/y scale for those.
        * glam::Affine3A::from_scale(
            scale.extend(2.0 / (1.0 / scale.x + 1.0 / scale.y)),
        )

        // Above calculation is nice for a certain kind of visualizing a projected image plane,
        // but the image plane distance is arbitrary and there might be other, better visualizations!

        // TODO(#1988):
        // As such we don't ever want to invert this matrix!
        // However, currently our 2D views require do to exactly that since we're forced to
        // build a relationship between the 2D plane and the 3D world, when actually the 2D plane
        // should have infinite depth!
        // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
        // See also `ui_2d.rs#setup_target_config`
    });

    // If there is any other transform, we ignore `DisconnectedSpace`.
    if transform3d.is_some() || pinhole.is_some() {
        Ok(Some(
            transform3d.unwrap_or(glam::Affine3A::IDENTITY)
                * pinhole.unwrap_or(glam::Affine3A::IDENTITY),
        ))
    } else if store
        .query_latest_component::<DisconnectedSpace>(entity_path, query)
        .is_some()
    {
        Err(UnreachableTransform::DisconnectedSpace)
    } else {
        Ok(None)
    }
}
