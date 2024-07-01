use re_log_types::EntityPath;
use re_renderer::renderer::TexturedRect;

/// Image rectangle that can be picked in the view.
pub struct PickableImageRect {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    /// Textured rectangle used by the renderer.
    pub textured_rect: TexturedRect,
}
