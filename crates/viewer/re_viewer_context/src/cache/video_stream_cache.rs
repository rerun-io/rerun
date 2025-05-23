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
use re_types::archetypes::VideoStream;
use re_video::decode::DecodeSettings;

use crate::Cache;

// ----------------------------------------------------------------------------

#[derive(Clone)]
pub struct StoreVideoStream {
    // TODO: video needs to remain editable.
    pub video_renderer: Arc<re_renderer::video::Video>,
    pub video_sample_data: Buffer,
}

pub struct VideoStreamCacheEntry {
    used_this_frame: AtomicBool,
    video_stream: StoreVideoStream,
}

// TODO: any chance we can unify with `VideoCache`?

#[derive(Hash, Eq, PartialEq)]
struct VideoStreamKey {
    entity_path: EntityPathHash,
    timeline: TimelineName,
}

// TODO: motivate
#[derive(Default)]
pub struct VideoStreamCache(HashMap<VideoStreamKey, VideoStreamCacheEntry>);

impl VideoStreamCache {
    /// TODO: what does this exactly do?
    ///
    /// Returns `None` if there was no video data for this entity on the given timeline.
    /// TODO: Keep track of other errors?
    pub fn entry(
        &mut self,
        store: &re_entity_db::EntityDb,
        entity_path: &EntityPath,
        timeline: TimelineName,
        decode_settings: DecodeSettings,
    ) -> Option<StoreVideoStream> {
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
                    Arc::new(video_data),
                    decode_settings,
                );
                vacant_entry.insert(VideoStreamCacheEntry {
                    used_this_frame: AtomicBool::new(true),
                    video_stream: StoreVideoStream {
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
        Some(entry.video_stream.clone())
    }
}

fn load_video_data_from_chunks(
    store: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
    timeline: TimelineName,
) -> Option<(re_video::VideoDataDescription, Buffer)> {
    re_tracing::profile_function!();

    let component_descr = VideoStream::descriptor_frame();

    // Query for all video chunks on the **entire** timeline.
    // Tempting to bypass the query cache for this, but we don't expect to get new video chunks every frame
    // even for a running stream, so let's stick with the cache!
    //
    // TODO(andreas): Can we be more clever about the chunk range here?
    // Kinda tricky since we need to know how far back (and ahead for b-frames) we have to look.
    let entire_timeline_query =
        re_chunk::RangeQuery::new(timeline, re_log_types::ResolvedTimeRange::EVERYTHING);
    let all_video_chunks = store.storage_engine().cache().range(
        &entire_timeline_query,
        entity_path,
        &[component_descr.clone()],
    );
    let video_chunks = all_video_chunks.get_required(&component_descr).ok()?;

    // Setup video decoder...

    // TODO: multiple chunks?
    let first_chunk = video_chunks.first()?;
    let raw_component_memory = first_chunk.raw_component_memory(&component_descr)?;

    let inner_list_array = raw_component_memory.downcast_array_ref::<arrow::array::ListArray>()?; // TODO: better error handling
    let values = inner_list_array
        .values()
        .downcast_array_ref::<arrow::array::PrimitiveArray<arrow::array::types::UInt8Type>>()?; // TODO: better error handling
    let values = values.values();

    let offsets = inner_list_array.offsets();
    let lengths = re_arrow_util::offsets_lengths(inner_list_array.offsets()).collect::<Vec<_>>();

    // TODO: don't build this up every frame.

    let mut samples = first_chunk
        .iter_component_offsets(&component_descr)
        .zip(first_chunk.iter_component_indices(&timeline, &component_descr))
        .map(|((idx, len), (time, _row_id))| {
            debug_assert_eq!(len, 1, "Expected only a single video sample per timestep");

            let byte_offset = &offsets[idx..idx + len];
            let byte_length = &lengths[idx..idx + len];

            // TODO:
            debug_assert_eq!(byte_offset.len(), 1);
            debug_assert_eq!(byte_length.len(), 1);
            let byte_offset = byte_offset[0];
            let byte_length = byte_length[0];

            re_video::Sample {
                // not the case.
                is_sync: true,

                // TODO(BFRAMETICKET): No b-frames for now. Therefore sample_idx == frame_nr.
                sample_idx: idx,
                frame_nr: idx,

                // TODO(BFRAMETICKET): No b-frames for now. Therefore sample_idx == frame_nr.
                // TODO: what's the actual timestamp...?
                decode_timestamp: re_video::Time(time.as_i64()),
                presentation_timestamp: re_video::Time(time.as_i64()),

                // Filled out later.
                duration: re_video::Time::MAX,

                // We're using offsets directly into the chunk data.
                byte_offset: byte_offset as _,
                byte_length: byte_length as _,
            }
        })
        .collect::<Vec<_>>();

    // Fill out frame durations.
    for sample in 0..samples.len().saturating_sub(1) {
        samples[sample].duration =
            samples[sample + 1].presentation_timestamp - samples[sample].presentation_timestamp;
    }

    // Calculate duration from samples. No bframes means we can just check first and last sample.
    // TODO(BFRAMETICKET): This may be slightly incorrect for b-frames.
    let (decode_start_time, duration) =
        if let (Some(first_sample), Some(last_sample)) = (samples.first(), samples.last()) {
            (
                first_sample.decode_timestamp,
                // TODO: duration of a video is really the range from start to finish.
                // But that messes with our time seeking a little bit because we start at the end. sort of.
                // We'd probably be best of renaming duration to something like "video end timestamp"
                // last_sample.presentation_timestamp - first_sample.presentation_timestamp
                //     + last_sample.duration,
                last_sample.presentation_timestamp + last_sample.duration,
            )
        } else {
            (re_video::Time(0), re_video::Time(0))
        };

    Some((
        re_video::VideoDataDescription {
            codec: re_video::VideoCodec::H264, // TODO, query or guess.
            config: None,
            timescale: re_video::Timescale::NO_SCALE, // TODO: We don't have to work with mp4 scaled time here, so 1 seems alright?
            duration,

            // TODO: how to determine? Player relies on this for seeking.
            gops: vec![re_video::demux::GroupOfPictures {
                decode_start_time,
                sample_range: 0..(samples.len() as _),
            }],

            samples,
            samples_statistics: re_video::SamplesStatistics::NO_BFRAMES, // TODO(BFRAMETICKET): No b-frames for now.
            tracks: std::iter::once((0, Some(re_video::TrackKind::Video))).collect(),
        },
        values.inner().clone(),
    ))
}

impl Cache for VideoStreamCache {
    fn begin_frame(&mut self, renderer_active_frame_idx: u64) {
        // TODO: is this the right purge strategy?

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
