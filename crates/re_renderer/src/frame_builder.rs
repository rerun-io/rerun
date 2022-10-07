/// Mirrors the GPU contents of a frame-global uniform buffer.
/// Contains information that is constant for a single frame like camera.
/// (does not contain information that is special to a particular renderer or global to the Context)
struct FrameUniformBuffer {
    // TODO(andreas): camera matrix and the like.
    // What does not go here are even more global things like
}

/// FrameBuilder are the highest level rendering block in re_renderer.
///
/// They are used to build up/collect various resources and then send them off for rendering.
/// Collecting objects in this fashion allows for re-use of common resources (e.g. camera)
pub struct FrameBuilder {}

impl FrameBuilder {
    pub fn new() -> Self {
        FrameBuilder {}
    }

    /// Consumes the frame builder and draws it as configured.
    ///
    /// TODO(andreas) dictating a pass is likely not giving us enough flexibility since we may want to have several passes (some of them compute)
    /// Related to
    /// * https://github.com/emilk/egui/issues/2022
    /// * https://github.com/emilk/egui/issues/2084
    pub fn draw<'a>(self, _pass: &mut wgpu::RenderPass<'a>) {}
}
