mod cache;
mod cached_transform_value;
mod cached_transforms_for_timeline;
mod parent_from_child_transform;
mod pose_transform_for_entity;
mod resolved_pinhole_projection;
mod transforms_for_child_frame_events;
mod tree_transforms_for_child_frame;

#[cfg(test)]
mod tests;

pub use self::cache::TransformResolutionCache;
pub use self::cached_transforms_for_timeline::CachedTransformsForTimeline;
pub use self::parent_from_child_transform::ParentFromChildTransform;
pub use self::resolved_pinhole_projection::{
    ResolvedPinholeProjection, ResolvedPinholeProjectionCached,
};

use arrow::array::Array as _;
use itertools::{Either, izip};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::Chunk;
use re_chunk_store::external::arrow;
use re_log_types::{TimeInt, TimelineName};
use re_sdk_types::{ComponentIdentifier, RowId};

use crate::TransformFrameIdHash;

/// Iterates over all relevant rows in a chunk in a given timeline, resolving the child frames for each.
///
/// If the chunk is static, `timeline` will be ignored.
///
/// Yields an entry for every row where at least one out of `relevant_components` is non-null (even if the `frame_component` is null on that row).
/// Note that there may be many entries per time though.
/// (Currently, there can be only a single frame id per row)
fn iter_relevant_rows_in_chunk_with_child_frames<'a>(
    chunk: &'a Chunk,
    timeline: TimelineName,
    frame_component: ComponentIdentifier,
    relevant_components: &'static [ComponentIdentifier],
) -> impl Iterator<Item = ((TimeInt, RowId), TransformFrameIdHash)> + 'a {
    let implicit_frame = TransformFrameIdHash::from_entity_path(chunk.entity_path());

    // This is similar to `iter_slices` but it also yields elements for rows where the component is null.
    let frame_ids_per_row =
    chunk.components().get_array(frame_component).map_or_else(
        || Either::Left(std::iter::repeat(implicit_frame)),
        |list_array| {
            let values_raw = list_array.values();
            let Some(values) =
                values_raw.downcast_array_ref::<arrow::array::StringArray>()
            else {
                re_log::error_once!("Expected at {frame_component:?} @ {:?} to be a string array, but its type is instead {:?}",
                                         chunk.entity_path(), values_raw.data_type());
                return Either::Left(std::iter::repeat(implicit_frame));
            };

            let offsets = list_array.offsets().iter().map(|idx| *idx as usize);
            let lengths = list_array.offsets().lengths();

            Either::Right(izip!(offsets, lengths).filter_map(move |(offset, length)| {
                // No need to check for nulls since we treat nulls and empty arrays both as the implicit frame.
                if length == 0 {
                    Some(implicit_frame)
                } else {
                    // There can only be a single frame id per row today, so only look at the first element.
                    let frame_id = values.value(offset);
                    if frame_id.is_empty() {
                        // Special case: we have a frame id value, but it's an empty string.
                        // Empty explicit frame names are undefined and thus ignored here.
                        // (see related errors / warnings that are shown in this case elsewhere)
                        None
                    } else {
                        Some(TransformFrameIdHash::from_str(frame_id))
                    }
                }
            }))
        }
    );

    let relevant_chunk_chunk_arrays = relevant_components
        .iter()
        .filter_map(|component| chunk.components().get_array(*component))
        .collect::<Vec<_>>();

    izip!(chunk.iter_indices(&timeline), frame_ids_per_row)
        .enumerate()
        .filter(move |(index, _)| {
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
            relevant_chunk_chunk_arrays
                .iter()
                .any(|array| !array.is_null(*index))
        })
        .map(|(_, values)| values)
}

