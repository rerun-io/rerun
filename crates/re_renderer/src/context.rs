use crate::resource_pools::{
    bind_group_layout_pool::BindGroupLayoutPool, bind_group_pool::BindGroupPool,
    pipeline_layout_pool::PipelineLayoutPool, render_pipeline_pool::RenderPipelinePool,
    texture_pool::TexturePool,
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

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self) {
        self.frame_index += 1;
        self.textures.frame_maintenance(self.frame_index);
        self.renderpipelines.frame_maintenance(self.frame_index);
        self.pipeline_layouts.frame_maintenance(self.frame_index);
        self.bind_group_layouts.frame_maintenance(self.frame_index);
        self.bind_groups.frame_maintenance(self.frame_index);
    }

    pub(crate) fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format_color
    }
}
