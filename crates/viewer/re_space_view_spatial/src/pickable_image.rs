use re_chunk_store::RowId;
use re_log_types::EntityPath;
use re_renderer::renderer::TexturedRect;
use re_types::{components::DepthMeter, datatypes::TensorData, tensor_data::TensorDataMeaning};
use re_viewer_context::ImageComponents;

/// Image rectangle that can be picked in the view.
pub struct PickableImageRect {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    /// The row id of the point-of-view component.
    pub row_id: RowId,

    /// Textured rectangle used by the renderer.
    pub textured_rect: TexturedRect,

    pub meaning: TensorDataMeaning,

    pub depth_meter: Option<DepthMeter>,

    pub tensor: Option<TensorData>,

    pub image: Option<ImageComponents>,
}

impl PickableImageRect {
    pub fn width(&self) -> Option<u64> {
        #![allow(clippy::manual_map)] // Annoying

        if let Some(tensor) = &self.tensor {
            tensor.image_height_width_channels().map(|[_, w, _]| w)
        } else if let Some(image) = &self.image {
            Some(image.width() as u64)
        } else {
            None
        }
    }
}
