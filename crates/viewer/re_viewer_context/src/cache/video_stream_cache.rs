use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use ahash::HashMap;

use arrow::buffer::Buffer;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{EntityPath, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_log_types::EntityPathHash;
use re_types::{archetypes::VideoStream, components};
use re_video::decode::DecodeSettings;

use crate::Cache;

// ----------------------------------------------------------------------------

/// Video stream from the store, ready for playback.
///
/// This is compromised of:
/// * raw video stream data (pointers into all live rerun-chunks holding video frame data)
/// * metadata with that we know about the stream (where are I-frames etc.)
/// * active players for this stream and their state
#[derive(Clone)]
pub struct PlayableStoreVideoStream {
    // TODO: video needs to remain editable.
    pub video_renderer: Arc<re_renderer::video::Video>,
    pub video_sample_data: Vec<Buffer>,
}

impl PlayableStoreVideoStream {
    pub fn sample_buffer_slices(&self) -> Vec<&[u8]> {
        self.video_sample_data
            .iter()
            .map(|b| b.as_slice())
            .collect()
    }
}

/// Entry in the video stream cache.
///
/// Keeps track of usage so we know when to remove from the cache.
struct VideoStreamCacheEntry {
    used_this_frame: AtomicBool,
    video_stream: PlayableStoreVideoStream,
}

/// Identifies a video stream.

#[derive(Hash, Eq, PartialEq)]
struct VideoStreamKey {
    entity_path: EntityPathHash,
    timeline: TimelineName,
}

/// Caches metadata and active players for video streams.
///
/// It also keeps track of any additions and removals of video chunks.
#[derive(Default)]
pub struct VideoStreamCache(HashMap<VideoStreamKey, VideoStreamCacheEntry>);

#[derive(thiserror::Error, Debug)]
pub enum VideoStreamProcessingError {
    #[error("No video frame chunks found.")]
    NoVideoChunksFound,

    #[error("Frame chunks present, but arrow type but unexpected arrow type: {0:?}")]
    InvalidVideoChunkType(arrow::datatypes::DataType),

    #[error("Expected only a single video sample per timestep")]
    MultipleVideoSamplesPerTimestep,

    #[error("No codec specified.")]
    MissingCodec,

    #[error("Failed reading codec: {0}")]
    FailedReadingCodec(re_chunk::ChunkError),
}

impl VideoStreamCache {
    /// Looks up a video stream + players.
    ///
    /// Returns `None` if there was no video data for this entity on the given timeline.
    ///
    /// The first time a video stream that is looked up that isn't in the cache,
    /// it creates all the necessary metadata.
    /// For any stream in the cache, metadata will be kept automatically up to date for incoming
    /// and removed video frames chunks.
    pub fn entry(
        &mut self,
        store: &re_entity_db::EntityDb,
        entity_path: &EntityPath,
        timeline: TimelineName,
        decode_settings: DecodeSettings,
    ) -> Result<PlayableStoreVideoStream, VideoStreamProcessingError> {
        let key = VideoStreamKey {
            entity_path: entity_path.hash(),
            timeline,
        };

        let entry = match self.0.entry(key) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let (video_data, video_sample_data) =
                    load_video_data_from_chunks(store, entity_path, timeline)?;
                // TODO: video needs to remain editable.
                let video = re_renderer::video::Video::load(
                    entity_path.to_string(),
                    video_data,
                    decode_settings,
                );
                vacant_entry.insert(VideoStreamCacheEntry {
                    used_this_frame: AtomicBool::new(true),
                    video_stream: PlayableStoreVideoStream {
                        video_renderer: Arc::new(video),
                        video_sample_data,
                    },
                })
            }
        };

        // Using acquire/release here to be on the safe side and for semantical soundness:
        // Whatever thread is acquiring the fact that this was used, should also see/acquire
        // the side effect of having the entry contained in the cache.
        entry.used_this_frame.store(true, Ordering::Release);
        Ok(entry.video_stream.clone())
    }
}

