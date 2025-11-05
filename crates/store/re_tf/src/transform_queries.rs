//! Utilities for querying out transform types.

use glam::DAffine3;
use itertools::Either;
use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::{
    Archetype as _, ArchetypeName, Component as _, ComponentDescriptor, ComponentIdentifier,
    TransformFrameIdHash,
    archetypes::{self, InstancePoses3D},
    components,
    reflection::ComponentDescriptorExt as _,
};
use vec1::smallvec_v1::SmallVec1;

use crate::{
    PoseTransformArchetypeMap, ResolvedPinholeProjection,
    transform_resolution_cache::ParentFromChildTransform,
};

/// Lists all archetypes except [`archetypes::InstancePoses3D`] that have their own instance poses.
// TODO(andreas, jleibs): Model this out as a generic extension mechanism.
fn archetypes_with_instance_pose_transforms_and_translation_descriptor()
-> [(ArchetypeName, ComponentDescriptor); 4] {
    [
        (
            archetypes::Boxes3D::name(),
            archetypes::Boxes3D::descriptor_centers(),
        ),
        (
            archetypes::Ellipsoids3D::name(),
            archetypes::Ellipsoids3D::descriptor_centers(),
        ),
        (
            archetypes::Capsules3D::name(),
            archetypes::Capsules3D::descriptor_translations(),
        ),
        (
            archetypes::Cylinders3D::name(),
            archetypes::Cylinders3D::descriptor_centers(),
        ),
    ]
}

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
            || TransformFrameIdHash::from_entity_path(&entity_path),
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
        let axis_angle = convert::rotation_axis_angle_to_daffine3(axis_angle).map_err(|_| {
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
        let quaternion = convert::rotation_quat_to_daffine3(quaternion).map_err(|_| {
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
) -> PoseTransformArchetypeMap {
    let instance_from_overall_poses = query_and_resolve_instance_from_pose_for_archetype_name(
        entity_path,
        entity_db,
        query,
        archetypes::InstancePoses3D::name(),
        &archetypes::InstancePoses3D::descriptor_translations(),
    );

    // Some archetypes support their own instance poses.
    // TODO(andreas): can we quickly determine whether this is necessary for any given archetype?
    // TODO(andreas): Should we make all of this a single large query?
    let mut instance_from_archetype_poses_per_archetype = IntMap::default();
    for (archetype_name, descriptor_translations) in
        archetypes_with_instance_pose_transforms_and_translation_descriptor()
    {
        if let Ok(mut instance_from_archetype_poses) =
            SmallVec1::try_from_vec(query_and_resolve_instance_from_pose_for_archetype_name(
                entity_path,
                entity_db,
                query,
                archetype_name,
                &descriptor_translations,
            ))
        {
            // "zip" up with the overall poses.
            let length = instance_from_archetype_poses
                .len()
                .max(instance_from_overall_poses.len());
            instance_from_archetype_poses
                .resize(length, *instance_from_archetype_poses.last()) // Components repeat.
                .expect("Overall number of poses can't be zero.");

            for (instance_from_archetype_pose, instance_from_overall_pose) in
                instance_from_archetype_poses
                    .iter_mut()
                    .zip(instance_from_overall_poses.iter())
            {
                let overall_pose_archetype_pose = *instance_from_archetype_pose;
                *instance_from_archetype_pose =
                    (*instance_from_overall_pose) * overall_pose_archetype_pose;
            }

            instance_from_archetype_poses_per_archetype
                .insert(archetype_name, instance_from_archetype_poses);
        }
    }

    PoseTransformArchetypeMap {
        instance_from_archetype_poses_per_archetype,
        instance_from_poses: instance_from_overall_poses,
    }
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

pub(crate) mod convert {
    //! Conversion functions for transform components to double precision types.
    //!
    //! These conversions are used internally by `re_tf` for transform computations until
    //! we have proper data type generics. We put them here to make future generic refactoring
    //! easier.

    use glam::{DAffine3, DMat3, DQuat, DVec3};
    use re_types::{components, datatypes};

    // ---------------------------------------------------------------------------
    // Helper functions for datatypes

    #[inline]
    pub(crate) fn quaternion_to_dquat(q: datatypes::Quaternion) -> Result<DQuat, ()> {
        let q = q.0;
        glam::DVec4::new(q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64)
            .try_normalize()
            .map(DQuat::from_vec4)
            .ok_or(())
    }

    #[inline]
    pub(super) fn vec3d_to_dvec3(v: datatypes::Vec3D) -> DVec3 {
        let v = v.0;
        DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
    }

    // ---------------------------------------------------------------------------
    // Component conversion functions

    #[inline]
    pub(super) fn translation_3d_to_daffine3(v: components::Translation3D) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::IDENTITY,
            translation: vec3d_to_dvec3(v.0),
        }
    }

    #[inline]
    pub(super) fn rotation_axis_angle_to_daffine3(
        val: components::RotationAxisAngle,
    ) -> Result<DAffine3, ()> {
        vec3d_to_dvec3(val.0.axis)
            .try_normalize()
            .map(|normalized| DAffine3::from_axis_angle(normalized, val.0.angle.radians() as f64))
            .ok_or(())
    }

    #[inline]
    pub(super) fn rotation_quat_to_daffine3(val: components::RotationQuat) -> Result<DAffine3, ()> {
        Ok(DAffine3::from_quat(quaternion_to_dquat(val.0)?))
    }

    #[inline]
    pub(super) fn scale_3d_to_daffine3(v: components::Scale3D) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::from_diagonal(vec3d_to_dvec3(v.0)),
            translation: DVec3::ZERO,
        }
    }

    #[inline]
    pub(super) fn transform_mat3x3_to_daffine3(v: components::TransformMat3x3) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::from_cols_array(&v.0.0.map(|x| x as f64)),
            translation: DVec3::ZERO,
        }
    }

    // ---------------------------------------------------------------------------
    // Pose component conversion functions

    #[inline]
    pub(super) fn pose_translation_3d_to_daffine3(v: components::PoseTranslation3D) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::IDENTITY,
            translation: vec3d_to_dvec3(v.0),
        }
    }

    #[inline]
    pub(super) fn pose_rotation_axis_angle_to_daffine3(
        val: components::PoseRotationAxisAngle,
    ) -> Result<DAffine3, ()> {
        // 0 degrees around any axis is an identity transform.
        if val.angle.radians == 0. {
            Ok(DAffine3::IDENTITY)
        } else {
            vec3d_to_dvec3(val.0.axis)
                .try_normalize()
                .map(|normalized| {
                    DAffine3::from_axis_angle(normalized, val.0.angle.radians() as f64)
                })
                .ok_or(())
        }
    }

    #[inline]
    pub(super) fn pose_rotation_quat_to_daffine3(
        val: components::PoseRotationQuat,
    ) -> Result<DAffine3, ()> {
        Ok(DAffine3::from_quat(quaternion_to_dquat(val.0)?))
    }

    #[inline]
    pub(super) fn pose_scale_3d_to_daffine3(v: components::PoseScale3D) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::from_diagonal(vec3d_to_dvec3(v.0)),
            translation: DVec3::ZERO,
        }
    }

    #[inline]
    pub(super) fn pose_transform_mat3x3_to_daffine3(
        v: components::PoseTransformMat3x3,
    ) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::from_cols_array(&v.0.0.map(|mn| mn as f64)),
            translation: DVec3::ZERO,
        }
    }
}
