//! Utilities for querying out transform types.

use glam::DAffine3;
use itertools::Either;

use crate::convert;
use crate::{ResolvedPinholeProjection, transform_resolution_cache::ParentFromChildTransform};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::archetypes::InstancePoses3D;
use re_types::{
    Archetype as _, ArchetypeName, Component as _, ComponentDescriptor, ComponentIdentifier,
    TransformFrameIdHash,
    archetypes::{self},
    components,
    reflection::ComponentDescriptorExt as _,
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
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns `None`.
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
// TODO(grtlr): Consider returning a `SmallVec1`.
pub fn query_and_resolve_tree_transform_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Result<Vec<(TransformFrameIdHash, ParentFromChildTransform)>, TransformError> {
    // TODO(RR-2799): Output more than one target at once, doing the usual clamping - means probably we can merge a lot of code here with instance poses!

    // Topology
    let identifier_parent_frame = archetypes::Transform3D::descriptor_parent_frame().component;
    let identifier_child_frame = archetypes::Transform3D::descriptor_child_frame().component;
    let identifier_relation = archetypes::Transform3D::descriptor_relation().component;

    // Geometry
    let identifier_translations = archetypes::Transform3D::descriptor_translation().component;
    let identifier_rotation_axis_angles =
        archetypes::Transform3D::descriptor_rotation_axis_angle().component;
    let identifier_quaternions = archetypes::Transform3D::descriptor_quaternion().component;
    let identifier_scales = archetypes::Transform3D::descriptor_scale().component;
    let identifier_mat3x3 = archetypes::Transform3D::descriptor_mat3x3().component;

    let results = entity_db.latest_at(
        query,
        entity_path,
        [
            identifier_parent_frame,
            identifier_child_frame,
            identifier_relation,
            identifier_translations,
            identifier_rotation_axis_angles,
            identifier_quaternions,
            identifier_scales,
            identifier_mat3x3,
        ],
    );
    if results.components.is_empty() {
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    }

    let parent = results
        .component_mono_quiet::<components::TransformFrameId>(identifier_parent_frame)
        .map_or_else(
            || {
                TransformFrameIdHash::from_entity_path(
                    &entity_path.parent().unwrap_or(EntityPath::root()),
                )
            },
            |frame_id| TransformFrameIdHash::new(&frame_id),
        );

    let child = results
        .component_mono_quiet::<components::TransformFrameId>(identifier_child_frame)
        .map_or_else(
            || TransformFrameIdHash::from_entity_path(entity_path),
            |frame_id| TransformFrameIdHash::new(&frame_id),
        );

    let mut transform = DAffine3::IDENTITY;

    // It's an error if there's more than one component. Warn in that case.
    let mono_log_level = re_log::Level::Warn;

    // The order of the components here is important and checked by `debug_assert_transform_field_order`
    if let Some(translation) = results.component_mono_with_log_level::<components::Translation3D>(
        identifier_translations,
        mono_log_level,
    ) {
        transform = convert::translation_3d_to_daffine3(translation);
    }
    if let Some(axis_angle) = results
        .component_mono_with_log_level::<components::RotationAxisAngle>(
            identifier_rotation_axis_angles,
            mono_log_level,
        )
    {
        let axis_angle = convert::rotation_axis_angle_to_daffine3(axis_angle).map_err(|_err| {
            TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_rotation_axis_angles,
            }
        })?;
        transform *= axis_angle;
    }
    if let Some(quaternion) = results.component_mono_with_log_level::<components::RotationQuat>(
        identifier_quaternions,
        mono_log_level,
    ) {
        let quaternion = convert::rotation_quat_to_daffine3(quaternion).map_err(|_err| {
            TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_quaternions,
            }
        })?;
        transform *= quaternion;
    }
    if let Some(scale) = results
        .component_mono_with_log_level::<components::Scale3D>(identifier_scales, mono_log_level)
    {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            return Err(TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_scales,
            });
        }
        transform *= convert::scale_3d_to_daffine3(scale);
    }
    if let Some(mat3x3) = results.component_mono_with_log_level::<components::TransformMat3x3>(
        identifier_mat3x3,
        mono_log_level,
    ) {
        let affine_transform = convert::transform_mat3x3_to_daffine3(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            return Err(TransformError::InvalidTransform {
                entity_path: entity_path.clone(),
                component: identifier_mat3x3,
            });
        }
        transform *= affine_transform;
    }

    if results.component_mono_with_log_level::<components::TransformRelation>(
        identifier_relation,
        mono_log_level,
    ) == Some(components::TransformRelation::ChildFromParent)
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

    Ok(vec![(
        child,
        ParentFromChildTransform { transform, parent },
    )])
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns a `glam::DAffine3::ZERO` for that instance.
/// (this effectively ignores the instance for most visualizations!)
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
pub fn query_and_resolve_instance_poses_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Vec<DAffine3> {
    query_and_resolve_instance_from_pose_for_archetype_name(
        entity_path,
        entity_db,
        query,
        archetypes::InstancePoses3D::name(),
        &archetypes::InstancePoses3D::descriptor_translations(),
    )
}

