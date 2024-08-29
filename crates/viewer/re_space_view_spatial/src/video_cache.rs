use egui::mutex::Mutex;
use re_entity_db::VersionedInstancePathHash;
use re_log_types::hash::Hash64;
use re_renderer::renderer::Video;
use re_renderer::RenderContext;
use re_types::components::MediaType;
use re_viewer_context::Cache;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct VideoCacheKey {
    pub versioned_instance_path_hash: VersionedInstancePathHash,
    pub query_result_hash: Hash64,
    pub media_type: Option<MediaType>,
}

struct Entry {
    used_this_frame: AtomicBool,
    video: Option<Arc<Mutex<Video>>>,
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
    ) -> Option<Arc<Mutex<Video>>> {
        re_tracing::profile_function!();

        let entry = self.0.entry(key).or_insert_with(|| {
            re_log::debug!("Loading video {name:?}…");

            let result = Video::load(render_ctx, media_type, video_data);
            let video = match result {
                Ok(video) => Some(Arc::new(Mutex::new(video))),
                Err(err) => {
                    re_log::warn_once!("Failed to load video {name:?}: {err}");
                    None
                }
            };
            Entry {
                used_this_frame: AtomicBool::new(false),
                video,
            }
        });
        entry.used_this_frame.store(true, Ordering::Relaxed);
        entry.video.clone()
    }
}

impl Cache for VideoCache {
    fn begin_frame(&mut self) {
        for v in self.0.values() {
            v.used_this_frame.store(false, Ordering::Relaxed);
        }
    }

    fn purge_memory(&mut self) {
        self.0
            .retain(|_, v| v.used_this_frame.load(Ordering::Relaxed));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
