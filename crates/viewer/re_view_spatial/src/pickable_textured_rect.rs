use re_log_types::EntityPath;
use re_renderer::QueueableDrawData;
use re_renderer::renderer::TexturedRect;
use re_sdk_types::components::DepthMeter;
use re_viewer_context::{ImageInfo, ViewSystemExecutionError};

#[derive(Clone)]
pub enum PickableRectSourceData {
    /// The rectangle is an image with pixel data, potentially some depth meta information.
    Image {
        image: ImageInfo,
        depth_meter: Option<DepthMeter>,
    },

    /// The rectangle is a frame in a video.
    Video,

    /// The rectangle represents a placeholder icon.
    Placeholder,
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
    /// Resolution of the underlying texture.
    pub fn resolution(&self) -> [u32; 2] {
        self.textured_rect.colormapped_texture.width_height()
    }

    pub fn to_draw_data(
        render_ctx: &re_renderer::RenderContext,
        rects: &[Self],
    ) -> Result<QueueableDrawData, ViewSystemExecutionError> {
        // TODO(wumpf): Can we avoid this copy, maybe let DrawData take an iterator?
        let rectangles = rects
            .iter()
            .map(|image| image.textured_rect.clone())
            .collect::<Vec<_>>();
        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => Ok(draw_data.into()),
            Err(err) => Err(ViewSystemExecutionError::DrawDataCreationError(Box::new(
                err,
            ))),
        }
    }
}
