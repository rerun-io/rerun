//! Asset-mode chunk emission: lift of `re_importer::importer_archetype::load_video`.

use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint};
use re_log_types::{TimeType, Timeline, TimelineName};
use re_sdk_types::archetypes::{AssetVideo, VideoFrameReference};
use re_sdk_types::components::VideoTimestamp;

use crate::Mp4Error;

/// Lazy iterator over asset-mode chunks for an mp4 file.
///
/// Chunks are constructed one at a time in [`Iterator::next`] so that at most
/// one chunk is materialized in memory at once:
///
/// 1. the [`AssetVideo`] blob chunk,
/// 2. the [`VideoFrameReference`] index chunk — skipped (with a warning) if
///    frame-timestamp parsing failed.
pub struct AssetChunkIter {
    entity_path: EntityPath,
    timeline: Timeline,
    state: State,
}

enum State {
    /// Emit the [`AssetVideo`] blob chunk next.
    Asset {
        bytes: Vec<u8>,
        timepoint: TimePoint,
    },

    /// Emit the [`VideoFrameReference`] index chunk next.
    Index {
        frame_timestamps_nanos: Vec<i64>,
    },

    Done,
}

impl AssetChunkIter {
    /// `timepoint` is placed on the [`AssetVideo`] blob chunk. `timeline_name`
    /// and `timeline_type` name and type the timeline used for the
    /// [`VideoFrameReference`] index chunk. The emitted frame timestamps are PTS
    /// (durations since the start of the video) regardless of `timeline_type`;
    /// passing [`TimeType::TimestampNs`] only changes the declared timeline type
    /// and is meant to be paired with a downstream retag step.
    pub(crate) fn new(
        bytes: Vec<u8>,
        entity_path: &EntityPath,
        timeline_name: TimelineName,
        timeline_type: TimeType,
        timepoint: TimePoint,
    ) -> Result<Self, Mp4Error> {
        // TODO(#10929): remove this once the limit got fixed.
        if bytes.len() > i32::MAX as usize {
            return Err(Mp4Error::AssetTooLarge(bytes.len()));
        }

        Ok(Self {
            entity_path: entity_path.clone(),
            timeline: Timeline::new(timeline_name, timeline_type),
            state: State::Asset { bytes, timepoint },
        })
    }

    fn next_asset_chunk(
        &mut self,
        bytes: Vec<u8>,
        timepoint: TimePoint,
    ) -> Result<Chunk, Mp4Error> {
        re_tracing::profile_function!();

        let video_asset = {
            re_tracing::profile_scope!("serialize-as-arrow");
            AssetVideo::new(bytes)
        };

        // Read the (small) frame timestamps now, so the (large) blob does not
        // have to be kept alive until the next `next()` call.
        match video_asset.read_frame_timestamps_nanos() {
            Ok(frame_timestamps_nanos) => {
                self.state = State::Index {
                    frame_timestamps_nanos,
                };
            }
            Err(err) => {
                re_log::warn_once!(
                    "Failed to read frame timestamps from mp4 asset: {err} (entity path: {})",
                    self.entity_path
                );
                self.state = State::Done;
            }
        }

        // Put video asset into its own chunk since it can be fairly large.
        let chunk = Chunk::builder(self.entity_path.clone())
            .with_archetype(RowId::new(), timepoint, &video_asset)
            .build();
        if chunk.is_err() {
            // No point emitting an index chunk for an asset that failed to load.
            self.state = State::Done;
        }
        Ok(chunk?)
    }

    fn next_index_chunk(&self, frame_timestamps_nanos: Vec<i64>) -> Result<Chunk, Mp4Error> {
        re_tracing::profile_function!();

        let is_sorted = Some(true);
        let frame_timestamps_nanos: arrow::buffer::ScalarBuffer<i64> =
            frame_timestamps_nanos.into();
        let time_column = TimeColumn::new(is_sorted, self.timeline, frame_timestamps_nanos.clone());

        let video_timestamps = frame_timestamps_nanos
            .iter()
            .copied()
            .map(VideoTimestamp::from_nanos)
            .collect::<Vec<_>>();
        let video_timestamp_batch = &video_timestamps as &dyn re_sdk_types::ComponentBatch;
        let video_timestamp_list_array = video_timestamp_batch
            .to_arrow_list_array()
            .map_err(re_chunk::ChunkError::from)?;

        Ok(Chunk::from_auto_row_ids(
            ChunkId::new(),
            self.entity_path.clone(),
            std::iter::once((*self.timeline.name(), time_column)).collect(),
            std::iter::once((
                VideoFrameReference::descriptor_timestamp(),
                video_timestamp_list_array,
            ))
            .collect(),
        )?)
    }
}

impl Iterator for AssetChunkIter {
    type Item = Result<Chunk, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match std::mem::replace(&mut self.state, State::Done) {
            State::Asset { bytes, timepoint } => Some(self.next_asset_chunk(bytes, timepoint)),
            State::Index {
                frame_timestamps_nanos,
            } => Some(self.next_index_chunk(frame_timestamps_nanos)),
            State::Done => None,
        }
    }
}
