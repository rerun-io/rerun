use nohash_hasher::IntMap;
use re_arrow_store::LatestAtQuery;
use re_data_store::{
    log_db::EntityDb, query_latest_single, EntityPath, EntityPropertyMap, EntityTree,
};

use crate::misc::TimeControl;

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
    reference_from_entity_per_entity: IntMap<EntityPath, glam::Affine3A>,

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
    UnknownTransform,
}

impl std::fmt::Display for UnreachableTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownSpaceInfo =>
                "Can't determine transform because internal data structures are not in a valid state. Please file an issue on https://github.com/rerun-io/rerun/",
            Self::NestedPinholeCameras =>
                "Can't display entities under nested pinhole cameras.",
            Self::UnknownTransform =>
                "Can't display entities that are connected via an unknown transform to this space.",
            Self::InversePinholeCameraWithoutResolution =>
                "Can't display entities that would require inverting a pinhole camera without a specified resolution.",
        })
    }
}

fn transform_affine3(a: glam::Affine3A, b: glam::Affine3A) -> glam::Affine3A {
    glam::Affine3A {
        matrix3: a.matrix3.mul_mat3(&b.matrix3),
        translation: a.matrix3.mul_vec3a(b.translation) + a.translation,
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
            reference_from_entity_per_entity: Default::default(),
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
            entity_db,
            &query,
            entity_prop_map,
            glam::Affine3A::IDENTITY,
            false,
        );

        // Walk up from the reference to the highest reachable parent.
        let mut encountered_pinhole = false;
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
                entity_db,
                entity_prop_map,
                &query,
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    transforms.first_unreachable_parent =
                        Some((parent_tree.path.clone(), unreachable_reason));
                    break;
                }
                Ok(None) => {}
                Ok(Some(parent_from_child)) => {
                    reference_from_ancestor =
                        transform_affine3(reference_from_ancestor, parent_from_child.inverse());
                }
            }

            // (skip over everything at and under `current_tree` automatically)
            transforms.gather_descendants_transforms(
                parent_tree,
                entity_db,
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
        entity_db: &EntityDb,
        query: &LatestAtQuery,
        entity_properties: &EntityPropertyMap,
        reference_from_entity: glam::Affine3A,
        encountered_pinhole: bool,
    ) {
        match self
            .reference_from_entity_per_entity
            .entry(tree.path.clone())
        {
            std::collections::hash_map::Entry::Occupied(_) => {
                return;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(reference_from_entity);
            }
        }

        for child_tree in tree.children.values() {
            let mut encountered_pinhole = encountered_pinhole;
            let reference_from_child = match transform_at(
                &child_tree.path,
                entity_db,
                entity_properties,
                query,
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    self.unreachable_descendants
                        .push((child_tree.path.clone(), unreachable_reason));
                    continue;
                }
                Ok(None) => reference_from_entity,
                Ok(Some(child_from_parent)) => {
                    transform_affine3(reference_from_entity, child_from_parent)
                }
            };
            self.gather_descendants_transforms(
                child_tree,
                entity_db,
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
    pub fn reference_from_entity(&self, entity_path: &EntityPath) -> Option<macaw::Affine3A> {
        self.reference_from_entity_per_entity
            .get(entity_path)
            .cloned()
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
    entity_db: &EntityDb,
    entity_properties: &EntityPropertyMap,
    query: &LatestAtQuery,
    encountered_pinhole: &mut bool,
) -> Result<Option<glam::Affine3A>, UnreachableTransform> {
    if let Some(transform) = query_latest_single(entity_db, entity_path, query) {
        match transform {
            re_log_types::Transform::Rigid3(rigid) => Ok(Some(rigid.parent_from_child().into())),
            // If we're connected via 'unknown' it's not reachable
            re_log_types::Transform::Unknown => Err(UnreachableTransform::UnknownTransform),

            re_log_types::Transform::Pinhole(pinhole) => {
                if *encountered_pinhole {
                    Err(UnreachableTransform::NestedPinholeCameras)
                } else {
                    *encountered_pinhole = true;

                    // A pinhole camera means that we're looking at an image.
                    // Images are spanned in their local x/y space.
                    // Center it and move it along z, scaling the further we move.
                    let props = entity_properties.get(entity_path);
                    let distance = *props.pinhole_image_plane_distance.get();

                    let focal_length = pinhole.focal_length_in_pixels();
                    let focal_length = glam::vec2(focal_length.x(), focal_length.y());
                    let scale = distance / focal_length;
                    let translation = (-pinhole.principal_point() * scale).extend(distance);
                    let parent_from_child = glam::Affine3A::from_scale_rotation_translation(
                        // We want to preserve any depth that might be on the pinhole image.
                        // Use harmonic mean of x/y scale for those.
                        scale.extend(1.0 / (1.0 / scale.x + 1.0 / scale.y)),
                        glam::Quat::IDENTITY,
                        translation,
                    );

                    Ok(Some(parent_from_child))
                }
            }
        }
    } else {
        Ok(None)
    }
}
