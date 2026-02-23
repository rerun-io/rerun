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
pub use self::resolved_pinhole_projection::ResolvedPinholeProjection;

use itertools::{Either, izip};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::Chunk;
use re_chunk_store::external::arrow;
use re_log_types::{TimeInt, TimelineName};
use re_sdk_types::ComponentIdentifier;

use crate::TransformFrameIdHash;

/// Iterates over all frames of a given component type that are in a chunk.
///
/// If the chunk is static, `timeline` will be ignored.
///
/// Yields an entry for every row. Note that there may be many entries per time though.
/// (Currently, there can be only a single frame id per row)
fn iter_child_frames_in_chunk(
    chunk: &Chunk,
    timeline: TimelineName,
    frame_component: ComponentIdentifier,
) -> impl Iterator<Item = (TimeInt, TransformFrameIdHash)> {
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

            Either::Right(izip!(offsets, lengths).map(move |(offset, length)| {
                // No need to check for nulls since we treat nulls, empty arrays, and empty strings all as the implicit frame.
                if length == 0 || values.value(offset).is_empty() {
                    implicit_frame
                } else {
                    // There can only be a single frame id per row today, so only look at the first element.
                    TransformFrameIdHash::from_str(values.value(offset))
                }
            }))
        }
    );

    izip!(
        chunk.iter_indices(&timeline).map(|(t, _)| t),
        frame_ids_per_row
    )
}