fn load_video_data_from_chunks(
    store: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
    timeline: TimelineName,
) -> Result<(re_video::VideoDataDescription, Vec<Buffer>), VideoStreamProcessingError> {
    re_tracing::profile_function!();

    let frame_chunk_descr = VideoStream::descriptor_frame();
    let codec_chunk_descr = VideoStream::descriptor_codec();

    // Query for all video chunks on the **entire** timeline.
    // Tempting to bypass the query cache for this, but we don't expect to get new video chunks every frame
    // even for a running stream, so let's stick with the cache!
    //
    // TODO(andreas): Can we be more clever about the chunk range here and build up only what we need?
    // Kinda tricky since we need to know how far back (and ahead for b-frames) we have to look.
    let entire_timeline_query =
        re_chunk::RangeQuery::new(timeline, re_log_types::ResolvedTimeRange::EVERYTHING);
    let query_results = store.storage_engine().cache().range(
        &entire_timeline_query,
        entity_path,
        &[frame_chunk_descr.clone(), codec_chunk_descr.clone()],
    );
    let video_chunks = query_results
        .get_required(&frame_chunk_descr)
        .map_err(|_err| VideoStreamProcessingError::NoVideoChunksFound)?;
    let codec_chunks = query_results
        .get_required(&codec_chunk_descr)
        .map_err(|_err| VideoStreamProcessingError::MissingCodec)?;

    // Translate codec by looking at the last codec.
    // TODO(andreas): Should validate whether all codecs ever logged are the same, but it's a bit tedious.
    let last_codec = codec_chunks
        .last()
        .and_then(|chunk| {
            chunk.component_instance::<components::VideoCodec>(&codec_chunk_descr, 0, 0)
        })
        .ok_or(VideoStreamProcessingError::MissingCodec)?
        .map_err(VideoStreamProcessingError::FailedReadingCodec)?;
    let codec = match last_codec {
        components::VideoCodec::H264 => re_video::VideoCodec::H264,
        // components::VideoCodec::H265 => re_video::VideoCodec::H265,
        // components::VideoCodec::VP8 => re_video::VideoCodec::Vp8,
        // components::VideoCodec::VP9 => re_video::VideoCodec::Vp9,
        // components::VideoCodec::AV1 => re_video::VideoCodec::Av1,
    };

    // Extract all video samples.
    let mut chunk_buffers = Vec::with_capacity(video_chunks.len());
    let mut samples = Vec::with_capacity(video_chunks.len()); // Number of video chunks is minimum number of samples.
    let mut gops = Vec::new();

    for (chunk_idx, chunk) in video_chunks.iter().enumerate() {
        read_additional_samples_from_chunk(
            timeline,
            &frame_chunk_descr,
            chunk,
            chunk_idx,
            codec,
            &mut chunk_buffers,
            &mut samples,
            &mut gops,
        )?;
    }

    // Fill out frame durations.
    for sample in 0..samples.len().saturating_sub(1) {
        samples[sample].duration = Some(
            samples[sample + 1].presentation_timestamp - samples[sample].presentation_timestamp,
        );
    }

    Ok((
        re_video::VideoDataDescription {
            codec,
            stsd: None,
            coded_dimensions: None,                   // Unknown so far.
            timescale: re_video::Timescale::NO_SCALE, // We use raw rerun timestamps, so we don't have to scale time.
            duration: None, // Streams have to be assumed to be open ended, so we don't have a duration.
            gops,
            samples,
            samples_statistics: re_video::SamplesStatistics::NO_BFRAMES, // TODO(BFRAMETICKET): No b-frames for now.
            tracks: std::iter::once((0, Some(re_video::TrackKind::Video))).collect(),
        },
        chunk_buffers,
    ))
}

