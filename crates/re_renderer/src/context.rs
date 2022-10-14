use type_map::{TypeMap, VacantEntry};

use crate::{
    renderer::renderer::Renderer,
    resource_pools::{
        bind_group_layout_pool::BindGroupLayoutPool, bind_group_pool::BindGroupPool,
        pipeline_layout_pool::PipelineLayoutPool, render_pipeline_pool::RenderPipelinePool,
        sampler_pool::SamplerPool, texture_pool::TexturePool,
    },
};

/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    /// The color format used by the eframe output buffer.
    output_format_color: wgpu::TextureFormat,

    renderers: TypeMap,

    pub(crate) textures: TexturePool,
    pub(crate) render_pipelines: RenderPipelinePool,
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

            renderers: TypeMap::new(),

            textures: TexturePool::default(),
            render_pipelines: RenderPipelinePool::default(),
            pipeline_layouts: PipelineLayoutPool::default(),
            bind_group_layouts: BindGroupLayoutPool::default(),
            bind_groups: BindGroupPool::default(),
            samplers: SamplerPool::default(),

            frame_index: 0,
        }
    }

    pub fn get_or_create_renderer<R: Renderer + 'static>(&mut self, device: &wgpu::Device) -> &R {
        if !self.renderers.contains::<R>() {
            let renderer = R::new(self, device);
            self.renderers.insert(renderer);
        }
        self.renderers.get::<R>().unwrap()
    }

    pub fn get_renderer<R: Renderer + 'static>(&self) -> Option<&R> {
        self.renderers.get::<R>()
    }

    pub fn frame_maintenance(&mut self) {
        let Self {
            output_format_color: _,

            renderers: _,

            textures,
            render_pipelines,
            pipeline_layouts: _,
            bind_group_layouts: _,
            bind_groups,
            samplers: _,

            frame_index,
        } = self; // not all pools require maintenance

        *frame_index += 1;

        render_pipelines.frame_maintenance(*frame_index);

        // Bind group maintenance must come before texture/buffer maintenance since it
        // registers texture/buffer use
        bind_groups.frame_maintenance(*frame_index, textures);

        textures.frame_maintenance(*frame_index);
    }

    pub(crate) fn output_format_color(&self) -> wgpu::TextureFormat {
        self.output_format_color
    }
}
