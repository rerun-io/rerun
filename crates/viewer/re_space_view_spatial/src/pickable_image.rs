use re_log_types::EntityPath;
use re_renderer::renderer::TexturedRect;
use re_types::components::DepthMeter;
use re_viewer_context::ImageInfo;

/// Image rectangle that can be picked in the view.
pub struct PickableImageRect {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    pub image: ImageInfo,

    /// Textured rectangle used by the renderer.
    pub textured_rect: TexturedRect,

    pub depth_meter: Option<DepthMeter>,
}

impl PickableImageRect {
    #[inline]
    pub fn width(&self) -> u64 {
        self.image.width() as u64
    }
}
