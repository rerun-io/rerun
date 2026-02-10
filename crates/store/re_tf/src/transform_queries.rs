//! Utilities for querying out transform types.

use glam::DAffine3;
use itertools::{Either, Itertools as _};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::{Chunk, LatestAtQuery, MissingChunkReporter, OnMissingChunk, UnitChunkShared};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::archetypes::{self, InstancePoses3D};
use re_sdk_types::external::arrow;
use re_sdk_types::external::arrow::array::Array as _;
use re_sdk_types::{ComponentIdentifier, TransformFrameIdHash, components};

use crate::transform_resolution_cache::ParentFromChildTransform;
use crate::{ResolvedPinholeProjection, convert};

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
        "Ignoring transform at root entity /. Transforms require either a parent entity that can be used as implicit frame, or the parent_frame field to be set."
    )]
    ImplicitRootParentFrame,
}

/// Returns true if any of the given components is non-null on the given row.
fn has_row_any_component(
    chunk: &Chunk,
    row_index: usize,
    components: &[ComponentIdentifier],
) -> bool {
    components.iter().any(|component| {
        chunk
            .components()
            .get_array(*component)
            .is_some_and(|array| !array.is_null(row_index))
    })
}

/// Filters a atomic-latest-at the given  [`Self::requested_frame_id`] at the [`Self::condition_frame_id_component`].
/// We have to find the last row-id for the given `condition_frame_id_component` and time.
/// Today, `condition_frame_id_component` is always `child_frame_id` for either `Transform3D` or `Pinhole`.
#[derive(Copy, Clone)]
struct AtomicLatestAtFrameFilter {
    condition_frame_id_component: ComponentIdentifier,
    requested_frame_id: TransformFrameIdHash,
}

