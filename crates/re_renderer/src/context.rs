use ahash::{HashSet, HashSetExt};
use type_map::concurrent::{self, TypeMap};

use crate::FileWatcher;
use crate::{
    global_bindings::GlobalBindings, renderer::Renderer, resource_pools::WgpuResourcePools,
};

/// Any resource involving wgpu rendering which can be re-used across different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    pub(crate) shared_renderer_data: SharedRendererData,
    pub(crate) renderers: Renderers,
    pub(crate) resource_pools: WgpuResourcePools,

    // TODO(andreas): Add frame/lifetime statistics, shared resources (e.g. "global" uniform buffer), ??
    frame_index: u64,
}

/// Startup configuration for a [`RenderContext`]
///
/// Contains any kind of configuration that doesn't change for the entire lifetime of a [`RenderContext`].
/// (flipside, if we do want to change any of these, the [`RenderContext`] needs to be re-created)
pub struct RenderContextConfig {
    /// The color format used by the eframe output buffer.
    pub output_format_color: wgpu::TextureFormat,
}

/// Immutable data that is shared between all [`Renderer`]
pub(crate) struct SharedRendererData {
    pub(crate) config: RenderContextConfig,

    /// Global bindings, always bound to 0 bind group slot zero.
    /// [`Renderer`] are not allowed to use bind group 0 themselves!
    pub(crate) global_bindings: GlobalBindings,
}

/// Struct owning *all* [`Renderer`].
/// [`Renderer`] are created lazily and stay around indefinitely.
pub(crate) struct Renderers {
    renderers: concurrent::TypeMap,
}

impl Renderers {
    pub fn get_or_create<R: 'static + Renderer + Send + Sync>(
        &mut self,
        shared_data: &SharedRendererData,
        resource_pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> &R {
        self.renderers
            .entry()
            .or_insert_with(|| R::create_renderer(shared_data, resource_pools, device))
    }

    pub fn get<R: 'static + Renderer>(&self) -> Option<&R> {
        self.renderers.get::<R>()
    }
}

impl RenderContext {
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, config: RenderContextConfig) -> Self {
        let mut resource_pools = WgpuResourcePools::default();
        let global_bindings = GlobalBindings::new(&mut resource_pools, device);

        RenderContext {
            shared_renderer_data: SharedRendererData {
                config,
                global_bindings,
            },

            renderers: Renderers {
                renderers: TypeMap::new(),
            },
            resource_pools,

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self, device: &wgpu::Device) {
        let updated_paths = FileWatcher::get_mut(|fw| fw.dequeue());

        {
            let WgpuResourcePools {
                bind_group_layouts: _,
                bind_groups,
                pipeline_layouts,
                render_pipelines,
                samplers,
                shader_modules,
                textures,
            } = &mut self.resource_pools; // not all pools require maintenance

            // 1. maintain pipeline so that shader modules don't get GC
            // 2. check filewatcher to see whether modules were updated
            // 3. recreate pipelines

            // RenderPipelines refer to ShaderModules and thus must me maintained first.
            render_pipelines.frame_maintenance(
                device,
                self.frame_index,
                shader_modules,
                pipeline_layouts,
            );

            shader_modules.frame_maintenance(device, self.frame_index, &updated_paths);

            // Bind group maintenance must come before texture/buffer maintenance since it
            // registers texture/buffer use
            bind_groups.frame_maintenance(self.frame_index, textures, samplers);

            textures.frame_maintenance(self.frame_index);
        }

        self.frame_index += 1;
    }
}
