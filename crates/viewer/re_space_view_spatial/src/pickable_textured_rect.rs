use re_log_types::EntityPath;
use re_renderer::{renderer::TexturedRect, QueueableDrawData};
use re_types::components::DepthMeter;
use re_viewer_context::{ImageInfo, SpaceViewSystemExecutionError};

pub enum PickableRectSourceData {
    /// The rectangle is an image with pixel data, potentially some depth meta information.
    Image {
        image: ImageInfo,
        depth_meter: Option<DepthMeter>,
    },

    /// The rectangle is a frame in a video.
    Video {
        // TODO(andreas): add more information here for picking hover etc.
        resolution: glam::Vec2,
    },

    /// The rectangle represents an error icon.
    ErrorPlaceholder {
        // TODO(andreas): add more information here for picking hover etc.
        resolution: glam::Vec2,
    },
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
    /// How wide the rectangle is in pixels.
    pub fn pixel_width(&self) -> u64 {
        match &self.source_data {
            PickableRectSourceData::Image { image, .. } => image.width() as u64,
            PickableRectSourceData::Video { resolution }
            | PickableRectSourceData::ErrorPlaceholder { resolution } => resolution.x as u64,
        }
    }

    pub fn to_draw_data(
        render_ctx: &re_renderer::RenderContext,
        rects: &[Self],
    ) -> Result<QueueableDrawData, SpaceViewSystemExecutionError> {
        // TODO(wumpf): Can we avoid this copy, maybe let DrawData take an iterator?
        let rectangles = rects
            .iter()
            .map(|image| image.textured_rect.clone())
            .collect::<Vec<_>>();
        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => Ok(draw_data.into()),
            Err(err) => Err(SpaceViewSystemExecutionError::DrawDataCreationError(
                Box::new(err),
            )),
        }
    }
}
