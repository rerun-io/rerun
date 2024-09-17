use crate::Cache;
use re_entity_db::VersionedInstancePathHash;
use re_renderer::{
    video::{Video, VideoError},
    RenderContext,
};
use re_types::components::MediaType;

use egui::mutex::Mutex;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct VideoCacheKey {
    pub versioned_instance_path_hash: VersionedInstancePathHash,
    pub media_type: Option<MediaType>,
}

struct Entry {
    used_this_frame: AtomicBool,

    /// Keeps failed loads around, so we can don't try again and again.
    video: Arc<Result<Mutex<Video>, VideoError>>,
}

/// Caches meshes based on their [`VideoCacheKey`].
#[derive(Default)]
pub struct VideoCache(ahash::HashMap<VideoCacheKey, Entry>);

impl VideoCache {
    pub fn entry(
        &mut self,
        name: &str,
        key: VideoCacheKey,
        video_data: &[u8],
        media_type: Option<&str>,
        render_ctx: &RenderContext,
    ) -> Arc<Result<Mutex<Video>, VideoError>> {
        re_tracing::profile_function!();

        let entry = self.0.entry(key).or_insert_with(|| {
            re_log::debug!("Loading video {name:?}…");

            let video = Video::load(render_ctx, video_data, media_type);
            Entry {
                used_this_frame: AtomicBool::new(true),
                video: Arc::new(video.map(Mutex::new)),
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
    fn begin_frame(&mut self) {
        for v in self.0.values() {
            v.used_this_frame.store(false, Ordering::Release);
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
