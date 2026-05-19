//! Utilities for querying out transform types.

use std::sync::OnceLock;

use glam::DAffine3;
use itertools::Either;
use re_chunk_store::{ChunkShared, LatestAtQuery, MissingChunkReporter};
use re_entity_db::EntityDb;
use re_entity_db::external::re_query::StorageEngineReadGuard;
use re_log_types::EntityPath;
use re_sdk_types::archetypes::{self, InstancePoses3D};
use re_sdk_types::external::arrow::array::Array as _;
use re_sdk_types::{ChunkId, ComponentIdentifier, RowId, TransformFrameIdHash, components};

use crate::convert;
use crate::transform_resolution_cache::{
    ParentFromChildTransform, ResolvedPinholeProjectionCached,
};

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("invalid transform for component `{component}` on entity `{entity_path}`")]
    InvalidTransform {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error("missing transform on entity `{entity_path}`")]
    MissingTransform { entity_path: EntityPath },

    #[error(
        "Ignoring transform due to empty parent frame name for component `{component}` on entity `{entity_path}`."
    )]
    EmptyParentFrame {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error(
        "Ignoring transform at root entity /. Transforms require either a parent entity that can be used as implicit frame, or the parent_frame field to be set."
    )]
    ImplicitRootParentFrame,
}

fn lookup_chunk_row<'a>(
    storage_engine: &'a StorageEngineReadGuard<'a>,
    missing_chunk_reporter: &MissingChunkReporter,
    chunk_id: ChunkId,
    row_id: RowId,
) -> Option<(&'a ChunkShared, usize)> {
    let store = storage_engine.store();
    let Some(chunk) = store.physical_chunk(&chunk_id) else {
        missing_chunk_reporter.report_missing_chunk();
        return None;
    };

    let index = if chunk.is_sorted() {
        chunk.row_ids_slice().binary_search(&row_id).ok()?
    } else {
        chunk.row_ids_slice().iter().position(|r| *r == row_id)?
    };

    Some((chunk, index))
}

pub fn atomic_component_set_for_tree_transforms() -> &'static [ComponentIdentifier] {
    static ATOMIC_COMPONENTS_FOR_TREE_TRANSFORMS: OnceLock<[ComponentIdentifier; 8]> =
        OnceLock::new();

    ATOMIC_COMPONENTS_FOR_TREE_TRANSFORMS.get_or_init(|| {
        [
            // Topology
            archetypes::Transform3D::descriptor_parent_frame().component,
            archetypes::Transform3D::descriptor_child_frame().component,
            archetypes::Transform3D::descriptor_relation().component,
            // Geometry
            archetypes::Transform3D::descriptor_translation().component,
            archetypes::Transform3D::descriptor_rotation_axis_angle().component,
            archetypes::Transform3D::descriptor_quaternion().component,
            archetypes::Transform3D::descriptor_scale().component,
            archetypes::Transform3D::descriptor_mat3x3().component,
        ]
    })
}

pub fn atomic_component_set_for_instance_poses() -> &'static [ComponentIdentifier] {
    static ATOMIC_COMPONENTS_FOR_INSTANCE_POSES: OnceLock<[ComponentIdentifier; 5]> =
        OnceLock::new();

    ATOMIC_COMPONENTS_FOR_INSTANCE_POSES.get_or_init(|| {
        [
            InstancePoses3D::descriptor_translations().component,
            InstancePoses3D::descriptor_rotation_axis_angles().component,
            InstancePoses3D::descriptor_quaternions().component,
            InstancePoses3D::descriptor_scales().component,
            InstancePoses3D::descriptor_mat3x3().component,
        ]
    })
}

pub fn atomic_component_set_for_pinhole_projection() -> &'static [ComponentIdentifier] {
    static ATOMIC_COMPONENTS_FOR_PINHOLE_PROJECTION: OnceLock<[ComponentIdentifier; 4]> =
        OnceLock::new();

    ATOMIC_COMPONENTS_FOR_PINHOLE_PROJECTION.get_or_init(|| {
        [
            // Topology
            archetypes::Pinhole::descriptor_parent_frame().component,
            archetypes::Pinhole::descriptor_child_frame().component,
            // Geometry
            archetypes::Pinhole::descriptor_image_from_camera().component,
            archetypes::Pinhole::descriptor_resolution().component,
        ]
    })
}

