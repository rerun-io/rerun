use crate::{render_pipeline_pool::RenderPipelinePool, texture_pool::TexturePool};

/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    /// The color format used by the eframe output buffer.
    output_format_color: wgpu::TextureFormat,

    pub(crate) texture_pool: TexturePool,
    pub(crate) renderpipeline_pool: RenderPipelinePool,

    frame_index: u64,
}

impl RenderContext {
    pub fn new(
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        output_format_color: wgpu::TextureFormat,
    ) -> Self {
        RenderContext {
            output_format_color,

            texture_pool: TexturePool::new(),
            renderpipeline_pool: RenderPipelinePool::new(),

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self) {
        self.frame_index += 1;
        self.texture_pool.frame_maintenance(self.frame_index);
        self.renderpipeline_pool.frame_maintenance(self.frame_index);
    }

    pub(crate) fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format_color
    }
}
