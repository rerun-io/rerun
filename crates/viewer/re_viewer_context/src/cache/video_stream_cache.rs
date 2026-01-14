use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ahash::HashMap;
use egui::NumExt as _;
use itertools::Itertools as _;
use parking_lot::RwLock;
use re_byte_size::SizeBytes as _;
use re_chunk::{ChunkId, EntityPath, Span, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::{EntityPathHash, TimeType};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components;
use re_video::{DecodeSettings, StableIndexDeque};

use crate::{Cache, CacheMemoryReport};

/// Video stream from the store, ready for playback.
///
/// This is compromised of:
/// * raw video stream data (pointers into all live rerun-chunks holding video frame data)
/// * metadata with that we know about the stream (where are I-frames etc.)
/// * active players for this stream and their state
pub struct PlayableVideoStream {
    pub video_renderer: re_renderer::video::Video,
}

impl re_byte_size::SizeBytes for PlayableVideoStream {
    fn heap_size_bytes(&self) -> u64 {
        let Self { video_renderer } = self;
        video_renderer.heap_size_bytes()
    }
}

impl PlayableVideoStream {
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
    known_chunk_ranges: BTreeMap<ChunkId, ChunkSampleRange>,
}

impl re_byte_size::SizeBytes for VideoStreamCacheEntry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            used_this_frame: _,
            video_stream,
            known_chunk_ranges,
        } = self;

        video_stream.read().heap_size_bytes() + known_chunk_ranges.heap_size_bytes()
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
                let (video_descr, known_chunk_ranges) =
                    load_video_data_from_chunks(store, entity_path, timeline)?;

                let video = re_renderer::video::Video::load(
                    entity_path.to_string(),
                    video_descr,
                    decode_settings,
                );
                vacant_entry.insert(VideoStreamCacheEntry {
                    used_this_frame: AtomicBool::new(true),
                    video_stream: Arc::new(RwLock::new(PlayableVideoStream {
                        video_renderer: video,
                    })),
                    known_chunk_ranges,
                })
            }
        };

        // Using acquire/release here to be on the safe side and for semantical soundness:
        // Whatever thread is acquiring the fact that this was used, should also see/acquire
        // the side effect of having the entry contained in the cache.
        entry.used_this_frame.store(true, Ordering::Release);
        Ok(entry.video_stream.clone())
    }

    fn handle_store_event(
        &mut self,
        event: &&ChunkStoreEvent,
        timeline: &TimelineName,
        key: &VideoStreamKey,
    ) {
        let Some(entry) = self.0.get_mut(key) else {
            // If we don't have a cache entry yet, we can skip over this event as there's nothing to update.
            return;
        };

        let mut video_stream = entry.video_stream.write();
        let PlayableVideoStream { video_renderer } = &mut *video_stream;
        let video_data = video_renderer.data_descr_mut();
        video_data.delivery_method = re_video::VideoDeliveryMethod::new_stream();

        match event.kind {
            re_chunk_store::ChunkStoreDiffKind::Addition => {
                // If this came with a compaction, throw out all samples and tracked keyframes that were compacted
                // away and restart there. This is a bit slower than re-using all the data, but much simpler and
                // more robust.
                //
                // Compactions events are documented to happen on **addition** only.
                // Therefore, it should be save to remove only the newest data.
                let chunk = if let Some(compaction) = &event.compacted {
                    if compaction
                        .srcs
                        .keys()
                        .any(|c| entry.known_chunk_ranges.contains_key(c))
                    {
                        drop(video_stream);
                        self.0.remove(key);
                        return;
                    }

                    if let Some(first_sample_with_chunk) = video_data.samples.position(|sample| {
                        sample.sample().is_some_and(|s| {
                            compaction
                                .srcs
                                .contains_key(&ChunkId::from_tuid(s.source_id))
                        })
                    }) {
                        // Remove all samples that are in this and future buffers.
                        video_data
                            .samples
                            .remove_all_with_index_larger_equal(first_sample_with_chunk);
                    }

                    adjust_keyframes_for_removed_samples(video_data);

                    // `event.chunk` is added data PRIOR to compaction.
                    &compaction.new_chunk
                } else {
                    &event.chunk
                };

                let encoding_details_before = video_data.encoding_details.clone();

                if let Err(err) = read_samples_from_chunk(
                    *timeline,
                    &chunk.sorted_by_timeline_if_unsorted(timeline),
                    &entry.known_chunk_ranges,
                    video_data,
                ) {
                    match err {
                        VideoStreamProcessingError::OutOfOrderSamples => {
                            re_log::debug!("Found out of order samples");
                            drop(video_stream);
                            // We found out of order samples, discard this video stream cache entry
                            // to reconstruct it with all data in mind.
                            self.0.remove(key);
                            return;
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
                if let Some(known_offset) = entry.known_chunk_ranges.get(&event.chunk.id()) {
                    for (_idx, sample) in video_data.samples.iter_index_range_clamped_mut(
                        &(known_offset.first_sample..video_data.samples.next_index()),
                    ) {
                        if sample.source_id() == event.chunk.id().as_tuid() {
                            sample.unload();
                        }
                    }
                    adjust_keyframes_for_removed_samples(video_data);

                    if cfg!(debug_assertions)
                        && let Err(err) = video_data.sanity_check()
                    {
                        panic!(
                            "VideoDataDescription sanity check stream at {:?} failed: {err}",
                            event.chunk.entity_path()
                        );
                    }
                }
                // For unknown chunks, chunk deletion typically happens at the start of the recording
                // due to garbage collection. Even if it were to happen somewhere in the middle of
                // the stream, we'd still want to delete all prior samples & buffers since we can't
                // handle gaps in the video stream in this case.
                else if let Some(last_invalid_sample_idx) = {
                    let iter = video_data.samples.iter_indexed();
                    iter.rev()
                        .find(|(_, s)| s.source_id() == event.chunk.id().as_tuid())
                        .map(|(idx, _)| idx)
                } {
                    video_data
                        .samples
                        .remove_all_with_index_smaller_equal(last_invalid_sample_idx);
                    adjust_keyframes_for_removed_samples(video_data);

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

fn load_video_data_from_chunks(
    store: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
    timeline: TimelineName,
) -> Result<
    (
        re_video::VideoDataDescription,
        BTreeMap<ChunkId, ChunkSampleRange>,
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
    let mut video_descr = re_video::VideoDataDescription {
        codec,
        encoding_details: None, // Unknown so far, we'll find out later.
        timescale: timescale_for_timeline(store, timeline),
        delivery_method: re_video::VideoDeliveryMethod::new_stream(),
        keyframe_indices: Vec::new(),
        samples: StableIndexDeque::with_capacity(sample_chunks.len()), // Number of video chunks is minimum number of samples.
        samples_statistics: re_video::SamplesStatistics::NO_BFRAMES, // TODO(#10090): No b-frames for now.
        mp4_tracks: Default::default(),
    };

    let mut known_chunk_ranges = BTreeMap::new();

    let known_chunks = if let Some(entity_timelines) = store
        .rrd_manifest_index()
        .native_temporal_map()
        .get(entity_path)
        && let Some((_, components)) = entity_timelines.iter().find(|(t, _)| *t.name() == timeline)
        && let Some(chunks) = components.get(&sample_component)
    {
        chunks
    } else {
        &BTreeMap::new()
    };

    let sorted_samples = sample_chunks
        .iter()
        .map(|c| c.sorted_by_timeline_if_unsorted(&timeline))
        .collect::<Vec<_>>();

    load_known_chunk_ranges(
        &mut video_descr,
        &mut known_chunk_ranges,
        known_chunks,
        &sorted_samples,
        &timeline,
    );

    for chunk in &sorted_samples {
        let Some(known_range) = known_chunk_ranges.get(&chunk.id()) else {
            assert!(
                !cfg!(debug_assertions),
                "[DEBUG] We just made sure this chunk's range was registered"
            );
            continue;
        };

        if let Err(err) =
            read_samples_from_known_chunk(timeline, chunk, known_range, &mut video_descr)
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

    Ok((video_descr, known_chunk_ranges))
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

/// This is the range all samples of the chunk is in. But there
/// may also be samples from other chunks in this range.
#[derive(Debug)]
struct ChunkSampleRange {
    first_sample: re_video::SampleIndex,

    /// Last sample (inclusive).
    last_sample: re_video::SampleIndex,
}

impl re_byte_size::SizeBytes for ChunkSampleRange {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

fn read_samples_from_chunk(
    timeline: TimelineName,
    chunk: &re_chunk::Chunk,
    known_offsets: &BTreeMap<ChunkId, ChunkSampleRange>,
    video_descr: &mut re_video::VideoDataDescription,
) -> Result<(), VideoStreamProcessingError> {
    if let Some(known_offset) = known_offsets.get(&chunk.id()) {
        read_samples_from_known_chunk(timeline, chunk, known_offset, video_descr)
    } else {
        read_samples_from_new_chunk(timeline, chunk, video_descr)
    }
}

/// Reads all video samples from a chunk that previously wasn't mention into an existing video
/// description.
///
/// Encoding details are automatically updated whenever detected.
/// Changes of encoding details over time will trigger a warning.
fn read_samples_from_known_chunk(
    timeline: TimelineName,
    chunk: &re_chunk::Chunk,
    known_range: &ChunkSampleRange,
    video_descr: &mut re_video::VideoDataDescription,
) -> Result<(), VideoStreamProcessingError> {
    let re_video::VideoDataDescription {
        codec,
        samples,
        keyframe_indices,
        encoding_details,
        ..
    } = video_descr;

    let sample_component = VideoStream::descriptor_sample().component;
    let Some(raw_array) = chunk.raw_component_array(sample_component) else {
        // This chunk doesn't have any video chunks.
        return Ok(());
    };

    let (offsets, values) = re_arrow_util::blob_arrays_offsets_and_buffer(&raw_array).ok_or(
        VideoStreamProcessingError::InvalidVideoSampleType(raw_array.data_type().clone()),
    )?;

    let lengths = offsets.lengths().collect::<Vec<_>>();

    let split_idx = keyframe_indices
        .binary_search(&known_range.first_sample)
        .unwrap_or_else(|e| e);

    let end_keyframes = keyframe_indices
        .drain(split_idx..)
        .filter(|idx| *idx >= known_range.first_sample + chunk.num_rows())
        .collect::<Vec<_>>();

    let range = known_range.first_sample..known_range.last_sample + 1;
    let mut samples_iter = samples
        .iter_index_range_clamped_mut(&range)
        .filter(|(_, c)| c.source_id() == chunk.id().as_tuid())
        .peekable();

    for (component_offset, (time, _row_id)) in chunk
        .iter_component_offsets(sample_component)
        .zip(chunk.iter_component_indices(timeline, sample_component))
    {
        if component_offset.len == 0 {
            // Ignore empty samples.
            continue;
        }
        if component_offset.len != 1 {
            re_log::warn_once!(
                "Expected only a single VideoSample per row (it is a mono-component)"
            );
            continue;
        }

        // Do **not** use the `component_offset.start` for determining the sample index
        // as it is only for the offset in the underlying arrow arrays which means that
        // it may in theory step arbitrarily through the data.
        let Some((sample_idx, _sample)) = samples_iter.peek() else {
            re_log::error!("Failed to add all video stream samples from chunk");
            break;
        };

        let byte_span = Span {
            start: offsets[component_offset.start] as usize,
            len: lengths[component_offset.start],
        };
        let sample_bytes = &values[byte_span.range()];

        // Note that the conversion of this time value is already handled by `VideoDataDescription::timescale`:
        // For sequence time we use a scale of 1, for nanoseconds time we use a scale of 1_000_000_000.
        let decode_timestamp = re_video::Time(time.as_i64());

        let is_sync = is_sample_sync(codec, encoding_details, sample_bytes);

        if is_sync {
            keyframe_indices.push(*sample_idx);
        }

        let Some(byte_span) = byte_span.try_cast::<u32>() else {
            re_log::warn_once!("Video byte range does not fit in u32: {byte_span:?}");
            continue;
        };

        let Some((sample_idx, sample)) = samples_iter.next() else {
            re_log::error_once!("Failed to add all video stream samples from chunk");
            break;
        };

        *sample = re_video::SampleMetadataState::Present(re_video::SampleMetadata {
            is_sync,

            // TODO(#10090): No b-frames for now. Therefore sample_idx == frame_nr.
            frame_nr: sample_idx as u32,
            decode_timestamp,
            presentation_timestamp: decode_timestamp,

            // Filled out later for everything but the last frame.
            duration: None,

            source_id: chunk.id().as_tuid(),
            byte_span,
        });
    }

    // Mark the remaining ones as skipped.
    for (_idx, sample) in samples_iter {
        *sample = re_video::SampleMetadataState::Skip(chunk.id().as_tuid());
    }

    let n = end_keyframes.partition_point(|sample_idx| *sample_idx <= known_range.last_sample);
    let sort_to = keyframe_indices.len() + n;
    keyframe_indices.extend(end_keyframes);

    if n > 0 {
        keyframe_indices[split_idx..sort_to].sort_unstable();
    }

    update_sample_durations(known_range, samples)?;

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

/// Checks if the sample is a sync frame, and updates encoding details if necessary.
fn is_sample_sync(
    codec: &re_video::VideoCodec,
    encoding_details: &mut Option<re_video::VideoEncodingDetails>,
    sample_bytes: &[u8],
) -> bool {
    match re_video::detect_gop_start(sample_bytes, *codec) {
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
        Ok(re_video::GopStartDetection::NotStartOfGop) => false,

        Err(err) => {
            re_log::error_once!("Failed to detect GOP for video sample: {err}");
            false
        }
    }
}

/// Fill out durations for all new samples plus the first existing sample for which we didn't know the duration yet.
/// (We set the duration for the last sample to `None` since we don't know how long it will last.)
fn update_sample_durations(
    known_offset: &ChunkSampleRange,
    samples: &mut StableIndexDeque<re_video::SampleMetadataState>,
) -> Result<(), VideoStreamProcessingError> {
    let mut start = known_offset.first_sample.at_least(samples.min_index());
    while let Some(new_start) = start.checked_sub(1)
        && let Some(sample) = samples.get(new_start)
        && !matches!(sample, re_video::SampleMetadataState::Unloaded(_))
    {
        start = new_start;

        if matches!(sample, re_video::SampleMetadataState::Present(_)) {
            break;
        }
    }
    let mut end = known_offset
        .last_sample
        .at_most(samples.next_index().saturating_sub(1));
    while let Some(new_end) = end.checked_add(1)
        && let Some(sample) = samples.get(new_end)
        && !matches!(sample, re_video::SampleMetadataState::Unloaded(_))
    {
        end = new_end;

        if matches!(sample, re_video::SampleMetadataState::Present(_)) {
            break;
        }
    }
    let mut last_present_sample = None;

    // (Note that we can't use tuple_windows here because it can't handle mutable references)
    for sample_idx in start..=end {
        let sample = match &samples[sample_idx] {
            re_video::SampleMetadataState::Present(sample) => sample,
            re_video::SampleMetadataState::Skip(_) => {
                continue;
            }
            re_video::SampleMetadataState::Unloaded(_) => {
                last_present_sample = None;
                continue;
            }
        };

        let current = sample.presentation_timestamp;

        if let Some((last_sample_idx, timestamp)) = last_present_sample
            && let Some(last_sample) = samples[last_sample_idx].sample_mut()
        {
            let duration = current - timestamp;
            if duration.0 < 0 {
                return Err(VideoStreamProcessingError::OutOfOrderSamples);
            }
            last_sample.duration = Some(duration);
        }

        last_present_sample = Some((sample_idx, current));
    }

    Ok(())
}

/// Reads all video samples from a chunk that previously wasn't mention into an existing video
/// description.
///
/// Rejects out of order samples - new samples must have a higher timestamp than the previous ones.
/// Since samples within a chunk are guaranteed to be ordered, this can only happen if a new chunk
/// is inserted that has is timestamped to be older than the data in the last added chunk.
///
/// Encoding details are automatically updated whenever detected.
/// Changes of encoding details over time will trigger a warning.
fn read_samples_from_new_chunk(
    timeline: TimelineName,
    chunk: &re_chunk::Chunk,
    video_descr: &mut re_video::VideoDataDescription,
) -> Result<(), VideoStreamProcessingError> {
    re_tracing::profile_function!();

    let re_video::VideoDataDescription {
        codec,
        samples,
        keyframe_indices,
        encoding_details,
        ..
    } = video_descr;

    let sample_component = VideoStream::descriptor_sample().component;
    let Some(raw_array) = chunk.raw_component_array(sample_component) else {
        // This chunk doesn't have any video chunks.
        return Ok(());
    };

    let mut previous_max_presentation_timestamp = samples
        .iter()
        .rev()
        .find_map(|s| match s {
            re_video::SampleMetadataState::Present(s) => Some(Some(s)),
            re_video::SampleMetadataState::Skip(_) => None,
            re_video::SampleMetadataState::Unloaded(_) => Some(None),
        })
        .flatten()
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

    let (offsets, values) = re_arrow_util::blob_arrays_offsets_and_buffer(&raw_array).ok_or(
        VideoStreamProcessingError::InvalidVideoSampleType(raw_array.data_type().clone()),
    )?;

    let lengths = offsets.lengths().collect::<Vec<_>>();

    let sample_base_idx = samples.next_index();

    let chunk_id = chunk.id();
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

                let byte_span = Span {
                    start: offsets[component_offset.start] as usize,
                    len: lengths[component_offset.start],
                };
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

                let is_sync = is_sample_sync(codec, encoding_details, sample_bytes);

                if is_sync {
                    keyframe_indices.push(sample_idx);
                }

                let Some(byte_span) = byte_span.try_cast::<u32>() else {
                    re_log::warn_once!("Video byte range does not fit in u32: {byte_span:?}");
                    return None;
                };

                Some(re_video::SampleMetadataState::Present(
                    re_video::SampleMetadata {
                        is_sync,

                        // TODO(#10090): No b-frames for now. Therefore sample_idx == frame_nr.
                        frame_nr: sample_idx as u32,
                        decode_timestamp,
                        presentation_timestamp: decode_timestamp,

                        // Filled out later for everything but the last frame.
                        duration: None,

                        source_id: chunk_id.as_tuid(),
                        byte_span,
                    },
                ))
            }),
    );

    // Any new samples actually added? Early out if not.
    if sample_base_idx == samples.next_index() {
        return Ok(());
    }

    update_sample_durations(
        &ChunkSampleRange {
            first_sample: sample_base_idx,
            last_sample: samples.next_index().saturating_sub(1),
        },
        samples,
    )?;

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

    fn on_rrd_manifest(&mut self, _entity_db: &EntityDb) {
        // Reset everything when we receive an rrd manifest.
        self.0.clear();
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
                self.handle_store_event(
                    event,
                    timeline,
                    &VideoStreamKey {
                        entity_path: event.chunk.entity_path().hash(),
                        timeline: *timeline,
                    },
                );
            }
        }
    }
}

/// `loaded_chunks` should be sorted by start time, and internally sorted on `timeline`.
fn load_known_chunk_ranges(
    data_descr: &mut re_video::VideoDataDescription,
    known_chunk_ranges: &mut BTreeMap<ChunkId, ChunkSampleRange>,
    chunks_from_manifest: &BTreeMap<ChunkId, re_log_encoding::RrdManifestTemporalMapEntry>,
    mut loaded_chunks: &[re_chunk::Chunk],
    timeline: &TimelineName,
) {
    re_tracing::profile_function!();

    // If we also added loaded chunks into this we could optimize the case where
    // we have a lot of chunks that the rrd manifest doesn't track.
    let chunk_timepoints = chunks_from_manifest
        .iter()
        .sorted_by_key(|(_, e)| e.time_range.min);
    let mut loaded_samples_timepoint_iterators = Vec::new();

    let sample_component = VideoStream::descriptor_sample().component;
    let all_loaded_chunks: ahash::HashSet<re_chunk::ChunkId> =
        loaded_chunks.iter().map(|c| c.id()).collect();
    for next_chunk in chunk_timepoints.map(Some).chain(std::iter::once(None)) {
        let next_timepoint = next_chunk
            .map(|(_, e)| e.time_range.min)
            .unwrap_or(re_chunk::TimeInt::MAX);

        let partition_point = loaded_chunks.partition_point(|c| {
            c.iter_component_timepoints(sample_component)
                .find_map(|t| {
                    t.get(timeline)
                        .map(|t| re_chunk::TimeInt::new_temporal(t.get()) <= next_timepoint)
                })
                // This shouldn't happen.
                .unwrap_or_else(|| {
                    debug_assert!(
                        false,
                        "Chunk with no samples on '{timeline}' should've already been filtered out"
                    );
                    false
                })
        });

        let (included, rest) = loaded_chunks.split_at(partition_point);

        loaded_chunks = rest;

        loaded_samples_timepoint_iterators.extend(included.iter().map(|chunk| {
            (
                chunk.id(),
                chunk
                    .iter_component_timepoints(sample_component)
                    .filter_map(|t| t.get(timeline))
                    .peekable(),
            )
        }));

        let mut chunk_id_for_for_next_sample = || {
            loaded_samples_timepoint_iterators
                .retain_mut(|(_, timepoints)| timepoints.peek().is_some());

            let (chunk_id, timepoints) = loaded_samples_timepoint_iterators
                .iter_mut()
                // There won't be any nones here since we retained above.
                .reduce(|a, b| if a.1.peek() < b.1.peek() { a } else { b })?;

            let timepoint = timepoints.peek()?;

            if re_chunk::TimeInt::from(*timepoint) <= next_timepoint {
                timepoints.next();

                Some(*chunk_id)
            } else {
                None
            }
        };

        while let Some(chunk_id) = chunk_id_for_for_next_sample() {
            let idx = data_descr.samples.next_index();

            known_chunk_ranges
                .entry(chunk_id)
                .or_insert(ChunkSampleRange {
                    first_sample: idx,
                    last_sample: idx,
                })
                .last_sample = idx;

            // This is loaded, but will be populated later.
            data_descr
                .samples
                .push_back(re_video::SampleMetadataState::Unloaded(chunk_id.as_tuid()));
        }

        let idx = data_descr.samples.next_index();
        if let Some((chunk, rrd_entry)) = next_chunk
            // `num_rows` gives us the maximal amount of frames in this chunk.
            && let Some(end_idx) = rrd_entry
                .num_rows
                .checked_sub(1)
                .map(|offset| idx.saturating_add(offset as usize))
            && !all_loaded_chunks.contains(chunk)
        {
            known_chunk_ranges.insert(
                *chunk,
                ChunkSampleRange {
                    first_sample: idx,
                    last_sample: end_idx,
                },
            );
            data_descr.samples.extend(std::iter::repeat_n(
                re_video::SampleMetadataState::Unloaded(chunk.as_tuid()),
                rrd_entry.num_rows as usize,
            ));
        }
    }

    debug_assert!(loaded_samples_timepoint_iterators.is_empty());
}

/// Adjust keyframes for removed samples.
fn adjust_keyframes_for_removed_samples(video_data: &mut re_video::VideoDataDescription) {
    // Keyframe indices don't have to be stable, so can retain here.
    video_data.keyframe_indices.retain(|idx| {
        video_data
            .samples
            .get(*idx)
            .and_then(|s| s.sample())
            .is_some_and(|sample| sample.is_sync)
    });
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
            keyframe_indices,
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

        // The GOPs in the sample data have a fixed size of 10.
        assert_eq!(keyframe_indices[0], 0);
        if num_frames_submitted > 10 {
            assert_eq!(keyframe_indices[1], 10);
        }
        if num_frames_submitted > 20 {
            assert_eq!(keyframe_indices[2], 20);
        }
        if num_frames_submitted > 30 {
            assert_eq!(keyframe_indices[3], 30);
        }
        if num_frames_submitted > 40 {
            assert_eq!(keyframe_indices[4], 40);
        }
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

        let data_descr = video_stream.video_renderer.data_descr();
        data_descr.sanity_check().unwrap();

        // Only one frame got removed, BUT the entire first GOP since the first frame was a keyframe!
        assert_eq!(
            data_descr
                .samples
                .iter()
                .filter(|s| !matches!(s, re_video::SampleMetadataState::Unloaded(_)))
                .count(),
            NUM_FRAMES - 1
        );
        assert_eq!(data_descr.keyframe_indices.first(), Some(&10));
    }
}