/// Queries & processes all components that are part of a transform, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns `None`.
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
pub fn query_and_resolve_tree_transform_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    chunk_id: ChunkId,
    row_id: RowId,
) -> Result<ParentFromChildTransform, TransformError> {
    // Topology
    let identifier_parent_frame = archetypes::Transform3D::descriptor_parent_frame().component;
    let identifier_relation = archetypes::Transform3D::descriptor_relation().component;

    // Geometry
    let identifier_translations = archetypes::Transform3D::descriptor_translation().component;
    let identifier_rotation_axis_angles =
        archetypes::Transform3D::descriptor_rotation_axis_angle().component;
    let identifier_quaternions = archetypes::Transform3D::descriptor_quaternion().component;
    let identifier_scales = archetypes::Transform3D::descriptor_scale().component;
    let identifier_mat3x3 = archetypes::Transform3D::descriptor_mat3x3().component;

    // We're querying for transactional/atomic transform state:
    // If any of the topology or geometry components change, we reset the entire transform.
    //
    // This means we don't have to do latest-at for individual components.
    // Instead, we're looking for the last change and then get everything with that row id.
    //
    // We bipass the query cache here:
    // * we're already doing special caching anyways
    // * we don't want to merge over row-ids *at all* since our query handling here is a little bit different. The query cache is geared towards "regular Rerun semantics"
    // * we already handled `Clear`/`ClearRecursive` upon pre-population of our cache entries (we know when a clear occurs on this entity!)
    let storage_engine = entity_db.storage_engine();
    let Some((chunk, row_index)) =
        lookup_chunk_row(&storage_engine, missing_chunk_reporter, chunk_id, row_id)
    else {
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };

    // TODO(andreas): silently ignores deserialization error right now.

    let parent = get_parent_frame(chunk, row_index, entity_path, identifier_parent_frame)?;

    #[expect(clippy::useless_let_if_seq)]
    let mut transform = DAffine3::IDENTITY;

    // The order of the components here is important.
    if let Some(translation) = chunk
        .component_mono::<components::Translation3D>(identifier_translations, row_index)
        .and_then(|v| v.ok())
    {
        transform = convert::translation_3d_to_daffine3(translation);
    }
    if let Some(axis_angle) = chunk
        .component_mono::<components::RotationAxisAngle>(identifier_rotation_axis_angles, row_index)
        .and_then(|v| v.ok())
    {
        let axis_angle = convert::rotation_axis_angle_to_daffine3(axis_angle).map_err(|_err| {
            TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_rotation_axis_angles,
            }
        })?;
        transform *= axis_angle;
    }
    if let Some(quaternion) = chunk
        .component_mono::<components::RotationQuat>(identifier_quaternions, row_index)
        .and_then(|v| v.ok())
    {
        let quaternion = convert::rotation_quat_to_daffine3(quaternion).map_err(|_err| {
            TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_quaternions,
            }
        })?;
        transform *= quaternion;
    }
    if let Some(scale) = chunk
        .component_mono::<components::Scale3D>(identifier_scales, row_index)
        .and_then(|v| v.ok())
    {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            return Err(TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_scales,
            });
        }
        transform *= convert::scale_3d_to_daffine3(scale);
    }
    if let Some(mat3x3) = chunk
        .component_mono::<components::TransformMat3x3>(identifier_mat3x3, row_index)
        .and_then(|v| v.ok())
    {
        let affine_transform = convert::transform_mat3x3_to_daffine3(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            return Err(TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_mat3x3,
            });
        }
        transform *= affine_transform;
    }

    if chunk
        .component_mono::<components::TransformRelation>(identifier_relation, row_index)
        .and_then(|v| v.ok())
        == Some(components::TransformRelation::ChildFromParent)
    {
        let determinant = transform.matrix3.determinant();
        if determinant != 0.0 && determinant.is_finite() {
            transform = transform.inverse();
        } else {
            // All "regular invalid" transforms should have been caught.
            // So ending up here means something else went wrong?
            re_log::warn_once!(
                "Failed to express child-from-parent transform at {} since it wasn't invertible",
                entity_path,
            );
        }
    }

    Ok(ParentFromChildTransform { parent, transform })
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
// TODO(#3849): There's no uniform way to discover invalid transforms right now (they can be intentional but often aren't).
// Here, we only detect and ignore invalid rotations and log an error.
pub fn query_and_resolve_instance_poses_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    chunk_id: ChunkId,
    row_id: RowId,
) -> Vec<DAffine3> {
    let identifier_translations = InstancePoses3D::descriptor_translations().component;
    let identifier_rotation_axis_angles =
        InstancePoses3D::descriptor_rotation_axis_angles().component;
    let identifier_quaternions = InstancePoses3D::descriptor_quaternions().component;
    let identifier_scales = InstancePoses3D::descriptor_scales().component;
    let identifier_mat3x3 = InstancePoses3D::descriptor_mat3x3().component;

    let all_components_of_transaction = atomic_component_set_for_instance_poses();

    // We're querying for transactional/atomic pose state:
    // If any of the topology or geometry components change, we reset all poses.
    //
    // This means we don't have to do latest-at for individual components.
    // Instead, we're looking for the last change and then get everything with that row id.
    //
    // We bipass the query cache here:
    // * we're already doing special caching anyways
    // * we don't want to merge over row-ids *at all* since our query handling here is a little bit different. The query cache is geared towards "regular Rerun semantics"
    // * we already handled `Clear`/`ClearRecursive` upon pre-population of our cache entries (we know when a clear occurs on this entity!)
    let storage_engine = entity_db.storage_engine();
    let Some((chunk, row_index)) =
        lookup_chunk_row(&storage_engine, missing_chunk_reporter, chunk_id, row_id)
    else {
        return Vec::new();
    };

    let max_num_instances = all_components_of_transaction
        .iter()
        .map(|component| {
            chunk
                .component_batch_raw(*component, row_index)
                .and_then(|batch| batch.ok())
                .map_or(0, |batch| batch.len())
        })
        .max()
        .unwrap_or(0);

    if max_num_instances == 0 {
        return Vec::new();
    }

    #[inline]
    pub fn clamped_or_nothing<T: Clone>(
        values: Vec<T>,
        clamped_len: usize,
    ) -> impl Iterator<Item = T> {
        let Some(last) = values.last() else {
            return Either::Left(std::iter::empty());
        };
        let last = last.clone();
        Either::Right(
            values
                .into_iter()
                .chain(std::iter::repeat(last))
                .take(clamped_len),
        )
    }

    let batch_translation = chunk
        .component_batch::<components::Translation3D>(identifier_translations, row_index)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_rotation_axis_angle = chunk
        .component_batch::<components::RotationAxisAngle>(
            identifier_rotation_axis_angles,
            row_index,
        )
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_rotation_quat = chunk
        .component_batch::<components::RotationQuat>(identifier_quaternions, row_index)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_scale = chunk
        .component_batch::<components::Scale3D>(identifier_scales, row_index)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_mat3x3 = chunk
        .component_batch::<components::TransformMat3x3>(identifier_mat3x3, row_index)
        .and_then(|v| v.ok())
        .unwrap_or_default();

    if batch_translation.is_empty()
        && batch_rotation_axis_angle.is_empty()
        && batch_rotation_quat.is_empty()
        && batch_scale.is_empty()
        && batch_mat3x3.is_empty()
    {
        return Vec::new();
    }
    let mut iter_translation = clamped_or_nothing(batch_translation, max_num_instances);
    let mut iter_rotation_axis_angle =
        clamped_or_nothing(batch_rotation_axis_angle, max_num_instances);
    let mut iter_rotation_quat = clamped_or_nothing(batch_rotation_quat, max_num_instances);
    let mut iter_scale = clamped_or_nothing(batch_scale, max_num_instances);
    let mut iter_mat3x3 = clamped_or_nothing(batch_mat3x3, max_num_instances);

    // Gracefully ignore invalid rotations (e.g. an accidentally unnormalized quaternion like [0, 0, 0, 0]),
    // but log an error about it to inform the user.
    let mut has_invalid_rotation = false;

    let transforms = (0..max_num_instances)
        .map(|_| {
            // We apply these in a specific order.
            #[expect(clippy::useless_let_if_seq)]
            let mut transform = DAffine3::IDENTITY;

            if let Some(translation) = iter_translation.next() {
                transform = convert::translation_3d_to_daffine3(translation);
            }
            if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
                if let Ok(axis_angle) =
                    convert::rotation_axis_angle_to_daffine3(rotation_axis_angle)
                {
                    transform *= axis_angle;
                } else {
                    has_invalid_rotation = true;
                }
            }
            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = convert::rotation_quat_to_daffine3(rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    has_invalid_rotation = true;
                }
            }
            if let Some(scale) = iter_scale.next() {
                transform *= convert::scale_3d_to_daffine3(scale);
            }
            if let Some(mat3x3) = iter_mat3x3.next() {
                transform *= convert::transform_mat3x3_to_daffine3(mat3x3);
            }
            transform
        })
        .collect();

    if has_invalid_rotation {
        re_log::warn_once!(
            "Detected an invalid rotation in the instance poses at {}. Ignoring it and treating it as an identity rotation.",
            entity_path
        );
    }

    transforms
}

