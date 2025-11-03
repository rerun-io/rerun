//! Utilities for querying out transform types.

use glam::DAffine3;
use itertools::Either;
use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::{
    Archetype as _, ArchetypeName, Component as _, ComponentDescriptor, TransformFrameIdHash,
    archetypes::{self, InstancePoses3D},
    components,
    reflection::ComponentDescriptorExt as _,
};
use vec1::smallvec_v1::SmallVec1;

use crate::{PoseTransformArchetypeMap, ResolvedPinholeProjection};

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
                transform = DAffine3::from(translation);
            }
            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = DAffine3::try_from(rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    transform = DAffine3::ZERO;
                }
            }
            if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
                if let Ok(axis_angle) = DAffine3::try_from(rotation_axis_angle) {
                    transform *= axis_angle;
                } else {
                    transform = DAffine3::ZERO;
                }
            }
            if let Some(scale) = iter_scale.next() {
                transform *= DAffine3::from(scale);
            }
            if let Some(mat3x3) = iter_mat3x3.next() {
                transform *= DAffine3::from(mat3x3);
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
            target: TransformFrameIdHash::from_entity_path(
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
