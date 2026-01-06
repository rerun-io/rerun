use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ahash::HashMap;
use arrow::buffer::Buffer as ArrowBuffer;
use egui::NumExt as _;
use parking_lot::RwLock;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_byte_size::SizeBytes as _;
use re_chunk::{ChunkId, EntityPath, Span, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::{EntityPathHash, TimeType};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components;
use re_video::{DecodeSettings, StableIndexDeque};

use crate::{Cache, CacheMemoryReport};

/// A buffer of multiple video sample data from the datastore.
///
/// It's essentially a pointer into a column of [`re_sdk_types::components::VideoSample`]s inside a Rerun chunk.
struct SampleBuffer {
    buffer: ArrowBuffer,
    source_chunk_id: ChunkId,

    /// Indexes into [`re_video::VideoDataDescription::samples`] that this buffer contains.
    sample_index_range: std::ops::Range<re_video::SampleIndex>,
}

impl re_byte_size::SizeBytes for SampleBuffer {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            buffer: _, // ref-counted - already counted in the store
            source_chunk_id: _,
            sample_index_range: _,
        } = self;
        0
    }
}

/// Video stream from the store, ready for playback.
///
/// This is compromised of:
/// * raw video stream data (pointers into all live rerun-chunks holding video frame data)
/// * metadata with that we know about the stream (where are I-frames etc.)
/// * active players for this stream and their state
pub struct PlayableVideoStream {
    pub video_renderer: re_renderer::video::Video,

    /// All buffers (each mapping 1:1 to a rerun chunk) that have samples for this video stream.
    video_sample_buffers: StableIndexDeque<SampleBuffer>,
}

impl re_byte_size::SizeBytes for PlayableVideoStream {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            video_renderer,
            video_sample_buffers,
        } = self;
        video_renderer.heap_size_bytes() + video_sample_buffers.heap_size_bytes()
    }
}

impl PlayableVideoStream {
    pub fn sample_buffers(&self) -> StableIndexDeque<&[u8]> {
        StableIndexDeque::from_iter_with_offset(
            self.video_sample_buffers.min_index(),
            self.video_sample_buffers
                .iter()
                .map(|b| b.buffer.as_slice()),
        )
    }

    pub fn video_descr(&self) -> &re_video::VideoDataDescription {
        self.video_renderer.data_descr()
    }
}

/// Entry in the video stream cache.
///
/// Keeps track of usage so we know when to remove from the cache.
struct VideoStreamCacheEntry {
    used_this_frame: AtomicBool,
    video_stream: Arc<RwLock<PlayableVideoStream>>,
}

impl re_byte_size::SizeBytes for VideoStreamCacheEntry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            used_this_frame: _,
            video_stream,
        } = self;

        video_stream.read().heap_size_bytes()
    }
}

/// Identifies a video stream.

#[derive(Hash, Eq, PartialEq)]
struct VideoStreamKey {
    entity_path: EntityPathHash,
    timeline: TimelineName,
}

impl re_byte_size::SizeBytes for VideoStreamKey {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path,
            timeline,
        } = self;
        entity_path.heap_size_bytes() + timeline.heap_size_bytes()
    }
}

/// Caches metadata and active players for video streams.
///
/// It also keeps track of any additions and removals of video chunks.
#[derive(Default)]
pub struct VideoStreamCache(HashMap<VideoStreamKey, VideoStreamCacheEntry>);

#[derive(thiserror::Error, Debug)]
pub enum VideoStreamProcessingError {
    #[error("No video samples.")]
    NoVideoSamplesFound,

    #[error("Unexpected arrow type for video sample {0:?}")]
    InvalidVideoSampleType(arrow::datatypes::DataType),

    #[error("No codec specified.")]
    MissingCodec,

    #[error("Failed to read codec - {0}")]
    FailedReadingCodec(Box<re_chunk::ChunkError>),

    #[error("Received video samples were not in chronological order.")]
    OutOfOrderSamples,
}