pub fn query_and_resolve_pinhole_projection_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    chunk_id: ChunkId,
    row_id: RowId,
) -> Result<ResolvedPinholeProjectionCached, TransformError> {
    // Topology
    let identifier_parent_frame = archetypes::Pinhole::descriptor_parent_frame().component;
    // Geometry
    let identifier_image_from_camera =
        archetypes::Pinhole::descriptor_image_from_camera().component;
    let identifier_resolution = archetypes::Pinhole::descriptor_resolution().component;

    let storage_engine = entity_db.storage_engine();
    let Some((chunk, row_index)) =
        lookup_chunk_row(&storage_engine, missing_chunk_reporter, chunk_id, row_id)
    else {
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };

    let Some(image_from_camera) = chunk
        .component_mono::<components::PinholeProjection>(identifier_image_from_camera, row_index)
        .and_then(|v| v.ok())
    else {
        // Intrinsics are required.
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };
    let resolution = chunk
        .component_mono::<components::Resolution>(identifier_resolution, row_index)
        .and_then(|v| v.ok());

    let parent = get_parent_frame(chunk, row_index, entity_path, identifier_parent_frame)?;

    Ok(ResolvedPinholeProjectionCached {
        parent,
        image_from_camera,
        resolution,
    })
}

fn get_parent_frame(
    chunk: &ChunkShared,
    row_index: usize,
    entity_path: &EntityPath,
    identifier_parent_frame: ComponentIdentifier,
) -> Result<TransformFrameIdHash, TransformError> {
    chunk
        .component_mono::<components::TransformFrameId>(identifier_parent_frame, row_index)
        .and_then(|v| v.ok())
        .map_or_else(
            || {
                entity_path
                    .parent()
                    .ok_or(TransformError::ImplicitRootParentFrame)
                    .map(|parent| TransformFrameIdHash::from_entity_path(&parent))
            },
            |frame_id| {
                if frame_id.as_str().is_empty() {
                    Err(TransformError::EmptyParentFrame {
                        entity_path: entity_path.clone(),
                        component: identifier_parent_frame,
                    })
                } else {
                    Ok(TransformFrameIdHash::new(&frame_id))
                }
            },
        )
}