/// Queries pose transforms for a specific archetype.
///
/// Note that the component for translation specifically may vary.
/// (this is technical debt, we should fix this)
fn query_and_resolve_instance_from_pose_for_archetype_name(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    archetype_name: ArchetypeName,
    descriptor_translations: &ComponentDescriptor,
) -> Vec<DAffine3> {
    debug_assert_eq!(
        descriptor_translations.component_type,
        Some(components::PoseTranslation3D::name())
    );
    debug_assert_eq!(descriptor_translations.archetype, Some(archetype_name));
    let identifier_translations = descriptor_translations.component;
    let identifier_rotation_axis_angles = InstancePoses3D::descriptor_rotation_axis_angles()
        .with_builtin_archetype(archetype_name)
        .component;
    let identifier_quaternions = InstancePoses3D::descriptor_quaternions()
        .with_builtin_archetype(archetype_name)
        .component;
    let identifier_scales = InstancePoses3D::descriptor_scales()
        .with_builtin_archetype(archetype_name)
        .component;
    let identifier_mat3x3 = InstancePoses3D::descriptor_mat3x3()
        .with_builtin_archetype(archetype_name)
        .component;

    let result = entity_db.latest_at(
        query,
        entity_path,
        [
            identifier_translations,
            identifier_rotation_axis_angles,
            identifier_quaternions,
            identifier_scales,
            identifier_mat3x3,
        ],
    );

    let max_num_instances = result
        .components
        .iter()
        .map(|(component, row)| row.num_instances(*component))
        .max()
        .unwrap_or(0) as usize;

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

    let batch_translation = result
        .component_batch::<components::PoseTranslation3D>(identifier_translations)
        .unwrap_or_default();
    let batch_rotation_quat = result
        .component_batch::<components::PoseRotationQuat>(identifier_quaternions)
        .unwrap_or_default();
    let batch_rotation_axis_angle = result
        .component_batch::<components::PoseRotationAxisAngle>(identifier_rotation_axis_angles)
        .unwrap_or_default();
    let batch_scale = result
        .component_batch::<components::PoseScale3D>(identifier_scales)
        .unwrap_or_default();
    let batch_mat3x3 = result
        .component_batch::<components::PoseTransformMat3x3>(identifier_mat3x3)
        .unwrap_or_default();

    if batch_translation.is_empty()
        && batch_rotation_quat.is_empty()
        && batch_rotation_axis_angle.is_empty()
        && batch_scale.is_empty()
        && batch_mat3x3.is_empty()
    {
        return Vec::new();
    }
    let mut iter_translation = clamped_or_nothing(batch_translation, max_num_instances);
    let mut iter_rotation_quat = clamped_or_nothing(batch_rotation_quat, max_num_instances);
    let mut iter_rotation_axis_angle =
        clamped_or_nothing(batch_rotation_axis_angle, max_num_instances);
    let mut iter_scale = clamped_or_nothing(batch_scale, max_num_instances);
    let mut iter_mat3x3 = clamped_or_nothing(batch_mat3x3, max_num_instances);

    (0..max_num_instances)
        .map(|_| {
            // We apply these in a specific order - see `debug_assert_transform_field_order`
            let mut transform = DAffine3::IDENTITY;
            if let Some(translation) = iter_translation.next() {
                transform = convert::pose_translation_3d_to_daffine3(translation);
            }
            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = convert::pose_rotation_quat_to_daffine3(rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    transform = DAffine3::ZERO;
                }
            }
            if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
                if let Ok(axis_angle) =
                    convert::pose_rotation_axis_angle_to_daffine3(rotation_axis_angle)
                {
                    transform *= axis_angle;
                } else {
                    transform = DAffine3::ZERO;
                }
            }
            if let Some(scale) = iter_scale.next() {
                transform *= convert::pose_scale_3d_to_daffine3(scale);
            }
            if let Some(mat3x3) = iter_mat3x3.next() {
                transform *= convert::pose_transform_mat3x3_to_daffine3(mat3x3);
            }
            transform
        })
        .collect()
}

pub fn query_and_resolve_pinhole_projection_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<ResolvedPinholeProjection> {
    entity_db
        .latest_at_component::<components::PinholeProjection>(
            entity_path,
            query,
            archetypes::Pinhole::descriptor_image_from_camera().component,
        )
        .map(|(_index, image_from_camera)| ResolvedPinholeProjection {
            // Pinholes don't have an explicit target frame yet, so they always apply to the parent frame.
            parent: TransformFrameIdHash::from_entity_path(
                &entity_path.parent().unwrap_or(EntityPath::root()),
            ),

            image_from_camera,
            resolution: entity_db
                .latest_at_component::<components::Resolution>(
                    entity_path,
                    query,
                    archetypes::Pinhole::descriptor_resolution().component,
                )
                .map(|(_index, resolution)| resolution),
            view_coordinates: {
                query_view_coordinates(entity_path, entity_db, query)
                    .unwrap_or(archetypes::Pinhole::DEFAULT_CAMERA_XYZ)
            },
        })
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
