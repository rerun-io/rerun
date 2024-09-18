use std::sync::Arc;

use re_log_types::EntityPath;
use re_renderer::renderer::TexturedRect;
use re_types::components::DepthMeter;
use re_viewer_context::ImageInfo;

pub enum PickableRectSourceData {
    Image {
        image: ImageInfo,
        depth_meter: Option<DepthMeter>,
    },
    #[allow(unused)] // TODO(#7353): wip
    Video(Arc<re_renderer::video::Video>),
}

/// Image rectangle that can be picked in the view.
pub struct PickableTexturedRect {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    /// Textured rectangle used by the renderer.
    pub textured_rect: TexturedRect,

    /// Associated data.
    pub source_data: PickableRectSourceData,
}

impl PickableTexturedRect {
    pub fn pixel_width(&self) -> u64 {
        match &self.source_data {
            PickableRectSourceData::Image { image, .. } => image.width() as u64,
            PickableRectSourceData::Video(video) => video.width() as u64,
        }
    }
}