/// Queries view coordinates from either the [`archetypes::Pinhole`] or [`archetypes::ViewCoordinates`] archetype.
///
/// Gives precedence to the `Pinhole` archetype.
// TODO(#2663): This is confusing and should be cleaned up.
pub fn query_view_coordinates(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<components::ViewCoordinates> {
    entity_db
        .latest_at_component::<components::ViewCoordinates>(
            entity_path,
            query,
            archetypes::Pinhole::descriptor_camera_xyz().component,
        )
        .or_else(|| {
            entity_db.latest_at_component::<components::ViewCoordinates>(
                entity_path,
                query,
                archetypes::ViewCoordinates::descriptor_xyz().component,
            )
        })
        .map(|(_index, view_coordinates)| view_coordinates)
}

/// Queries view coordinates from either the [`archetypes::Pinhole`] or [`archetypes::ViewCoordinates`] archetype
/// at the closest ancestor of the given entity path.
///
/// Gives precedence to the `Pinhole` archetype.
// TODO(#2663): This is confusing and should be cleaned up.
pub fn query_view_coordinates_at_closest_ancestor(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<components::ViewCoordinates> {
    entity_db
        .latest_at_component_at_closest_ancestor::<components::ViewCoordinates>(
            entity_path,
            query,
            archetypes::Pinhole::descriptor_camera_xyz().component,
        )
        .or_else(|| {
            entity_db.latest_at_component_at_closest_ancestor::<components::ViewCoordinates>(
                entity_path,
                query,
                archetypes::ViewCoordinates::descriptor_xyz().component,
            )
        })
        .map(|(_path, _index, view_coordinates)| view_coordinates)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::Chunk;
    use re_entity_db::{EntityDb, EntityPath};
    use re_log_types::Timeline;
    use re_sdk_types::{archetypes::InstancePoses3D, components::RotationQuat};

    use super::*;

    /// Test that an invalid instance pose quaternion is ignored while still keeping the translation.
    #[test]
    fn invalid_instance_pose_quaternion_preserves_translation()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        let timeline = Timeline::new_sequence("t");
        let entity_path = EntityPath::from("my_entity");
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype_auto_row(
                [(timeline, 1)],
                &InstancePoses3D::new()
                    .with_translations([[1.0, 2.0, 3.0]])
                    .with_quaternions([RotationQuat::INVALID]),
            )
            .build()?;
        let chunk_id = chunk.id();
        let row_id = chunk.row_ids_slice()[0];
        entity_db.add_chunk(&Arc::new(chunk))?;

        let poses = query_and_resolve_instance_poses_at_entity(
            &entity_db,
            &MissingChunkReporter::default(),
            &entity_path,
            chunk_id,
            row_id,
        );

        assert_eq!(
            poses,
            vec![DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))]
        );

        Ok(())
    }
}
