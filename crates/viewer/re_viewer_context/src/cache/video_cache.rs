use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use ahash::{HashMap, HashSet};
use itertools::Either;

use crate::Cache;
use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_renderer::{
    external::re_video::VideoLoadError,
    video::{DecodeHardwareAcceleration, Video},
};
use re_types::Loggable as _;

// ----------------------------------------------------------------------------

struct Entry {
    used_this_frame: AtomicBool,

    /// Keeps failed loads around, so we can don't try again and again.
    video: Arc<Result<Video, VideoLoadError>>,
}

/// Caches meshes based on media type & row id.
#[derive(Default)]
pub struct VideoCache(HashMap<RowId, HashMap<Hash64, Entry>>);

impl VideoCache {
    /// Read in some video data and cache the result.
    ///
    /// The `row_id` should be the `RowId` of the blob.
    /// NOTE: videos are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        blob_row_id: RowId,
        video_data: &re_types::datatypes::Blob,
        media_type: Option<&str>,
        hw_acceleration: DecodeHardwareAcceleration,
    ) -> Arc<Result<Video, VideoLoadError>> {
        re_tracing::profile_function!();

        let inner_key = Hash64::hash(media_type);

        let entry = self
            .0
            .entry(blob_row_id)
            .or_default()
            .entry(inner_key)
            .or_insert_with(|| {
                let video = Video::load(video_data, media_type, hw_acceleration);
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
        self.0.retain(|_row_id, per_key| {
            per_key.retain(|_, v| v.used_this_frame.load(Ordering::Acquire));
            !per_key.is_empty()
        });
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let row_ids_removed: HashSet<RowId> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_video_blob = || {
                    event
                        .chunk
                        .components()
                        .contains_key(&re_types::components::Blob::name())
                };

                if is_deletion() && contains_video_blob() {
                    Either::Left(event.chunk.row_ids())
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|row_id, _per_key| !row_ids_removed.contains(row_id));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