/// Finds a unit chunk/row that has the latest changes for the given set of components and optionally matches for a frame id.
///
/// Since everything has the same row-id, everything has to be on the same chunk -> we return a unit chunk!
///
/// Does **not** handle clears. Our transform cache already handles clear events separately,
/// since we eagerly create events whenever a change occurs.
/// (Unlike transform components, we immediately read out clears and add those clear events to our event book-keeping)
fn atomic_latest_at_query(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    frame_filter: Option<AtomicLatestAtFrameFilter>,
    atomic_component_set: &[ComponentIdentifier],
) -> Option<UnitChunkShared> {
    let storage_engine = entity_db.storage_engine();
    let store = storage_engine.store();
    let include_static = true;
    let chunks = store.latest_at_relevant_chunks_for_all_components(
        OnMissingChunk::Report,
        query,
        entity_path,
        include_static,
    );

    let re_chunk_store::QueryResults {
        chunks,
        missing_virtual,
    } = chunks;

    if !missing_virtual.is_empty() {
        missing_chunk_reporter.report_missing_chunk();
    }

    let entity_path_derived_frame_id = TransformFrameIdHash::from_entity_path(entity_path);

    let mut unit_chunk: Option<UnitChunkShared> = None;

    let query_time = query.at().as_i64();

    // TODO(RR-3295): what should we do with virtual chunks here?
    for chunk in chunks {
        // Make sure the chunk is sorted (they usually are) in order to ensure we're getting the last relevant row.
        let chunk = if chunk.is_sorted() {
            chunk
        } else {
            let mut sorted_chunk = (*chunk).clone();
            sorted_chunk.sort_if_unsorted();
            std::sync::Arc::new(sorted_chunk)
        };

        let mut row_indices_with_queried_time_from_new_to_old = if let Some(time_column) =
            chunk.timelines().get(&query.timeline())
            && query_time != TimeInt::STATIC.as_i64()
        {
            if time_column.is_sorted() {
                let partition_point = time_column
                    .times_raw()
                    .partition_point(|time| *time <= query_time);
                Either::Left((0..partition_point).rev())
            } else {
                Either::Right(
                    time_column
                        .times_raw()
                        .iter()
                        .enumerate()
                        .filter(|(_row_index, time)| **time <= query_time)
                        .sorted_by_key(|(_row_index, time)| *time)
                        // Do *not* sort by negative time instead.
                        // This gives a subtly different outcome since sorting is stable it would mean that runs of equal times wouldn't be reversed then.
                        .rev()
                        .map(|(row_index, _time)| row_index),
                )
            }
        } else {
            Either::Left((0..chunk.num_rows()).rev())
        };

        // Finds the last row index with time <= the query time and a matching frame id.
        let highest_row_index_with_expected_frame_id = if let Some(AtomicLatestAtFrameFilter {
            condition_frame_id_component,
            requested_frame_id,
        }) = frame_filter
        {
            if let Some(frame_id_column) =
                chunk.components().get_array(condition_frame_id_component)
            {
                row_indices_with_queried_time_from_new_to_old.find(|index| {
                let frame_id_row_untyped = frame_id_column.value(*index);
                let Some(frame_id_row) =
                    frame_id_row_untyped.downcast_array_ref::<arrow::array::StringArray>()
                else {
                    re_log::error_once!("Expected at {condition_frame_id_component:?} @ {entity_path:?} to be a string array, but its type is instead {}", frame_id_row_untyped.data_type());
                    return false;
                };
                // Right now everything is singular on a single row, so check only the first element of this string array.
                let frame_id = if frame_id_row.is_empty() || frame_id_row.is_null(0) {
                    // *Something* on this row has to be non-empty & non-null!
                    // Example where this is not the case:
                    //
                    // ┌────────────────┬─────────────┬────────────┐
                    // │ child_frame_id │ translation │ color      │
                    // ├────────────────┼─────────────┼────────────┤
                    // │ ["myframe"]    │ [[1,2,3]]   │ null       │
                    // │ null           │ null        │ 0xFF00FFFF │
                    // │ null           │ []          │ null       │
                    // └────────────────┴─────────────┴────────────┘
                    //
                    // The second row doesn't have any of the components of our atomic set.
                    // It is therefore not relevant for what we're looking for!
                    // The last row *is* relevant, because it clears out the translation for the
                    // entity derived child_frame_id, thus setting it to an identity transform.
                    if !has_row_any_component(&chunk, *index, atomic_component_set) {
                        return false;
                    }
                    entity_path_derived_frame_id
                } else {
                    TransformFrameIdHash::from_str(frame_id_row.value(0))
                };

                frame_id == requested_frame_id
            })
            } else if entity_path_derived_frame_id == requested_frame_id {
                // Pick the last where any relevant component is non-null & non-empty.
                row_indices_with_queried_time_from_new_to_old
                    .find(|index| has_row_any_component(&chunk, *index, atomic_component_set))
            } else {
                // There's no child_frame id and we're also not looking for the entity-path derived frame,
                // so this chunk doesn't have any information about the transform we're looking for.
                continue;
            }
        } else {
            // Pick the last where any relevant component is non-null & non-empty.
            row_indices_with_queried_time_from_new_to_old
                .find(|index| has_row_any_component(&chunk, *index, atomic_component_set))
        };

        if let Some(row_index) = highest_row_index_with_expected_frame_id {
            debug_assert!(!chunk.is_empty());
            let new_unit_chunk = chunk.row_sliced_shallow(row_index, 1).into_unit()
                .expect("Chunk was just sliced to single row, therefore it must be convertible to a unit chunk");

            if let Some(previous_chunk) = &unit_chunk
                && previous_chunk.row_id() > new_unit_chunk.row_id()
            {
                // This should be rare: there's another chunk that also fits the exact same child id and the exact same time.
                // Have to use row id as the tie breaker - if we failed that we're in here.
            } else {
                unit_chunk = chunk.row_sliced_shallow(row_index, 1).into_unit();
            }
        }
    }

    unit_chunk
}