const _: () = assert!(
    std::mem::size_of::<VideoStreamProcessingError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

pub type SharablePlayableVideoStream = Arc<RwLock<PlayableVideoStream>>;

impl VideoStreamCache {
    /// Looks up a video stream + players.
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
    ) -> Result<SharablePlayableVideoStream, VideoStreamProcessingError> {
        let key = VideoStreamKey {
            entity_path: entity_path.hash(),
            timeline,
        };

        let entry = match self.0.entry(key) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let (video_data, video_sample_buffers) =
                    load_video_data_from_chunks(store, entity_path, timeline)?;
                let video = re_renderer::video::Video::load(
                    entity_path.to_string(),
                    video_data,
                    decode_settings,
                );
                vacant_entry.insert(VideoStreamCacheEntry {
                    used_this_frame: AtomicBool::new(true),
                    video_stream: Arc::new(RwLock::new(PlayableVideoStream {
                        video_renderer: video,
                        video_sample_buffers,
                    })),
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
) -> Result<
    (
        re_video::VideoDataDescription,
        StableIndexDeque<SampleBuffer>,
    ),
    VideoStreamProcessingError,
> {
    re_tracing::profile_function!();

    let sample_component = VideoStream::descriptor_sample().component;
    let codec_component = VideoStream::descriptor_codec().component;

    // Query for all video chunks on the **entire** timeline.
    // Tempting to bypass the query cache for this, but we don't expect to get new video chunks every frame
    // even for a running stream, so let's stick with the cache!
    //
    // TODO(andreas): Can we be more clever about the chunk range here and build up only what we need?
    // Kinda tricky since we need to know how far back (and ahead for b-frames) we have to look.
    let entire_timeline_query =
        re_chunk::RangeQuery::new(timeline, re_log_types::AbsoluteTimeRange::EVERYTHING);
    let query_results = store.storage_engine().cache().range(
        &entire_timeline_query,
        entity_path,
        [sample_component, codec_component],
    );
    let sample_chunks = query_results
        .get_required(sample_component)
        .map_err(|_err| VideoStreamProcessingError::NoVideoSamplesFound)?;
    let codec_chunks = query_results
        .get_required(codec_component)
        .map_err(|_err| VideoStreamProcessingError::MissingCodec)?;

    // Translate codec by looking at the last codec.
    // TODO(andreas): Should validate whether all codecs ever logged are the same, but it's a bit tedious.
    let last_codec = codec_chunks
        .last()
        .and_then(|chunk| chunk.component_instance::<components::VideoCodec>(codec_component, 0, 0))
        .ok_or(VideoStreamProcessingError::MissingCodec)?
        .map_err(|err| VideoStreamProcessingError::FailedReadingCodec(Box::new(err)))?;
    let codec = last_codec.into();

    // Extract all video samples.
    let mut video_sample_buffers = StableIndexDeque::new();
    let mut video_descr = re_video::VideoDataDescription {
        codec,
        encoding_details: None, // Unknown so far, we'll find out later.
        timescale: timescale_for_timeline(store, timeline),
        delivery_method: re_video::VideoDeliveryMethod::new_stream(),
        gops: StableIndexDeque::new(),
        samples: StableIndexDeque::with_capacity(sample_chunks.len()), // Number of video chunks is minimum number of samples.
        samples_statistics: re_video::SamplesStatistics::NO_BFRAMES, // TODO(#10090): No b-frames for now.
        mp4_tracks: Default::default(),
    };

    for chunk in sample_chunks {
        if let Err(err) =
            read_samples_from_chunk(timeline, chunk, &mut video_descr, &mut video_sample_buffers)
        {
            match err {
                VideoStreamProcessingError::OutOfOrderSamples => {
                    re_log::warn_once!(
                        "Late insertions of video frames within an established video stream is not supported, some video data has been ignored."
                    );
                }
                err => return Err(err),
            }
        }
    }

    Ok((video_descr, video_sample_buffers))
}

fn timescale_for_timeline(
    store: &re_entity_db::EntityDb,
    timeline: TimelineName,
) -> Option<re_video::Timescale> {
    let timeline_typ = store.timelines().get(&timeline).map(|t| t.typ());
    match timeline_typ {
        Some(TimeType::Sequence) | None => None, // Can't translate the sequence time to real durations
        Some(TimeType::DurationNs | TimeType::TimestampNs) => Some(re_video::Timescale::NANOSECOND),
    }
}

/// Reads all video samples from a chunk into an existing video description.
///
/// Rejects out of order samples - new samples must have a higher timestamp than the previous ones.
/// Since samples within a chunk are guaranteed to be ordered, this can only happen if a new chunk
/// is inserted that has is timestamped to be older than the data in the last added chunk.
///
/// Encoding details are automatically updated whenever detected.
/// Changes of encoding details over time will trigger a warning.
fn read_samples_from_chunk(
    timeline: TimelineName,
    chunk: &re_chunk::Chunk,
    video_descr: &mut re_video::VideoDataDescription,
    chunk_buffers: &mut StableIndexDeque<SampleBuffer>,
) -> Result<(), VideoStreamProcessingError> {
    re_tracing::profile_function!();

    let re_video::VideoDataDescription {
        codec,
        samples,
        gops,
        encoding_details,
        ..
    } = video_descr;

    let sample_component = VideoStream::descriptor_sample().component;
    let Some(raw_array) = chunk.raw_component_array(sample_component) else {
        // This chunk doesn't have any video chunks.
        return Ok(());
    };

    let mut previous_max_presentation_timestamp = samples
        .back()
        .map_or(re_video::Time::MIN, |s| s.presentation_timestamp);

    // Validate whether this chunk is an insertion into existing data.
    // If so, discard it and warn the user.
    let time_ranges = chunk.time_range_per_component();
    match time_ranges
        .get(&timeline)
        .and_then(|time_range| time_range.get(&sample_component))
    {
        Some(time_range) => {
            if time_range.min().as_i64() < previous_max_presentation_timestamp.0 {
                return Err(VideoStreamProcessingError::OutOfOrderSamples);
            }
        }
        None => {
            // This chunk doesn't have any data on this timeline.
            return Ok(());
        }
    }

    // Make sure our index is sorted by the timeline we're interested in.
    let chunk = chunk.sorted_by_timeline_if_unsorted(&timeline);

    // The underlying data within a chunk is logically a Vec<Vec<Blob>>,
    // where the inner Vec always has a len=1, because we're dealing with a "mono-component"
    // (each VideoStream has exactly one VideoSample instance per time)`.
    //
    // Because of how arrow works, the bytes of all the blobs are actually sequential in memory (yay!) in a single buffer,
    // what you call values below (could use a better name btw).
    //
    // We want to figure out the byte offsets of each blob within the arrow buffer that holds all the blobs,
    // i.e. get out a Vec<ByteRange>.
    let inner_list_array = raw_array
        .downcast_array_ref::<arrow::array::ListArray>()
        .ok_or(VideoStreamProcessingError::InvalidVideoSampleType(
            raw_array.data_type().clone(),
        ))?;
    let values = inner_list_array
        .values()
        .downcast_array_ref::<arrow::array::PrimitiveArray<arrow::array::types::UInt8Type>>()
        .ok_or(VideoStreamProcessingError::InvalidVideoSampleType(
            raw_array.data_type().clone(),
        ))?;
    let values = values.values().inner();

    let offsets = inner_list_array.offsets();
    let lengths = offsets.lengths().collect::<Vec<_>>();

    let buffer_index = chunk_buffers.next_index();
    let sample_base_idx = samples.next_index();

    // Extract sample metadata.
    samples.extend(
        chunk
            .iter_component_offsets(sample_component)
            .zip(chunk.iter_component_indices(timeline, sample_component))
            .enumerate()
            .filter_map(move |(idx, (component_offset, (time, _row_id)))| {
                if component_offset.len == 0 {
                    // Ignore empty samples.
                    return None;
                }
                if component_offset.len != 1 {
                    re_log::warn_once!(
                        "Expected only a single VideoSample per row (it is a mono-component)"
                    );
                    return None;
                }

                // Do **not** use the `component_offset.start` for determining the sample index
                // as it is only for the offset in the underlying arrow arrays which means that
                // it may in theory step arbitrarily through the data.
                let sample_idx = sample_base_idx + idx;

                let byte_span = Span { start:offsets[component_offset.start] as usize, len: lengths[component_offset.start] };
                let sample_bytes = &values[byte_span.range()];

                // Note that the conversion of this time value is already handled by `VideoDataDescription::timescale`:
                // For sequence time we use a scale of 1, for nanoseconds time we use a scale of 1_000_000_000.
                let decode_timestamp = re_video::Time(time.as_i64());

                // Samples within a chunk are expected to be always in order since we called `chunk.sorted_by_timeline_if_unsorted` earlier.
                //
                // Equality means that we have two samples falling onto the same time.
                // This is strange, but we allow it since decoders are fine with it (they care little about exact times)
                // and this may well happen in practice, in fact it can be spuriously observed in the video streaming example.
                debug_assert!(decode_timestamp >= previous_max_presentation_timestamp);
                previous_max_presentation_timestamp = decode_timestamp;

                let is_sync = match re_video::detect_gop_start(sample_bytes, *codec) {
                    Ok(re_video::GopStartDetection::StartOfGop(new_encoding_details)) => {
                        if encoding_details.as_ref() != Some(&new_encoding_details) {
                            if let Some(old_encoding_details) = encoding_details.as_ref() {
                                re_log::warn_once!(
                                    "Detected change of video encoding properties (like size, bit depth, compression etc.) over time. \
                                    This is not supported and may cause playback issues."
                                );
                                re_log::trace!(
                                    "Previous encoding details: {:?}\n\nNew encoding details: {:?}",
                                    old_encoding_details,
                                    new_encoding_details
                                );
                            }
                            *encoding_details = Some(new_encoding_details);
                        }

                        true
                    }
                    Ok(re_video::GopStartDetection::NotStartOfGop) => { false },

                    Err(err) => {
                        re_log::error_once!("Failed to detect GOP for video sample: {err}");
                        false
                    }
                };

                if is_sync {
                    // New gop starts at this frame.
                    gops.push_back(re_video::GroupOfPictures {
                        sample_range: sample_idx..(sample_idx + 1),
                    });
                } else {
                    // Last GOP extends until here now, including the current sample.
                    if let Some(last_gop) = gops.back_mut() {
                        last_gop.sample_range.end = sample_idx + 1;
                    }
                }

                let Some(byte_span) = byte_span.try_cast::<u32>() else {
                    re_log::warn_once!("Video byte range does not fit in u32: {byte_span:?}");
                    return None;
                };

                Some(re_video::SampleMetadata {
                    is_sync,

                    // TODO(#10090): No b-frames for now. Therefore sample_idx == frame_nr.
                    frame_nr: sample_idx as u32,
                    decode_timestamp,
                    presentation_timestamp: decode_timestamp,

                    // Filled out later for everything but the last frame.
                    duration: None,

                    // We're using offsets directly into the chunk data.
                    buffer_index,
                    byte_span
                })
            }),
    );

    // Any new samples actually added? Early out if not.
    if sample_base_idx == samples.next_index() {
        return Ok(());
    }

    // Fill out durations for all new samples plus the first existing sample for which we didn't know the duration yet.
    // (We set the duration for the last sample to `None` since we don't know how long it will last.)
    // (Note that we can't use tuple_windows here because it can't handle mutable references)
    {
        let start = sample_base_idx
            .saturating_sub(1)
            .at_least(samples.min_index());
        let end = samples.next_index().saturating_sub(1);
        for sample in start..end {
            samples[sample].duration = Some(
                samples[sample + 1].presentation_timestamp - samples[sample].presentation_timestamp,
            );
        }
    }

    // Sanity checks on chunk buffers.
    if let Some(last_buffer) = chunk_buffers.back() {
        debug_assert_eq!(
            last_buffer.sample_index_range.end, sample_base_idx,
            "Sample range of each chunk buffer must be non-overlapping and without gaps."
        );
    }

    chunk_buffers.push_back(SampleBuffer {
        buffer: values.clone(),
        source_chunk_id: chunk.id(),
        sample_index_range: sample_base_idx..samples.next_index(),
    });

    if cfg!(debug_assertions)
        && let Err(err) = video_descr.sanity_check()
    {
        panic!(
            "VideoDataDescription sanity check failed for video stream at {:?}: {err}",
            chunk.entity_path()
        );
    }

    Ok(())
}

impl Cache for VideoStreamCache {
    fn begin_frame(&mut self) {
        // TODO(andreas): This removal strategy is likely aggressive.
        // Scanning an entire video stream again is probably very costly. Have to evaluate.
        // Arguably it would be even better to keep this purging but not do full scans all the time.
        // (have some handwavy limit of number of samples around the current frame?)

        // Clean up unused video data.
        self.0
            .retain(|_, entry| entry.used_this_frame.load(Ordering::Acquire));

        // Of the remaining video data, remove all unused decoders.
        #[expect(clippy::iter_over_hash_type)]
        for entry in self.0.values_mut() {
            entry.used_this_frame.store(false, Ordering::Release);
            let video_stream = entry.video_stream.write();
            video_stream.video_renderer.begin_frame();
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

    fn memory_report(&self) -> CacheMemoryReport {
        CacheMemoryReport {
            bytes_cpu: self.0.total_size_bytes(),
            bytes_gpu: None,
            per_cache_item_info: Vec::new(),
        }
    }

    fn name(&self) -> &'static str {
        "Video Streams"
    }

    /// Keep existing cache entries up to date with new and removed video data.
    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let sample_component = VideoStream::descriptor_sample().component;

        for event in events {
            if !event
                .chunk
                .components()
                .contains_component(sample_component)
            {
                continue;
            }

            #[expect(clippy::iter_over_hash_type)] //  TODO(#6198): verify that this is fine
            for timeline in event.chunk.timelines().keys() {
                let key = VideoStreamKey {
                    entity_path: event.chunk.entity_path().hash(),
                    timeline: *timeline,
                };
                let Some(entry) = self.0.get_mut(&key) else {
                    // If we don't have a cache entry yet, we can skip over this event as there's nothing to update.
                    continue;
                };

                let mut video_stream = entry.video_stream.write();
                let PlayableVideoStream {
                    video_renderer,
                    video_sample_buffers,
                } = &mut *video_stream;
                let video_data = video_renderer.data_descr_mut();
                video_data.delivery_method = re_video::VideoDeliveryMethod::new_stream();

                match event.kind {
                    re_chunk_store::ChunkStoreDiffKind::Addition => {
                        // If this came with a compaction, throw out all samples and gops that were compacted away and restart there.
                        // This is a bit slower than re-using all the data, but much simpler and more robust.
                        //
                        // Compactions events are document to happen on **addition** only.
                        // Therefore, it should be save to remove only the newest data.
                        let chunk = if let Some(compaction) = &event.compacted {
                            for chunk_id in compaction.srcs.keys() {
                                if let Some(first_invalid_buffer_idx) = video_sample_buffers
                                    .position(|buffer| buffer.source_chunk_id == *chunk_id)
                                {
                                    // Remove all samples that are in this and future buffers.
                                    video_data.samples.remove_all_with_index_larger_equal(
                                        video_sample_buffers[first_invalid_buffer_idx]
                                            .sample_index_range
                                            .start,
                                    );

                                    // Remove all buffers starting with the found matching one.
                                    // (does nothing if we haven't found one)
                                    video_sample_buffers.remove_all_with_index_larger_equal(
                                        first_invalid_buffer_idx,
                                    );
                                }
                            }

                            adjust_gops_for_removed_samples_back(video_data);

                            // `event.chunk` is added data PRIOR to compaction.
                            &compaction.new_chunk
                        } else {
                            &event.chunk
                        };

                        let encoding_details_before = video_data.encoding_details.clone();

                        if let Err(err) = read_samples_from_chunk(
                            *timeline,
                            chunk,
                            video_data,
                            video_sample_buffers,
                        ) {
                            match err {
                                VideoStreamProcessingError::OutOfOrderSamples => {
                                    drop(video_stream);
                                    // We found out of order samples, discard this video stream cache entry
                                    // to reconstruct it with all data in mind.
                                    self.0.remove(&key);
                                    continue;
                                }
                                err => {
                                    re_log::error_once!(
                                        "Failed to read process additional incoming video samples: {err}"
                                    );
                                }
                            }
                        }

                        if encoding_details_before != video_data.encoding_details {
                            re_log::error_once!(
                                "The video stream codec details on {} changed over time, which is not supported.",
                                event.chunk.entity_path()
                            );
                            video_renderer.reset_all_decoders();
                        }
                    }
                    re_chunk_store::ChunkStoreDiffKind::Deletion => {
                        // Chunk deletion typically happens at the start of the recording due to garbage collection.
                        // Even if it were to happen somewhere in the middle of the stream,
                        // we'd still want to delete all prior samples & buffers since we can't handle gaps
                        // in the video stream.
                        if let Some(last_invalid_buffer_idx) = video_sample_buffers
                            .position(|buffer| buffer.source_chunk_id == event.chunk.id())
                        {
                            let last_invalid_buffer =
                                &video_sample_buffers[last_invalid_buffer_idx];
                            let last_invalid_sample_idx =
                                last_invalid_buffer.sample_index_range.end.saturating_sub(1);

                            video_data
                                .samples
                                .remove_all_with_index_smaller_equal(last_invalid_sample_idx);
                            video_sample_buffers
                                .remove_all_with_index_smaller_equal(last_invalid_buffer_idx);
                            adjust_gops_for_removed_samples_front(video_data);

                            re_log::trace!(
                                "GC'ed video sample buffer from video streaming cache. Now referencing {:?} video sample chunks with total size of {:?} bytes",
                                video_sample_buffers.num_elements(),
                                video_sample_buffers
                                    .iter()
                                    .map(|b| b.buffer.len())
                                    .sum::<usize>()
                            );

                            if cfg!(debug_assertions)
                                && let Err(err) = video_data.sanity_check()
                            {
                                panic!(
                                    "VideoDataDescription sanity check stream at {:?} failed: {err}",
                                    event.chunk.entity_path()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Adjust GOPs for removed samples at the back of the sample list.
fn adjust_gops_for_removed_samples_back(video_data: &mut re_video::VideoDataDescription) {
    let end_sample_index = video_data.samples.next_index();
    while let Some(gop) = video_data.gops.back_mut() {
        if gop.sample_range.start >= end_sample_index {
            video_data.gops.pop_back();
        } else {
            gop.sample_range.end = end_sample_index;
            break;
        }
    }
}

/// Adjust GOPs for removed samples at the front of the sample list.
fn adjust_gops_for_removed_samples_front(video_data: &mut re_video::VideoDataDescription) {
    let start_sample_index = video_data.samples.min_index();
    while let Some(gop) = video_data.gops.front_mut() {
        if gop.sample_range.end <= start_sample_index {
            video_data.gops.pop_front();
        } else {
            // Do *NOT* forshorten the GOP. The start sample has to be always a keyframe (is_sync==true) sample.
            // So instead, we may have to remove this GOP entirely if it straddles removed samples.
            if gop.sample_range.start < start_sample_index {
                video_data.gops.pop_front();
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::cast_possible_wrap)] // u64 -> i64 is fine

    use re_chunk::{ChunkBuilder, RowId, TimePoint, Timeline};
    use re_chunk_store::ChunkStoreDiff;
    use re_log_types::StoreId;
    use re_sdk_types::archetypes::VideoStream;
    use re_sdk_types::components::VideoCodec;
    use re_video::{VideoDataDescription, VideoEncodingDetails};

    use super::*;

    // Generated using:
    // ffmpeg -i 'rerun-io/internal-test-assets/video/gif_conversion_color_issues_h264.mp4' -c:v libx264 -pix_fmt yuv420p -g 10 -vf scale=iw/2:ih/2 -bf 0 gif_as_h264_nobframes.mp4
    const RAW_H264_DATA: &[u8] =
        include_bytes!("../../../../../tests/assets/video/gif_as_h264_nobframes.h264");

    const NUM_FRAMES: usize = 44;

    /// Iter h264 frames of test data.
    /// Makes a bunch of assumptions on the test data. DO NOT USE THIS IN PRODUCTION.
    fn iter_h264_frames(data: &[u8]) -> impl Iterator<Item = &[u8]> {
        // Don't want to depend on `h264_reader`` here, so quickly whip this up by hand!
        // Assumes Annex-B format with 0x00000001 start codes
        let mut pos = 0;
        std::iter::from_fn(move || {
            if pos >= data.len() {
                return None;
            }
            let start = pos;

            // Skip over current start code.
            pos += 4;

            // Find next start code
            while pos < data.len() {
                if pos + 4 < data.len() && data[pos..pos + 4] == [0, 0, 0, 1] {
                    // Check NAL type (first byte after start code)
                    let nal_type = data[pos + 4] & 0x1F;

                    // Cut a frame if the nal type is 1 (regular frame) or 7 (SPS, expected to be followed by a keyframe).
                    if nal_type == 1 || nal_type == 7 {
                        return Some(&data[start..pos]);
                    }
                }
                pos += 1;
            }

            Some(&data[start..])
        })
    }

    fn validate_stream_from_test_data(
        video_stream: &PlayableVideoStream,
        num_frames_submitted: usize,
    ) {
        let data_descr = video_stream.video_renderer.data_descr();
        data_descr.sanity_check().unwrap();

        let VideoDataDescription {
            codec,
            encoding_details,
            timescale,
            delivery_method,
            gops,
            samples,
            samples_statistics,
            mp4_tracks,
        } = data_descr.clone();

        assert_eq!(codec, re_video::VideoCodec::H264);
        assert_eq!(timescale, None); // Sequence timeline doesn't have a timescale.
        assert!(matches!(
            delivery_method,
            re_video::VideoDeliveryMethod::Stream { .. }
        ));
        assert_eq!(samples_statistics, re_video::SamplesStatistics::NO_BFRAMES);
        assert!(mp4_tracks.is_empty());

        let VideoEncodingDetails {
            codec_string,
            coded_dimensions,
            bit_depth,
            chroma_subsampling,
            stsd,
        } = encoding_details.unwrap();
        assert_eq!(codec_string, "avc1.64000A");
        assert_eq!(coded_dimensions, [110, 82]);
        assert_eq!(bit_depth, Some(8));
        assert_eq!(
            chroma_subsampling,
            Some(re_video::ChromaSubsamplingModes::Yuv420)
        );
        assert_eq!(stsd, None);

        assert_eq!(samples.num_elements(), num_frames_submitted);

        let video_sample_buffers = &video_stream.video_sample_buffers;
        assert!(
            samples
                .iter()
                .all(|s| s.buffer_index < video_sample_buffers.num_elements())
        );
        assert!(samples.iter().all(
            |s| s.byte_span.end() as usize <= video_sample_buffers[s.buffer_index].buffer.len()
        ));

        // The GOPs in the sample data have a fixed size of 10.
        assert_eq!(gops[0].sample_range, 0..10.min(num_frames_submitted));
        if num_frames_submitted > 10 {
            assert_eq!(gops[1].sample_range, 10..20.min(num_frames_submitted));
        }
        if num_frames_submitted > 20 {
            assert_eq!(gops[2].sample_range, 20..30.min(num_frames_submitted));
        }
        if num_frames_submitted > 30 {
            assert_eq!(gops[3].sample_range, 30..40.min(num_frames_submitted));
        }
        if num_frames_submitted > 40 {
            assert_eq!(gops[4].sample_range, 40..44.min(num_frames_submitted));
        }
    }

    fn validate_buffers_fully_compacted(buffers: &StableIndexDeque<SampleBuffer>) {
        assert_eq!(buffers.num_elements(), 1);
        assert_eq!(buffers[0].buffer.len(), RAW_H264_DATA.len());
        assert_eq!(buffers[0].sample_index_range, 0..NUM_FRAMES);
    }

    fn validate_buffers_no_compaction(buffers: &StableIndexDeque<SampleBuffer>) {
        assert_eq!(buffers.num_elements(), NUM_FRAMES);
        assert_eq!(
            buffers.iter().map(|b| b.buffer.len()).sum::<usize>(),
            RAW_H264_DATA.len()
        );
        assert_eq!(buffers[0].sample_index_range.start, 0);
        assert_eq!(buffers.back().unwrap().sample_index_range.end, NUM_FRAMES);
    }

    #[test]
    fn video_stream_cache_from_single_chunk() {
        let mut cache = VideoStreamCache::default();
        let mut store = re_entity_db::EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));
        let timeline = Timeline::new_sequence("frame");

        let mut chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into());
        for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
            chunk_builder = chunk_builder.with_archetype(
                RowId::new(),
                TimePoint::from_iter([(timeline, i as i64)]),
                &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
            );
        }
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();

        let video_stream_lock = cache
            .entry(
                &store,
                &"vid".into(),
                *timeline.name(),
                DecodeSettings::default(),
            )
            .unwrap();
        let video_stream = video_stream_lock.read();

        validate_stream_from_test_data(&video_stream, NUM_FRAMES);

        let video_sample_buffers = &video_stream.video_sample_buffers;
        validate_buffers_fully_compacted(video_sample_buffers);
    }

    #[test]
    fn video_stream_cache_from_chunk_per_frame() {
        let mut cache = VideoStreamCache::default();
        let mut store = re_entity_db::EntityDb::with_store_config(
            StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
        );
        let timeline = Timeline::new_sequence("frame");

        for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
            let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
                RowId::new(),
                TimePoint::from_iter([(timeline, i as i64)]),
                &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
            );
            store
                .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
                .unwrap();
        }

        let video_stream_lock = cache
            .entry(
                &store,
                &"vid".into(),
                *timeline.name(),
                DecodeSettings::default(),
            )
            .unwrap();
        let video_stream = video_stream_lock.read();

        validate_stream_from_test_data(&video_stream, NUM_FRAMES);

        let video_sample_buffers = &video_stream.video_sample_buffers;
        validate_buffers_no_compaction(video_sample_buffers);
    }

    #[test]
    fn video_stream_cache_from_chunk_per_frame_buildup_over_time() {
        let timeline = Timeline::new_sequence("frame");

        // TODO(RR-3212): We disabled compaction on VideoStream for now. Details see https://github.com/rerun-io/rerun/pull/12270
        //for compaction_enabled in [true, false] {
        for compaction_enabled in [false] {
            println!("compaction enabled: {compaction_enabled}");

            let mut cache = VideoStreamCache::default();
            let mut store = re_entity_db::EntityDb::with_store_config(
                StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
                if compaction_enabled {
                    re_chunk_store::ChunkStoreConfig::DEFAULT
                } else {
                    re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED
                },
            );

            let mut frame_iter = iter_h264_frames(RAW_H264_DATA);

            // Add a first frame so we can populate the cache with something
            let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
                RowId::new(),
                TimePoint::from_iter([(timeline, 0)]),
                &VideoStream::new(VideoCodec::H264).with_sample(frame_iter.next().unwrap()),
            );
            store
                .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
                .unwrap();
            let video_stream = cache
                .entry(
                    &store,
                    &"vid".into(),
                    *timeline.name(),
                    DecodeSettings::default(),
                )
                .unwrap();
            validate_stream_from_test_data(&video_stream.read(), 1);

            for (i, frame_bytes) in frame_iter.enumerate() {
                let t = 1 + i as i64;
                let timepoint = TimePoint::from_iter([(timeline, t)]);
                let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
                    RowId::new(),
                    timepoint,
                    &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
                );
                let store_events = store
                    .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
                    .unwrap();
                let store_events_refs = store_events.iter().collect::<Vec<_>>();
                cache.on_store_events(&store_events_refs, &store);

                let video_stream = cache
                    .entry(
                        &store,
                        &"vid".into(),
                        *timeline.name(),
                        DecodeSettings::default(),
                    )
                    .unwrap();
                validate_stream_from_test_data(&video_stream.read(), t as usize + 1);
            }

            let video_sample_buffers = &video_stream.read().video_sample_buffers;
            if compaction_enabled {
                validate_buffers_fully_compacted(video_sample_buffers);
            } else {
                validate_buffers_no_compaction(video_sample_buffers);
            }
        }
    }

    #[test]
    fn video_stream_cache_from_chunk_per_frame_with_gc() {
        let mut cache = VideoStreamCache::default();
        let mut store = re_entity_db::EntityDb::with_store_config(
            StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
        );
        let timeline = Timeline::new_sequence("frame");

        for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
            let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
                RowId::new(),
                TimePoint::from_iter([(timeline, i as i64)]),
                &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
            );
            store
                .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
                .unwrap();
        }

        // Create the cache entry.
        cache
            .entry(
                &store,
                &"vid".into(),
                *timeline.name(),
                DecodeSettings::default(),
            )
            .unwrap();

        // Instead of relying on the "real" GC, we fake it by creating a GC event, pretending the first chunk got removed.
        let storage_engine = store.storage_engine();
        let chunk_store = storage_engine.store();
        cache.on_store_events(
            &[&ChunkStoreEvent {
                store_id: store.store_id().clone(),
                store_generation: store.generation(),
                event_id: 0, // Wrong but don't care.
                diff: ChunkStoreDiff::deletion(chunk_store.iter_chunks().next().unwrap().clone()),
            }],
            &store,
        );

        // Check whether the chunk removal had the expected effect.

        let video_stream_lock = cache
            .entry(
                &store,
                &"vid".into(),
                *timeline.name(),
                DecodeSettings::default(),
            )
            .unwrap();
        let video_stream = video_stream_lock.read();

        // In this setup we have one chunk per sample, this makes it quite easy to reason about it!
        assert_eq!(
            video_stream.video_sample_buffers.num_elements(),
            NUM_FRAMES - 1
        );
        assert_eq!(
            video_stream.video_sample_buffers[1].sample_index_range,
            1..2
        ); // Just to make sure it was the right buffer.

        let data_descr = video_stream.video_renderer.data_descr();
        data_descr.sanity_check().unwrap();

        // Only one frame got removed, BUT the entire first GOP since the first frame was a keyframe!
        assert_eq!(data_descr.samples.num_elements(), NUM_FRAMES - 1);
        assert_eq!(data_descr.gops.get(0), None);
        assert_eq!(
            data_descr.gops.get(1),
            Some(&re_video::GroupOfPictures {
                sample_range: 10..20
            })
        );
    }
}
