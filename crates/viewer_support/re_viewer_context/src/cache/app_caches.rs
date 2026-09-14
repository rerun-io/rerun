use re_mutex::RwLock;

use crate::{Cache as _, ImageDecodeCache, ImageStatsCache};

/// App-level caches for data that is not tied to any particular store.
///
/// Unlike per-store caches ([`crate::StoreCache`]), these receive no store events
/// and are not dropped together with a store.
/// Therefore, only caches whose keys are globally unique (e.g. content-addressed by row id)
/// and whose entries expire on their own (see [`crate::Cache::begin_frame`]) belong here.
#[derive(Default)]
pub struct AppCaches {
    pub image_decode: RwLock<ImageDecodeCache>,
    pub image_stats: RwLock<ImageStatsCache>,
}

impl AppCaches {
    /// Call once per frame to potentially flush the caches.
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        let Self {
            image_decode,
            image_stats,
        } = self;
        image_decode.write().begin_frame();
        image_stats.write().begin_frame();
    }

    /// Attempt to free up memory.
    ///
    /// Called BEFORE `begin_frame` (if at all).
    pub fn purge_memory(&mut self) {
        re_tracing::profile_function!();

        let Self {
            image_decode,
            image_stats,
        } = self;
        image_decode.get_mut().purge_memory();
        image_stats.get_mut().purge_memory();
    }
}