/// Iterates over relevant rows of a chunk in a given timeline.
///
/// If the chunk is static, `timeline` will be ignored.
///
/// Yields an entry for every row where at least one out of `relevant_components` is non-null.
/// Note that there may be many entries per time though.
fn iter_relevant_rows_in_chunk<'a>(
    chunk: &'a Chunk,
    timeline: TimelineName,
    relevant_components: &'static [ComponentIdentifier],
) -> impl Iterator<Item = (TimeInt, RowId)> + 'a {
    let relevant_chunk_chunk_arrays = relevant_components
        .iter()
        .filter_map(|component| chunk.components().get_array(*component))
        .collect::<Vec<_>>();

    chunk
        .iter_indices(&timeline)
        .enumerate()
        .filter(move |(index, _)| {
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
            relevant_chunk_chunk_arrays
                .iter()
                .any(|array| !array.is_null(*index))
        })
        .map(|(_, values)| values)
}

#[cfg(test)]
mod iterator_tests {
    use re_chunk_store::Chunk;
    use re_log_types::{
        EntityPath, TimeInt, Timeline,
        example_components::{MyPoint, MyPoints},
    };
    use re_sdk_types::{
        archetypes::{self, Pinhole, Transform3D},
        components::PinholeProjection,
    };

    use super::{iter_relevant_rows_in_chunk, iter_relevant_rows_in_chunk_with_child_frames};
    use crate::{TransformFrameIdHash, transform_queries};

    #[test]
    fn iter_relevant_rows_in_chunk_with_child_frames_skips_unrelated_rows_and_uses_implicit_frame()
    -> Result<(), Box<dyn std::error::Error>> {
        let timeline = Timeline::new_sequence("t");
        let entity_path = EntityPath::from("my_entity");
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]).with_child_frame("explicit_frame"),
            )
            .with_archetype_auto_row([(timeline, 2)], &MyPoints::new([MyPoint::new(1.0, 2.0)]))
            .with_archetype_auto_row([(timeline, 3)], &Transform3D::clear_fields())
            .with_archetype_auto_row([(timeline, 4)], &Transform3D::from_scale([2.0, 3.0, 4.0]))
            .build()?;

        let row_ids = chunk.row_ids_slice().to_vec();
        let relevant_rows = iter_relevant_rows_in_chunk_with_child_frames(
            &chunk,
            *timeline.name(),
            archetypes::Transform3D::descriptor_child_frame().component,
            transform_queries::atomic_component_set_for_tree_transforms(),
        )
        .collect::<Vec<_>>();

        assert_eq!(
            relevant_rows,
            vec![
                (
                    (TimeInt::new_temporal(1), row_ids[0]),
                    TransformFrameIdHash::from_str("explicit_frame"),
                ),
                (
                    (TimeInt::new_temporal(3), row_ids[2]),
                    TransformFrameIdHash::from_entity_path(&entity_path),
                ),
                (
                    (TimeInt::new_temporal(4), row_ids[3]),
                    TransformFrameIdHash::from_entity_path(&entity_path),
                ),
            ]
        );

        Ok(())
    }

    #[test]
    fn iter_relevant_rows_in_chunk_skips_unrelated_rows() -> Result<(), Box<dyn std::error::Error>>
    {
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Pinhole::new(PinholeProjection::from_focal_length_and_principal_point(
                    [1.0, 2.0],
                    [3.0, 4.0],
                )),
            )
            .with_archetype_auto_row([(timeline, 2)], &MyPoints::new([MyPoint::new(1.0, 2.0)]))
            .with_archetype_auto_row([(timeline, 3)], &Pinhole::clear_fields())
            .build()?;

        let row_ids = chunk.row_ids_slice().to_vec();
        let relevant_rows = iter_relevant_rows_in_chunk(
            &chunk,
            *timeline.name(),
            transform_queries::atomic_component_set_for_pinhole_projection(),
        )
        .collect::<Vec<_>>();

        assert_eq!(
            relevant_rows,
            vec![
                (TimeInt::new_temporal(1), row_ids[0]),
                (TimeInt::new_temporal(3), row_ids[2]),
            ]
        );

        Ok(())
    }
}
