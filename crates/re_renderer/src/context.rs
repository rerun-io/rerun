use std::sync::Arc;

use type_map::concurrent::{self, TypeMap};

use crate::{
    config::RenderContextConfig,
    global_bindings::GlobalBindings,
    renderer::Renderer,
    resource_managers::{MeshManager, TextureManager2D, TextureManager3D},
    wgpu_resources::WgpuResourcePools,
    FileResolver, FileServer, FileSystem, RecommendedFileResolver,
};

// ---

/// Any resource involving wgpu rendering which can be re-used across different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,

    pub(crate) shared_renderer_data: SharedRendererData,
    pub(crate) renderers: Renderers,
    pub(crate) resolver: RecommendedFileResolver,
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
    pub(crate) err_tracker: std::sync::Arc<crate::error_tracker::ErrorTracker>,

    pub gpu_resources: WgpuResourcePools,
    pub mesh_manager: MeshManager,
    pub texture_manager_2d: TextureManager2D,
    pub texture_manager_3d: TextureManager3D,

    // TODO(andreas): Add frame/lifetime statistics, shared resources (e.g. "global" uniform buffer), ??
    frame_index: u64,
}

/// Immutable data that is shared between all [`Renderer`]
pub struct SharedRendererData {
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
    pub fn get_or_create<Fs: FileSystem, R: 'static + Renderer + Send + Sync>(
        &mut self,
        shared_data: &SharedRendererData,
        resource_pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> &R {
        self.renderers.entry().or_insert_with(|| {
            crate::profile_scope!("create_renderer", std::any::type_name::<R>());
            R::create_renderer(shared_data, resource_pools, device, resolver)
        })
    }

    pub fn get<R: 'static + Renderer>(&self) -> Option<&R> {
        self.renderers.get::<R>()
    }
}

impl RenderContext {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: RenderContextConfig,
    ) -> Self {
        let mut gpu_resources = WgpuResourcePools::default();
        let global_bindings = GlobalBindings::new(&mut gpu_resources, &device);

        // Validate capabilities of the device.
        assert!(
            config.hardware_tier.limits().check_limits(&device.limits()),
            "The given device doesn't support the required limits for the given hardware tier {:?}.
            Required:
            {:?}
            Actual:
            {:?}",
            config.hardware_tier,
            config.hardware_tier.limits(),
            device.limits(),
        );
        assert!(
            device.features().contains(config.hardware_tier.features()),
            "The given device doesn't support the required features for the given hardware tier {:?}.
            Required:
            {:?}
            Actual:
            {:?}",
            config.hardware_tier,
            config.hardware_tier.features(),
            device.features(),
        );
        // Can't check downlevel feature flags since they sit on the adapter, not on the device.

        // In debug builds, make sure to catch all errors, never crash, and try to
        // always let the user find a way to return a poisoned pipeline back into a
        // sane state.
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        let err_tracker = {
            let err_tracker = std::sync::Arc::new(crate::error_tracker::ErrorTracker::default());
            device.on_uncaptured_error({
                let err_tracker = std::sync::Arc::clone(&err_tracker);
                Box::new(move |err| err_tracker.handle_error(err))
            });
            err_tracker
        };

        let shared_renderer_data = SharedRendererData {
            config,
            global_bindings,
        };

        let mut resolver = crate::new_recommended_file_resolver();
        let mut renderers = Renderers {
            renderers: TypeMap::new(),
        };

        let mesh_manager = MeshManager::new(
            device.clone(),
            queue.clone(),
            renderers.get_or_create(
                &shared_renderer_data,
                &mut gpu_resources,
                &device,
                &mut resolver,
            ),
        );
        let texture_manager_2d =
            TextureManager2D::new(device.clone(), queue.clone(), &mut gpu_resources.textures);
        let texture_manager_3d =
            TextureManager3D::new(device.clone(), queue.clone(), &mut gpu_resources.textures);

        RenderContext {
            device,
            queue,

            shared_renderer_data,

            renderers,
            gpu_resources,

            mesh_manager,
            texture_manager_2d,
            texture_manager_3d,

            resolver,

            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
            err_tracker,

            frame_index: 0,
        }
    }

    /// Call this at the beginning of a new frame.
    ///
    /// Updates internal book-keeping, frame allocators and executes delayed events like shader reloading.
    pub fn frame_maintenance(&mut self) {
        self.frame_index += 1;

        // Tick the error tracker so that it knows when to reset!
        // Note that we're ticking on frame_maintenance rather than raw frames, which
        // makes a world of difference when we're in a poisoned state.
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        self.err_tracker.tick();

        // The set of files on disk that were modified in any way since last frame,
        // ignoring deletions.
        // Always an empty set in release builds.
        let modified_paths = FileServer::get_mut(|fs| fs.collect(&mut self.resolver));
        if !modified_paths.is_empty() {
            re_log::debug!(?modified_paths, "got some filesystem events");
        }

        self.mesh_manager.frame_maintenance(self.frame_index);
        self.texture_manager_2d.frame_maintenance(self.frame_index);

        {
            let WgpuResourcePools {
                bind_group_layouts,
                bind_groups,
                pipeline_layouts,
                render_pipelines,
                samplers,
                shader_modules,
                textures,
                buffers,
            } = &mut self.gpu_resources; // not all pools require maintenance

            // Shader module maintenance must come before render pipelines because render pipeline
            // recompilation picks up all shaders that have been recompiled this frame.
            shader_modules.frame_maintenance(
                &self.device,
                &mut self.resolver,
                self.frame_index,
                &modified_paths,
            );
            render_pipelines.frame_maintenance(
                &self.device,
                self.frame_index,
                shader_modules,
                pipeline_layouts,
            );

            // Bind group maintenance must come before texture/buffer maintenance since it
            // registers texture/buffer use
            bind_groups.frame_maintenance(self.frame_index, textures, buffers, samplers);

            textures.frame_maintenance(self.frame_index);
            buffers.frame_maintenance(self.frame_index);

            pipeline_layouts.frame_maintenance(self.frame_index);
            bind_group_layouts.frame_maintenance(self.frame_index);
            samplers.frame_maintenance(self.frame_index);
        }
    }
}

/// Gets allocation size for a uniform buffer padded in a way that multiple can be put in a single wgpu buffer.
///
/// TODO(andreas): Once we have higher level buffer allocators this should be handled there.
pub(crate) fn uniform_buffer_allocation_size<Data>(device: &wgpu::Device) -> u64 {
    let uniform_buffer_size = std::mem::size_of::<Data>();
    wgpu::util::align_to(
        uniform_buffer_size as u32,
        device.limits().min_uniform_buffer_offset_alignment,
    ) as u64
}
