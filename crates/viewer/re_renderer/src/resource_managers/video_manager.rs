use std::sync::Arc;

use crate::renderer::Video;
use crate::RenderContext;

use super::resource_manager::{
    ResourceHandle, ResourceLifeTime, ResourceManager, ResourceManagerError,
};

slotmap::new_key_type! { pub struct VideoHandleInner; }

pub type VideoHandle = ResourceHandle<VideoHandleInner>;

pub struct VideoManager {
    inner: ResourceManager<VideoHandleInner, Arc<Video>>,
}

impl VideoManager {
    pub(crate) fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub fn create(&mut self, context: &RenderContext, url: String) -> VideoHandle {
        let video = Video::load(context, url);

        self.inner
            .store_resource(Arc::new(video), ResourceLifeTime::LongLived)
    }

    pub fn get(&self, handle: &VideoHandle) -> Result<&Video, ResourceManagerError> {
        self.inner.get(handle).map(|v| v.as_ref())
    }

    pub(crate) fn begin_frame(&mut self, frame_index: u64) {
        self.inner.begin_frame(frame_index);
    }
}
