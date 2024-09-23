use crate::Cache;
use re_chunk::RowId;
use re_log_types::hash::Hash64;
use re_renderer::{external::re_video::VideoLoadError, video::Video};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// ----------------------------------------------------------------------------

struct Entry {
    used_this_frame: AtomicBool,

    /// Keeps failed loads around, so we can don't try again and again.
    video: Arc<Result<Video, VideoLoadError>>,
}

/// Caches meshes based on media type & row id.
#[derive(Default)]
pub struct VideoCache(ahash::HashMap<Hash64, Entry>);

impl VideoCache {
    /// Read in some video data and cache the result.
    ///
    /// The `row_id` should be the `RowId` of the blob.
    /// NOTE: videos are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        row_id: RowId,
        video_data: &re_types::datatypes::Blob,
        media_type: Option<&str>,
    ) -> Arc<Result<Video, VideoLoadError>> {
        re_tracing::profile_function!();

        let key = Hash64::hash((row_id, media_type));

        let entry = self.0.entry(key).or_insert_with(|| {
            let video = Video::load(video_data, media_type);
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
        for v in self.0.values() {
            v.used_this_frame.store(false, Ordering::Release);
            if let Ok(video) = v.video.as_ref() {
                video.purge_unused_decoders(renderer_active_frame_idx);
            }
        }
    }

    fn purge_memory(&mut self) {
        self.0
            .retain(|_, v| v.used_this_frame.load(Ordering::Acquire));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