/// Queries & processes all components that are part of a transform, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns `None`.
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
pub fn query_and_resolve_tree_transform_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    child_frame_id: TransformFrameIdHash,
    query: &LatestAtQuery,
) -> Result<ParentFromChildTransform, TransformError> {
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

    let all_components_of_transaction = [
        identifier_parent_frame,
        identifier_child_frame,
        identifier_relation,
        // Geometry
        identifier_translations,
        identifier_rotation_axis_angles,
        identifier_quaternions,
        identifier_scales,
        identifier_mat3x3,
    ];

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
    let unit_chunk: Option<UnitChunkShared> = atomic_latest_at_query(
        entity_db,
        missing_chunk_reporter,
        query,
        entity_path,
        Some(AtomicLatestAtFrameFilter {
            condition_frame_id_component: identifier_child_frame,
            requested_frame_id: child_frame_id,
        }),
        &all_components_of_transaction,
    );
    let Some(unit_chunk) = unit_chunk else {
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };

    // TODO(andreas): silently ignores deserialization error right now.

    let parent = get_parent_frame(&unit_chunk, entity_path, identifier_parent_frame)?;

    #[expect(clippy::useless_let_if_seq)]
    let mut transform = DAffine3::IDENTITY;

    // The order of the components here is important.
    if let Some(translation) = unit_chunk
        .component_mono::<components::Translation3D>(identifier_translations)
        .and_then(|v| v.ok())
    {
        transform = convert::translation_3d_to_daffine3(translation);
    }
    if let Some(axis_angle) = unit_chunk
        .component_mono::<components::RotationAxisAngle>(identifier_rotation_axis_angles)
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
    if let Some(quaternion) = unit_chunk
        .component_mono::<components::RotationQuat>(identifier_quaternions)
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
    if let Some(scale) = unit_chunk
        .component_mono::<components::Scale3D>(identifier_scales)
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
    if let Some(mat3x3) = unit_chunk
        .component_mono::<components::TransformMat3x3>(identifier_mat3x3)
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

    if unit_chunk
        .component_mono::<components::TransformRelation>(identifier_relation)
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
/// If any of the components yields an invalid transform, returns a `glam::DAffine3::ZERO` for that instance.
/// (this effectively ignores the instance for most visualizations!)
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
pub fn query_and_resolve_instance_poses_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Vec<DAffine3> {
    let identifier_translations = InstancePoses3D::descriptor_translations().component;
    let identifier_rotation_axis_angles =
        InstancePoses3D::descriptor_rotation_axis_angles().component;
    let identifier_quaternions = InstancePoses3D::descriptor_quaternions().component;
    let identifier_scales = InstancePoses3D::descriptor_scales().component;
    let identifier_mat3x3 = InstancePoses3D::descriptor_mat3x3().component;

    let all_components_of_transaction = [
        identifier_translations,
        identifier_rotation_axis_angles,
        identifier_quaternions,
        identifier_scales,
        identifier_mat3x3,
    ];

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
    let unit_chunk: Option<UnitChunkShared> = atomic_latest_at_query(
        entity_db,
        missing_chunk_reporter,
        query,
        entity_path,
        None,
        &all_components_of_transaction,
    );
    let Some(unit_chunk) = unit_chunk else {
        return Vec::new();
    };

    let max_num_instances = all_components_of_transaction
        .iter()
        .map(|component| {
            unit_chunk
                .component_batch_raw(*component)
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

    let batch_translation = unit_chunk
        .component_batch::<components::Translation3D>(identifier_translations)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_rotation_axis_angle = unit_chunk
        .component_batch::<components::RotationAxisAngle>(identifier_rotation_axis_angles)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_rotation_quat = unit_chunk
        .component_batch::<components::RotationQuat>(identifier_quaternions)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_scale = unit_chunk
        .component_batch::<components::Scale3D>(identifier_scales)
        .and_then(|v| v.ok())
        .unwrap_or_default();
    let batch_mat3x3 = unit_chunk
        .component_batch::<components::TransformMat3x3>(identifier_mat3x3)
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

    (0..max_num_instances)
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
                    transform = DAffine3::ZERO;
                }
            }
            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = convert::rotation_quat_to_daffine3(rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    transform = DAffine3::ZERO;
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
        .collect()
}

pub fn query_and_resolve_pinhole_projection_at_entity(
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    entity_path: &EntityPath,
    child_frame_id: TransformFrameIdHash,
    query: &LatestAtQuery,
) -> Result<ResolvedPinholeProjection, TransformError> {
    // Topology
    let identifier_parent_frame = archetypes::Pinhole::descriptor_parent_frame().component;
    let identifier_child_frame = archetypes::Pinhole::descriptor_child_frame().component;
    // Geometry
    let identifier_image_from_camera =
        archetypes::Pinhole::descriptor_image_from_camera().component;
    let identifier_resolution = archetypes::Pinhole::descriptor_resolution().component;

    let all_components_of_transaction = [
        identifier_parent_frame,
        identifier_child_frame,
        // Geometry
        identifier_image_from_camera,
        identifier_resolution,
    ];

    let unit_chunk = atomic_latest_at_query(
        entity_db,
        missing_chunk_reporter,
        query,
        entity_path,
        Some(AtomicLatestAtFrameFilter {
            condition_frame_id_component: identifier_child_frame,
            requested_frame_id: child_frame_id,
        }),
        &all_components_of_transaction,
    );
    let Some(unit_chunk) = unit_chunk else {
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };

    let Some(image_from_camera) = unit_chunk
        .component_mono::<components::PinholeProjection>(identifier_image_from_camera)
        .and_then(|v| v.ok())
    else {
        // Intrinsics are required.
        return Err(TransformError::MissingTransform {
            entity_path: entity_path.clone(),
        });
    };
    let resolution = unit_chunk
        .component_mono::<components::Resolution>(identifier_resolution)
        .and_then(|v| v.ok());

    let parent = get_parent_frame(&unit_chunk, entity_path, identifier_parent_frame)?;

    Ok(ResolvedPinholeProjection {
        parent,
        image_from_camera,
        resolution,

        // TODO(andreas): view coordinates are in a weird limbo state in more than one way.
        // Not only are they only _partially_ relevant for the camera's transform (they both name axis & orient cameras),
        // we also rely on them too much being latest-at driven and to make matters worse query them from two different archetypes.
        view_coordinates: {
            query_view_coordinates(entity_path, entity_db, query)
                .unwrap_or(archetypes::Pinhole::DEFAULT_CAMERA_XYZ)
        },
    })
}

fn get_parent_frame(
    unit_chunk: &UnitChunkShared,
    entity_path: &EntityPath,
    identifier_parent_frame: ComponentIdentifier,
) -> Result<TransformFrameIdHash, TransformError> {
    unit_chunk
        .component_mono::<components::TransformFrameId>(identifier_parent_frame)
        .and_then(|v| v.ok())
        .map_or_else(
            || {
                entity_path
                    .parent()
                    .ok_or(TransformError::ImplicitRootParentFrame)
                    .map(|parent| TransformFrameIdHash::from_entity_path(&parent))
            },
            |frame_id| Ok(TransformFrameIdHash::new(&frame_id)),
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

    use re_chunk_store::{Chunk, LatestAtQuery};
    use re_entity_db::{EntityDb, EntityPath};
    use re_log_types::example_components::{MyColor, MyIndex, MyLabel, MyPoint, MyPoints};
    use re_log_types::{TimePoint, Timeline};
    use re_sdk_types::RowId;

    use super::*;

    fn atomic_latest_at_query_test(
        entity_db: &EntityDb,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        frame_filter: Option<AtomicLatestAtFrameFilter>,
        atomic_component_set: &[ComponentIdentifier],
    ) -> Option<UnitChunkShared> {
        let missing_chunk_reporter = MissingChunkReporter::default();
        let result = atomic_latest_at_query(
            entity_db,
            &missing_chunk_reporter,
            query,
            entity_path,
            frame_filter,
            atomic_component_set,
        );
        assert!(
            missing_chunk_reporter.is_empty(),
            "Test expected no missing chunks, but some were missing. This likely means the test is not properly populating the store with all relevant chunks."
        );
        result
    }

    fn timeline() -> Timeline {
        Timeline::new("test_timeline", re_log_types::TimeType::Sequence)
    }

    fn tp(tick: i64) -> TimePoint {
        TimePoint::from([(timeline(), tick)])
    }

    fn atomic_component_set() -> [ComponentIdentifier; 3] {
        [
            MyPoints::descriptor_points().component,
            MyPoints::descriptor_colors().component,
            MyPoints::descriptor_labels().component,
        ]
    }

    fn frame_condition_component() -> ComponentIdentifier {
        // We stick with `MyPoints` all the way and its labels happen to be compatible with frame ids (it's just utf8!)
        MyPoints::descriptor_labels().component
    }

    fn atomic_latest_at_temporal_only_no_frames_present(
        out_of_order: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        // Populate store.
        let entity_path = EntityPath::from("my_entity");
        let row_id_temp0 = RowId::new();
        let row_id_temp1 = RowId::new();
        let row_id_irrelevant = RowId::new();
        let row_id_temp2 = RowId::new();
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_temp0,
                if out_of_order { tp(30) } else { tp(10) },
                &MyPoints::new([MyPoint::new(1.0, 1.0)]).with_colors([MyColor(1)]),
            )
            .with_archetype(
                row_id_temp1,
                tp(20),
                &MyPoints::update_fields().with_colors([MyColor(2)]),
            )
            .with_component(
                row_id_irrelevant,
                tp(25),
                // Some random components that aren't of interest to us!
                MyIndex::partial_descriptor(),
                &MyIndex(123),
            )?
            .with_archetype(
                row_id_temp2,
                if out_of_order { tp(10) } else { tp(30) },
                &MyPoints::new([MyPoint::new(2.0, 2.0)]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let requested_frame_id = TransformFrameIdHash::from_entity_path(&entity_path);

        let query_row_at_time = |t| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id,
                }),
                &atomic_component_set(),
            )?
            .row_id()
        };

        assert_eq!(query_row_at_time(0), None);
        if out_of_order {
            assert_eq!(query_row_at_time(10), Some(row_id_temp2));
            assert_eq!(query_row_at_time(15), Some(row_id_temp2));
            assert_eq!(query_row_at_time(20), Some(row_id_temp1));
            assert_eq!(query_row_at_time(25), Some(row_id_temp1));
            assert_eq!(query_row_at_time(30), Some(row_id_temp0));
            assert_eq!(query_row_at_time(35), Some(row_id_temp0));
        } else {
            assert_eq!(query_row_at_time(10), Some(row_id_temp0));
            assert_eq!(query_row_at_time(15), Some(row_id_temp0));
            assert_eq!(query_row_at_time(20), Some(row_id_temp1));
            assert_eq!(query_row_at_time(25), Some(row_id_temp1));
            assert_eq!(query_row_at_time(30), Some(row_id_temp2));
            assert_eq!(query_row_at_time(35), Some(row_id_temp2));
        }

        // The condition should not make any difference in this scenario!
        for t in [0, 10, 15, 20, 25, 30, 35] {
            assert_eq!(
                query_row_at_time(t),
                atomic_latest_at_query_test(
                    &entity_db,
                    &LatestAtQuery::new(*timeline().name(), t),
                    &entity_path,
                    None,
                    &atomic_component_set(),
                )
                .and_then(|chunk| chunk.row_id())
            );
        }

        // Any query with another frame should fail
        for t in [0, 15, 30, 40] {
            assert!(
                atomic_latest_at_query_test(
                    &entity_db,
                    &LatestAtQuery::new(*timeline().name(), t),
                    &entity_path,
                    Some(AtomicLatestAtFrameFilter {
                        condition_frame_id_component: frame_condition_component(),
                        requested_frame_id: TransformFrameIdHash::from_str("nope"),
                    }),
                    &atomic_component_set(),
                )
                .is_none()
            );
        }

        Ok(())
    }
    #[test]
    fn atomic_latest_at_temporal_only_no_frame_cond_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        atomic_latest_at_temporal_only_no_frames_present(false)
    }

    #[test]
    fn atomic_latest_at_temporal_only_no_frame_cond_out_of_order()
    -> Result<(), Box<dyn std::error::Error>> {
        atomic_latest_at_temporal_only_no_frames_present(true)
    }

    #[test]
    fn atomic_latest_at_static_and_temporal_no_frames_present()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        // Populate store.
        let entity_path = EntityPath::from("my_entity");
        let row_id_static0 = RowId::new();
        let row_id_static1 = RowId::new();
        let row_id_irrelevant = RowId::new();
        let row_id_temp = RowId::new();
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_static0,
                TimePoint::STATIC,
                &MyPoints::new([MyPoint::new(1.0, 1.0)]),
            )
            .with_archetype(
                row_id_static1,
                TimePoint::STATIC,
                &MyPoints::new([MyPoint::new(2.0, 2.0)]),
            )
            .with_component(
                row_id_irrelevant,
                TimePoint::STATIC,
                // Some random components that aren't of interest to us!
                MyIndex::partial_descriptor(),
                &MyIndex(123),
            )?
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_temp,
                tp(10),
                // Not allowed to write position & index, but color is fine since it wasn't written statically.
                &MyPoints::update_fields().with_colors([MyColor(1)]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let requested_frame_id = TransformFrameIdHash::from_entity_path(&entity_path);

        let query_row_at_time = |t| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id,
                }),
                &atomic_component_set(),
            )?
            .row_id()
        };

        assert_eq!(query_row_at_time(0), Some(row_id_static1));
        assert_eq!(query_row_at_time(10), Some(row_id_temp));
        assert_eq!(query_row_at_time(123), Some(row_id_temp));

        // Any query with another frame should fail
        assert!(
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), 0),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id: TransformFrameIdHash::from_str("nope"),
                }),
                &atomic_component_set(),
            )
            .is_none()
        );

        Ok(())
    }

    fn atomic_latest_at_temporal_only_with_frames(
        out_of_order: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        // Populate store.
        let entity_path = EntityPath::from("my_entity");
        let row_id_temp0 = RowId::new();
        let row_id_temp1 = RowId::new();
        let row_id_irrelevant = RowId::new();
        let row_id_temp2 = RowId::new();
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_temp0,
                if out_of_order { tp(30) } else { tp(10) },
                &MyPoints::new([MyPoint::new(1.0, 1.0)])
                    .with_colors([MyColor(1)])
                    .with_labels([MyLabel("first".to_owned())]),
            )
            .with_archetype(
                row_id_temp1,
                tp(20),
                &MyPoints::update_fields()
                    .with_colors([MyColor(2)])
                    .with_labels([MyLabel("second!".to_owned())]),
            )
            .with_component(
                row_id_irrelevant,
                tp(25),
                // Some random components that aren't of interest to us!
                MyIndex::partial_descriptor(),
                &MyIndex(123),
            )?
            .with_archetype(
                row_id_temp2,
                if out_of_order { tp(10) } else { tp(30) },
                &MyPoints::new([MyPoint::new(2.0, 2.0)]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let query_row = |t, label: &str| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id: TransformFrameIdHash::from_str(label),
                }),
                &atomic_component_set(),
            )?
            .row_id()
        };

        let query_row_no_cond = |t| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                None,
                &atomic_component_set(),
            )?
            .row_id()
        };

        assert_eq!(query_row(0, "first"), None);
        assert_eq!(query_row(0, "second!"), None);
        assert_eq!(query_row(0, "tf#/my_entity"), None);
        if out_of_order {
            assert_eq!(query_row(10, "first"), None);
            assert_eq!(query_row(20, "first"), None);
            assert_eq!(query_row(25, "first"), None);
            assert_eq!(query_row(35, "first"), Some(row_id_temp0));

            assert_eq!(query_row(10, "second!"), None);
            assert_eq!(query_row(20, "second!"), Some(row_id_temp1));
            assert_eq!(query_row(25, "second!"), Some(row_id_temp1));
            assert_eq!(query_row(35, "second!"), Some(row_id_temp1));

            assert_eq!(query_row(10, "tf#/my_entity"), Some(row_id_temp2));
            assert_eq!(query_row(20, "tf#/my_entity"), Some(row_id_temp2));
            assert_eq!(query_row(25, "tf#/my_entity"), Some(row_id_temp2));
            assert_eq!(query_row(35, "tf#/my_entity"), Some(row_id_temp2));

            assert_eq!(query_row_no_cond(10), Some(row_id_temp2));
            assert_eq!(query_row_no_cond(20), Some(row_id_temp1));
            assert_eq!(query_row_no_cond(25), Some(row_id_temp1));
            assert_eq!(query_row_no_cond(35), Some(row_id_temp0));
        } else {
            assert_eq!(query_row(10, "first"), Some(row_id_temp0));
            assert_eq!(query_row(20, "first"), Some(row_id_temp0));
            assert_eq!(query_row(25, "first"), Some(row_id_temp0));
            assert_eq!(query_row(35, "first"), Some(row_id_temp0));

            assert_eq!(query_row(10, "second!"), None);
            assert_eq!(query_row(20, "second!"), Some(row_id_temp1));
            assert_eq!(query_row(25, "second!"), Some(row_id_temp1));
            assert_eq!(query_row(35, "second!"), Some(row_id_temp1));

            assert_eq!(query_row(10, "tf#/my_entity"), None);
            assert_eq!(query_row(20, "tf#/my_entity"), None);
            assert_eq!(query_row(25, "tf#/my_entity"), None);
            assert_eq!(query_row(35, "tf#/my_entity"), Some(row_id_temp2));

            assert_eq!(query_row_no_cond(10), Some(row_id_temp0));
            assert_eq!(query_row_no_cond(20), Some(row_id_temp1));
            assert_eq!(query_row_no_cond(25), Some(row_id_temp1));
            assert_eq!(query_row_no_cond(35), Some(row_id_temp2));
        }

        Ok(())
    }
    #[test]
    fn atomic_latest_at_temporal_only_with_frames_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        atomic_latest_at_temporal_only_with_frames(false)
    }

    #[test]
    fn atomic_latest_at_temporal_only_with_frames_out_of_order()
    -> Result<(), Box<dyn std::error::Error>> {
        atomic_latest_at_temporal_only_with_frames(true)
    }

    #[test]
    fn atomic_latest_at_handle_simultaneous_events() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        // Populate store.
        let entity_path = EntityPath::from("my_entity");
        let row_id_temp0 = RowId::new();
        let row_id_temp1 = RowId::new();
        let row_id_irrelevant = RowId::new();
        let row_id_temp2 = RowId::new();

        let time = tp(10);

        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_temp0,
                time.clone(),
                &MyPoints::new([MyPoint::new(1.0, 1.0)])
                    .with_colors([MyColor(1)])
                    .with_labels([MyLabel("first".to_owned())]),
            )
            .with_archetype(
                row_id_temp1,
                time.clone(),
                &MyPoints::update_fields()
                    .with_colors([MyColor(2)])
                    .with_labels([MyLabel("second!".to_owned())]),
            )
            .with_component(
                row_id_irrelevant,
                time.clone(),
                // Some random components that aren't of interest to us!
                MyIndex::partial_descriptor(),
                &MyIndex(123),
            )?
            .with_archetype(
                row_id_temp2,
                time.clone(),
                &MyPoints::new([MyPoint::new(2.0, 2.0)]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let query_row = |t, label: &str| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id: TransformFrameIdHash::from_str(label),
                }),
                &atomic_component_set(),
            )?
            .row_id()
        };

        assert_eq!(query_row(0, "first"), None);
        assert_eq!(query_row(0, "second!"), None);
        assert_eq!(query_row(0, "tf#/my_entity"), None);
        assert_eq!(query_row(10, "first"), Some(row_id_temp0));
        assert_eq!(query_row(10, "second!"), Some(row_id_temp1));
        assert_eq!(query_row(10, "tf#/my_entity"), Some(row_id_temp2));

        Ok(())
    }

    #[test]
    fn atomic_latest_at_handle_empty_arrays() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(re_log_types::StoreInfo::testing().store_id);

        // Populate store.
        let entity_path = EntityPath::from("my_entity");
        let row_id_temp0 = RowId::new();
        let row_id_irrelevant = RowId::new();
        let row_id_temp1 = RowId::new();

        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(
                row_id_temp0,
                tp(10),
                &MyPoints::new([MyPoint::new(1.0, 1.0)])
                    .with_labels([MyLabel("myframe".to_owned())]),
            )
            .with_component(
                row_id_irrelevant,
                tp(20),
                // Some random components that aren't of interest to us!
                MyIndex::partial_descriptor(),
                &MyIndex(123),
            )?
            .with_archetype(
                row_id_temp1,
                tp(30),
                &MyPoints::update_fields().with_colors(std::iter::empty::<MyColor>()), // Empty array on a relevant component, still clears out things|
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        let query_row = |t, label: &str| {
            atomic_latest_at_query_test(
                &entity_db,
                &LatestAtQuery::new(*timeline().name(), t),
                &entity_path,
                Some(AtomicLatestAtFrameFilter {
                    condition_frame_id_component: frame_condition_component(),
                    requested_frame_id: TransformFrameIdHash::from_str(label),
                }),
                &atomic_component_set(),
            )?
            .row_id()
        };

        assert_eq!(query_row(0, "myframe"), None);
        assert_eq!(query_row(0, "tf#/my_entity"), None);
        assert_eq!(query_row(10, "myframe"), Some(row_id_temp0));
        assert_eq!(query_row(10, "tf#/my_entity"), None);
        assert_eq!(query_row(20, "myframe"), Some(row_id_temp0));
        assert_eq!(query_row(20, "tf#/my_entity"), None);
        assert_eq!(query_row(30, "myframe"), Some(row_id_temp0));
        assert_eq!(query_row(30, "tf#/my_entity"), Some(row_id_temp1));

        Ok(())
    }
}
