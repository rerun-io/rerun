use type_map::concurrent::{self, TypeMap};

use crate::{renderer::Renderer, resource_pools::WgpuResourcePools};

/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    pub(crate) config: RenderContextConfig,
    pub(crate) renderers: Renderers,
    pub(crate) resource_pools: WgpuResourcePools,

    // TODO(andreas): Add frame/lifetime statistics, shared resources (e.g. "global" uniform buffer), ??
    frame_index: u64,
}

pub struct RenderContextConfig {
    /// The color format used by the eframe output buffer.
    pub output_format_color: wgpu::TextureFormat,
}

pub(crate) struct Renderers {
    renderers: concurrent::TypeMap,
}

impl Renderers {
    pub fn get_or_create<R: Renderer + 'static + Send + Sync>(
        &mut self,
        ctx_config: &RenderContextConfig,
        resource_pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> &R {
        self.renderers
            .entry()
            .or_insert_with(|| R::new(ctx_config, resource_pools, device))
    }

    pub fn get<R: Renderer + 'static>(&self) -> Option<&R> {
        self.renderers.get::<R>()
    }
}

impl RenderContext {
    pub fn new(_device: &wgpu::Device, _queue: &wgpu::Queue, config: RenderContextConfig) -> Self {
        RenderContext {
            config,

            renderers: Renderers {
                renderers: TypeMap::new(),
            },
            resource_pools: WgpuResourcePools::default(),

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self) {
        {
            let WgpuResourcePools {
                textures,
                render_pipelines,
                pipeline_layouts: _,
                bind_group_layouts: _,
                bind_groups,
                samplers,
            } = &mut self.resource_pools; // not all pools require maintenance

            render_pipelines.frame_maintenance(self.frame_index);

            // Bind group maintenance must come before texture/buffer maintenance since it
            // registers texture/buffer use
            bind_groups.frame_maintenance(self.frame_index, textures, samplers);

            textures.frame_maintenance(self.frame_index);
        }

        self.frame_index += 1;
    }
}