/// Reads all samples from a chunk and appends them to a list.
#[expect(clippy::too_many_arguments)]
fn read_additional_samples_from_chunk(
    timeline: TimelineName,
    frame_chunk_descr: &re_types::ComponentDescriptor,
    chunk: &re_chunk::Chunk,
    chunk_idx: usize,
    codec: re_video::VideoCodec,
    chunk_buffers: &mut Vec<Buffer>,
    samples: &mut Vec<re_video::Sample>,
    gops: &mut Vec<re_video::demux::GroupOfPictures>,
) -> Result<(), VideoStreamProcessingError> {
    re_tracing::profile_function!();

    let Some(raw_array) = chunk.raw_component_array(frame_chunk_descr) else {
        // This chunk doesn't have any video chunks.
        return Ok(());
    };

    let inner_list_array = raw_array
        .downcast_array_ref::<arrow::array::ListArray>()
        .ok_or(VideoStreamProcessingError::InvalidVideoChunkType(
            raw_array.data_type().clone(),
        ))?;
    let values = inner_list_array
        .values()
        .downcast_array_ref::<arrow::array::PrimitiveArray<arrow::array::types::UInt8Type>>()
        .ok_or(VideoStreamProcessingError::InvalidVideoChunkType(
            raw_array.data_type().clone(),
        ))?;
    let values = values.values().inner();
    chunk_buffers.push(values.clone());

    let offsets = inner_list_array.offsets();
    let lengths = re_arrow_util::offsets_lengths(inner_list_array.offsets()).collect::<Vec<_>>();
    let num_samples_before = samples.len();

    samples.extend(
        chunk
            .iter_component_offsets(frame_chunk_descr)
            .zip(chunk.iter_component_indices(&timeline, frame_chunk_descr))
            .map(move |((idx, len), (time, _row_id))| {
                debug_assert_eq!(len, 1, "Expected only a single video sample per timestep");

                let sample_idx = (num_samples_before + idx) as u32;
                let byte_offset = &offsets[idx..idx + len];
                let byte_length = &lengths[idx..idx + len];
                let decode_timestamp = re_video::Time(time.as_i64());

                // TODO:
                debug_assert_eq!(byte_offset.len(), 1);
                debug_assert_eq!(byte_length.len(), 1);
                let byte_offset = byte_offset[0] as u32;
                let byte_length = byte_length[0] as u32;

                let is_sync_result = re_video::is_sample_start_of_gop(
                    &values[byte_offset as usize..(byte_offset + byte_length) as usize],
                    codec,
                );
                // TODO(andreas): Do something with the error?
                // Detecting too few GOPs is better than too many since it merely means that the decoder has more work to do to get to a given frame.
                let is_sync = is_sync_result.unwrap_or(false);

                if is_sync {
                    // Last GOP extends until here now
                    if let Some(last_gop) = gops.last_mut() {
                        last_gop.sample_range.end = sample_idx;
                    }

                    // New gop starts at this frame.
                    gops.push(re_video::demux::GroupOfPictures {
                        decode_start_time: decode_timestamp,
                        sample_range: sample_idx..(sample_idx + 1),
                    });
                }

                re_video::Sample {
                    is_sync,

                    // TODO(BFRAMETICKET): No b-frames for now. Therefore sample_idx == frame_nr.
                    frame_nr: sample_idx,
                    // TODO(BFRAMETICKET): No b-frames for now. Therefore sample_idx == frame_nr.
                    decode_timestamp,
                    presentation_timestamp: decode_timestamp,

                    // Filled out later for everything but the last frame.
                    duration: None,

                    // We're using offsets directly into the chunk data.
                    buffer_index: chunk_idx as u32,
                    byte_offset,
                    byte_length,
                }
            }),
    );
    Ok(())
}

impl Cache for VideoStreamCache {
    fn begin_frame(&mut self, renderer_active_frame_idx: u64) {
        // TODO(andreas): This removal strategy is likely aggressive.
        // Scanning an entire video stream again is probably very costly. Have to evaluate.
        // Arguably it would be even better to keep this purging but not do full scans all the time.
        // (have some handwavy limit of number of samples around the current frame?)

        // Clean up unused video data.
        self.0
            .retain(|_, entry| entry.used_this_frame.load(Ordering::Acquire));

        // Of the remaining video data, remove all unused decoders.
        for entry in self.0.values_mut() {
            entry.used_this_frame.store(false, Ordering::Release);
            entry
                .video_stream
                .video_renderer
                .purge_unused_decoders(renderer_active_frame_idx);
        }
    }

    fn purge_memory(&mut self) {
        // We aggressively purge all unused video data every frame.
        // The expectation here is that parsing video data is fairly fast,
        // since decoding happens separately.
        //
        // As of writing, in a debug wasm build with Chrome loading a 600MiB 1h video
        // this assumption holds up fine: There is a (sufferable) delay,
        // but it's almost entirely due to the decoder trying to retrieve a frame.
    }

    fn on_store_events(&mut self, _events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        // TODO: handle adding and removing video chunks to the video data.
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
