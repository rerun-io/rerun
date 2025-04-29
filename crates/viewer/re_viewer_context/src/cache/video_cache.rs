use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_renderer::{external::re_video::VideoLoadError, video::Video};
use re_types::{components::MediaType, Component as _};
use re_video::decode::DecodeSettings;

use crate::Cache;

// ----------------------------------------------------------------------------

struct Entry {
    used_this_frame: AtomicBool,

    /// Keeps failed loads around, so we can don't try again and again.
    video: Arc<Result<Video, VideoLoadError>>,
}

/// Caches meshes based on media type & row id.
#[derive(Default)]
pub struct VideoCache(HashMap<Hash64, HashMap<Hash64, Entry>>);

impl VideoCache {
    /// Read in some video data and cache the result.
    ///
    /// You may use the `RowId` as cache key if any.
    /// NOTE: videos are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        debug_name: String,
        blob_cache_key: Hash64,
        video_data: &re_types::datatypes::Blob,
        media_type: Option<&MediaType>,
        decode_settings: DecodeSettings,
    ) -> Arc<Result<Video, VideoLoadError>> {
        re_tracing::profile_function!(&debug_name);

        // In order to avoid loading the same video multiple times with
        // known and unknown media type, we have to resolve the media type before
        // loading & building the cache key.
        let Some(media_type) = media_type
            .cloned()
            .or_else(|| MediaType::guess_from_data(video_data))
        else {
            return Arc::new(Err(VideoLoadError::UnrecognizedMimeType));
        };

        let inner_key = Hash64::hash((media_type.as_str(), decode_settings.hw_acceleration));

        let entry = self
            .0
            .entry(blob_cache_key)
            .or_default()
            .entry(inner_key)
            .or_insert_with(|| {
                let video = re_video::VideoData::load_from_bytes(video_data, &media_type)
                    .map(|data| Video::load(debug_name, Arc::new(data), decode_settings));
                Entry {
                    used_this_frame: AtomicBool::new(true),
                    video: Arc::new(video),
                }
            });

        // Using acquire/release here to be on the safe side and for semantical soundness:
        // Whatever thread is acquiring the fact that this was used, should also see/acquire
        // the side effect of having the entry contained in the cache.
        entry.used_this_frame.store(true, Ordering::Release);
        entry.video.clone()
    }
}

impl Cache for VideoCache {
    fn begin_frame(&mut self, renderer_active_frame_idx: u64) {
        // Clean up unused video data.
        self.0.retain(|_row_id, per_key| {
            per_key.retain(|_, v| v.used_this_frame.load(Ordering::Acquire));
            !per_key.is_empty()
        });

        // Of the remaining video data, remove all unused decoders.
        for per_key in self.0.values() {
            for v in per_key.values() {
                v.used_this_frame.store(false, Ordering::Release);
                if let Ok(video) = v.video.as_ref() {
                    video.purge_unused_decoders(renderer_active_frame_idx);
                }
            }
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

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let cache_key_removed: HashSet<Hash64> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_video_blob = || {
                    event
                        .chunk
                        .components()
                        .contains_component_name(re_types::components::Blob::name())
                };

                if is_deletion() && contains_video_blob() {
                    Either::Left(event.chunk.row_ids().map(Hash64::hash))
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|cache_key, _per_key| !cache_key_removed.contains(cache_key));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
