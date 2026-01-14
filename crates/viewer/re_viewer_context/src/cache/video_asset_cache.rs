use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ahash::HashMap;
use re_byte_size::SizeBytes as _;
use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_renderer::external::re_video::VideoLoadError;
use re_renderer::video::Video;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::MediaType;
use re_video::DecodeSettings;

use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{Cache, CacheMemoryReport};

// ----------------------------------------------------------------------------

struct Entry {
    used_this_frame: AtomicBool,

    /// Keeps failed loads around, so we can don't try again and again.
    video: Arc<Result<Video, VideoLoadError>>,
}

impl re_byte_size::SizeBytes for Entry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            used_this_frame: _,
            video,
        } = self;
        match video.as_ref() {
            Ok(video) => video.heap_size_bytes(),
            Err(_) => 100, // close enough
        }
    }
}

/// Caches videos assets and their players based on media type & row id.
#[derive(Default)]
pub struct VideoAssetCache(HashMap<StoredBlobCacheKey, HashMap<Hash64, Entry>>);

impl VideoAssetCache {
    /// Read in some video data and cache the result.
    ///
    /// You may use the `RowId` as cache key if any.
    /// NOTE: videos are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        debug_name: String,
        blob_row_id: RowId,
        blob_component: ComponentIdentifier,
        video_buffer: &re_sdk_types::datatypes::Blob,
        media_type: Option<&MediaType>,
        decode_settings: DecodeSettings,
    ) -> Arc<Result<Video, VideoLoadError>> {
        re_tracing::profile_function!(&debug_name);

        // The component should always be the one in the video asset archetype, but in the future
        // we may allow overrides such that it is sourced from somewhere else.
        let blob_cache_key = StoredBlobCacheKey::new(blob_row_id, blob_component);

        // In order to avoid loading the same video multiple times with
        // known and unknown media type, we have to resolve the media type before
        // loading & building the cache key.
        let Some(media_type) = media_type
            .cloned()
            .or_else(|| MediaType::guess_from_data(video_buffer))
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
                let video = re_video::VideoDataDescription::load_from_bytes(
                    video_buffer,
                    &media_type,
                    &debug_name,
                )
                .map(|data| Video::load(debug_name, data, decode_settings));
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

impl Cache for VideoAssetCache
where
    // NOTE: Explicit bounds help the compiler avoid recursion overflow when checking trait implementations.
    Video: Send + Sync,
    VideoLoadError: Send + Sync,
{
    fn begin_frame(&mut self) {
        re_tracing::profile_function!();

        // Clean up unused video data.
        self.0.retain(|_row_id, per_key| {
            per_key.retain(|_, v| v.used_this_frame.load(Ordering::Acquire));
            !per_key.is_empty()
        });

        // Of the remaining video data, remove all unused decoders.
        #[expect(clippy::iter_over_hash_type)]
        for per_key in self.0.values() {
            for v in per_key.values() {
                v.used_this_frame.store(false, Ordering::Release);
                if let Ok(video) = v.video.as_ref() {
                    video.begin_frame();
                }
            }
        }
    }

    fn memory_report(&self) -> CacheMemoryReport {
        CacheMemoryReport {
            bytes_cpu: self.0.total_size_bytes(),
            bytes_gpu: None,
            per_cache_item_info: Vec::new(),
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

    fn name(&self) -> &'static str {
        "Video Assets"
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_key_removed = filter_blob_removed_events(events);
        self.0
            .retain(|cache_key, _per_key| !cache_key_removed.contains(cache_key));
    }
}
