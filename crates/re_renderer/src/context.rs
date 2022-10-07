/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {}

impl RenderContext {
    pub fn new(/*device: &wgpu::Device, queue: &wgpu::Queue*/) -> Self {
        RenderContext {}
    }
}
