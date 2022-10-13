use crate::resource_pools::{
    bind_group_layout_pool::BindGroupLayoutPool, bind_group_pool::BindGroupPool,
    pipeline_layout_pool::PipelineLayoutPool, render_pipeline_pool::RenderPipelinePool,
    sampler_pool::SamplerPool, texture_pool::TexturePool,
};

/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    /// The color format used by the eframe output buffer.
    output_format_color: wgpu::TextureFormat,

    pub(crate) textures: TexturePool,
    pub(crate) renderpipelines: RenderPipelinePool,
    pub(crate) pipeline_layouts: PipelineLayoutPool,
    pub(crate) bind_group_layouts: BindGroupLayoutPool,
    pub(crate) bind_groups: BindGroupPool,
    pub(crate) samplers: SamplerPool,

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

            textures: TexturePool::new(),
            renderpipelines: RenderPipelinePool::new(),
            pipeline_layouts: PipelineLayoutPool::new(),
            bind_group_layouts: BindGroupLayoutPool::new(),
            bind_groups: BindGroupPool::new(),
            samplers: SamplerPool::new(),

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self) {
        self.frame_index += 1;

        // Note that not all pools require frame maintenance.
        // (the ones that don't don't do any resource cleanup as their resources are lightweight and rare enough)
        self.renderpipelines.frame_maintenance(self.frame_index);
        // Bind group maintenance must come before texture/buffer maintenance since it registers texture/buffer use
        self.bind_groups
            .frame_maintenance(self.frame_index, &mut self.textures);
        self.textures.frame_maintenance(self.frame_index);
    }

    pub(crate) fn output_format_color(&self) -> wgpu::TextureFormat {
        self.output_format_color
    }
}
