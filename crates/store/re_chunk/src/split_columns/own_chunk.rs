//! Split out components that always belong in a chunk of their own.
//!
//! These are components marked `#[rerun(own_chunk)]` in their type definition.
//! They are typically small, and queried alone.
//!
//! This overrides the rule that keeps an archetype together (see
//! [`super::thick_thin`]): `VideoStream:is_keyframe` leaves the rest of `VideoStream`
//! behind.

use itertools::{Either, Itertools as _};

use re_types_core::reflection::ComponentTypeSet;

use crate::Chunk;

use super::SplitColumnsOptions;

/// Does this column hold a component that wants a chunk of its own?
#[inline]
fn wants_own_chunk(
    descriptor: &re_types_core::ComponentDescriptor,
    own_chunks: &ComponentTypeSet,
) -> bool {
    descriptor
        .component_type
        .is_some_and(|component_type| own_chunks.contains(&component_type))
}

/// Is `chunk` a chunk of its own for a component that wants one?
///
/// That is the only shape [`Chunk::split_columns`] emits for such a component, so it is the
/// only shape [`may_merge`] has to protect.
fn is_dedicated_own_chunk(chunk: &Chunk, own_chunks: &ComponentTypeSet) -> bool {
    let mut columns = chunk.components().values();
    let (Some(column), None) = (columns.next(), columns.next()) else {
        return false; // Zero or several columns, so dedicated to nothing.
    };

    wants_own_chunk(&column.descriptor, own_chunks)
}

/// May these two chunks be merged, or would that pull a component out of the chunk of its own?
///
/// [`Chunk::concatenable`] alone would allow it: it ignores the columns a chunk does not have in
/// common with the other, so it happily merges a dedicated marker chunk into a chunk that also
/// carries the samples.
///
/// Only a dedicated chunk is guarded, and only against gaining a column. Two dedicated marker
/// chunks still merge — that is how the markers end up in one single chunk. A chunk that was
/// logged with a marker sitting next to other components is left alone: separating those is
/// [`Chunk::split_columns`]'s job, and refusing those merges here would stop ordinary `VideoStream` chunks
/// from compacting while recording.
pub fn may_merge(lhs: &Chunk, rhs: &Chunk, own_chunks: &ComponentTypeSet) -> bool {
    if !is_dedicated_own_chunk(lhs, own_chunks) && !is_dedicated_own_chunk(rhs, own_chunks) {
        return true;
    }

    has_exactly_same_components(lhs, rhs)
}

fn has_exactly_same_components(lhs: &Chunk, rhs: &Chunk) -> bool {
    lhs.components().len() == rhs.components().len()
        && lhs.components().values().all(|column| {
            rhs.components()
                .contains_component(column.descriptor.component)
        })
}

/// Split `chunk` so that every component that wants a chunk of its own gets one.
///
/// The components that do not want one stay together in one last chunk, to be split further by
/// [`super::thick_thin`] if the caller asks for that.
///
/// Returns `None` if the chunk is already in that shape.
pub fn split(chunk: &Chunk, options: &SplitColumnsOptions) -> Option<Vec<Chunk>> {
    let SplitColumnsOptions {
        own_chunks,
        split_size_ratio: _, // handled by `Chunk::split_columns`
    } = options;

    let (wants_own_chunk, rest): (Vec<_>, Vec<_>) = chunk
        .components()
        .values()
        .map(|column| &column.descriptor)
        .partition_map(|descriptor| {
            if wants_own_chunk(descriptor, own_chunks) {
                Either::Left(descriptor.component)
            } else {
                Either::Right(descriptor.component)
            }
        });

    if wants_own_chunk.is_empty() || (wants_own_chunk.len() == 1 && rest.is_empty()) {
        return None; // Nothing to split out, or already a chunk of its own.
    }

    let mut splits: Vec<Chunk> = wants_own_chunk
        .iter()
        .map(|component| chunk.components_sliced(&[*component]))
        .collect();

    if !rest.is_empty() {
        splits.push(chunk.components_sliced(&rest));
    }

    re_log::trace!(
        entity = %chunk.entity_path(),
        num_splits = splits.len(),
        "splitting out components that want their own chunk"
    );

    Some(splits)
}

#[cfg(test)]
mod tests {
    use super::*;

    use re_log_types::{EntityPath, TimePoint, Timeline};
    use re_sdk_types::archetypes::VideoStream;
    use re_sdk_types::components::{IsKeyframe, VideoSample};
    use re_types_core::{Component as _, ComponentBatch, ComponentDescriptor};

    use crate::RowId;

    /// The real thing: `IsKeyframe` is the one component marked `#[rerun(own_chunk)]`.
    fn own_chunks() -> ComponentTypeSet {
        let mut own_chunks = ComponentTypeSet::default();
        own_chunks.insert(IsKeyframe::name());
        own_chunks
    }

    fn options() -> SplitColumnsOptions {
        SplitColumnsOptions {
            own_chunks: own_chunks(),
            ..Default::default()
        }
    }

