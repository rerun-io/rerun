use re_log_types::EntityPath;
use re_renderer::renderer::TexturedRect;
use re_types::tensor_data::TensorDataMeaning;

/// Image rectangle that can be picked in the view.
pub struct PickableImageRect {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    /// The meaning of the tensor stored in the image
    pub meaning: TensorDataMeaning,

    /// Textured rectangle used by the renderer.
    pub textured_rect: TexturedRect,
}