    fn video_chunk(entity_path: &str, timepoint: TimePoint) -> Chunk {
        let sample = VideoSample::from(vec![0u8; 64 * 1024]);
        let is_keyframe = IsKeyframe::from(true);

        Chunk::builder(EntityPath::from(entity_path))
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    (
                        VideoStream::descriptor_sample(),
                        &[sample] as &dyn ComponentBatch,
                    ),
                    (
                        VideoStream::descriptor_is_keyframe(),
                        &[is_keyframe] as &dyn ComponentBatch,
                    ),
                ],
            )
            .build()
            .expect("failed to build chunk")
    }

    fn keyframe_chunk() -> Chunk {
        Chunk::builder(EntityPath::from("video"))
            .with_component_batches(
                RowId::new(),
                TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
                [(
                    VideoStream::descriptor_is_keyframe(),
                    &[IsKeyframe::from(true)] as &dyn ComponentBatch,
                )],
            )
            .build()
            .expect("failed to build chunk")
    }

    fn sample_chunk() -> Chunk {
        Chunk::builder(EntityPath::from("video"))
            .with_component_batches(
                RowId::new(),
                TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
                [(
                    VideoStream::descriptor_sample(),
                    &[VideoSample::from(vec![0u8; 1024])] as &dyn ComponentBatch,
                )],
            )
            .build()
            .expect("failed to build chunk")
    }

    /// `Chunk::concatenable` ignores the columns two chunks do not share, so it says yes to
    /// merging a marker chunk with a chunk holding the samples too. `may_merge` is what
    /// stops the store from undoing the split.
    #[test]
    fn refuses_to_merge_a_marker_chunk_with_anything_else() {
        let marker = keyframe_chunk();
        let mixed = video_chunk(
            "video",
            TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
        );

        assert!(
            marker.concatenable(&mixed),
            "the guard is only needed because `concatenable` allows this"
        );
        assert!(!may_merge(&marker, &mixed, &own_chunks()));
        assert!(!may_merge(&mixed, &marker, &own_chunks()));
        assert!(!may_merge(&marker, &sample_chunk(), &own_chunks()));
    }

    #[test]
    fn allows_merging_chunks_that_hold_the_same_components() {
        assert!(may_merge(
            &keyframe_chunk(),
            &keyframe_chunk(),
            &own_chunks()
        ));
        assert!(may_merge(&sample_chunk(), &sample_chunk(), &own_chunks()));

        // Never split, e.g. because it was logged straight from the SDK: merging two of those
        // does not put the marker next to anything it was not already next to.
        let timepoint = TimePoint::from([(Timeline::new_sequence("frame"), 1)]);
        assert!(may_merge(
            &video_chunk("video", timepoint.clone()),
            &video_chunk("video", timepoint),
            &own_chunks(),
        ));
    }

    #[test]
    fn splits_is_keyframe_out_of_its_archetype() {
        let timepoint = TimePoint::from([(Timeline::new_sequence("frame"), 1)]);
        let chunk = video_chunk("video", timepoint);

        let splits = split(&chunk, &options()).expect("should split");

        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].components().len(), 1);
        assert!(
            splits[0]
                .components()
                .contains_component(VideoStream::descriptor_is_keyframe().component),
            "the marker comes first, in a chunk of its own"
        );
        assert!(
            splits[1]
                .components()
                .contains_component(VideoStream::descriptor_sample().component)
        );

        for split in &splits {
            assert_eq!(split.num_rows(), chunk.num_rows());
        }
    }

    /// A static chunk is split like any other: the reader that wants the marker still
    /// should not have to fetch the samples.
    #[test]
    fn splits_from_a_static_chunk_too() {
        let chunk = video_chunk("video", TimePoint::default());

        assert!(chunk.is_static());
        assert_eq!(split(&chunk, &options()).expect("should split").len(), 2);
    }

    #[test]
    fn leaves_a_dedicated_chunk_alone() {
        let chunk = Chunk::builder(EntityPath::from("video"))
            .with_component_batches(
                RowId::new(),
                TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
                [(
                    VideoStream::descriptor_is_keyframe(),
                    &[IsKeyframe::from(true)] as &dyn ComponentBatch,
                )],
            )
            .build()
            .expect("failed to build chunk");

        assert!(split(&chunk, &options()).is_none());
    }

    #[test]
    fn leaves_unmarked_components_alone() {
        let chunk = Chunk::builder(EntityPath::from("video"))
            .with_component_batches(
                RowId::new(),
                TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
                [(
                    VideoStream::descriptor_sample(),
                    &[VideoSample::from(vec![0u8; 1024])] as &dyn ComponentBatch,
                )],
            )
            .build()
            .expect("failed to build chunk");

        assert!(split(&chunk, &options()).is_none());
    }

    /// A column with no component type cannot be looked up in reflection, so it stays put.
    #[test]
    fn leaves_untyped_columns_alone() {
        let chunk = Chunk::builder(EntityPath::from("video"))
            .with_component_batches(
                RowId::new(),
                TimePoint::from([(Timeline::new_sequence("frame"), 1)]),
                [
                    (
                        ComponentDescriptor {
                            archetype: Some("rerun.archetypes.VideoStream".into()),
                            component: "VideoStream:is_keyframe".into(),
                            component_type: None,
                        },
                        &[IsKeyframe::from(true)] as &dyn ComponentBatch,
                    ),
                    (
                        VideoStream::descriptor_sample(),
                        &[VideoSample::from(vec![0u8; 1024])] as &dyn ComponentBatch,
                    ),
                ],
            )
            .build()
            .expect("failed to build chunk");

        assert!(split(&chunk, &options()).is_none());
    }
}
