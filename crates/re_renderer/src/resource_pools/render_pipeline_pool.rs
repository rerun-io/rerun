use std::sync::atomic::{AtomicU64, Ordering};

use crate::debug_label::DebugLabel;

use super::{pipeline_layout_pool::*, resource_pool::*, shader_module_pool::*};

slotmap::new_key_type! { pub(crate) struct RenderPipelineHandle; }

pub(crate) struct RenderPipeline {
    last_frame_used: AtomicU64,
    pub(crate) pipeline: wgpu::RenderPipeline,
}

impl UsageTrackedResource for RenderPipeline {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

/// Renderpipeline descriptor, can be converted into [`wgpu::RenderPipeline`] (which isn't hashable or comparable)
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct RenderPipelineDesc {
    /// Debug label of the pipeline. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    pub pipeline_layout: PipelineLayoutHandle,

    // TODO: eeeeeeeeeeehhhhhhhhhhhhhh
    pub vertex_handle: ShaderModuleHandle,
    pub vertex_entrypoint: String,
    pub fragment_handle: ShaderModuleHandle,
    pub fragment_entrypoint: String,

    /// The format of any vertex buffers used with this pipeline.
    // TODO(andreas) use SmallVec or simliar, limited to <?>
    pub vertex_buffers: Vec<wgpu::VertexBufferLayout<'static>>,

    // TODO(andreas) use SmallVec or simliar, limited to <?>
    pub render_targets: Vec<Option<wgpu::ColorTargetState>>,

    /// The properties of the pipeline at the primitive assembly and rasterization level.
    pub primitive: wgpu::PrimitiveState,

    /// The effect of draw calls on the depth and stencil aspects of the output target, if any.
    pub depth_stencil: Option<wgpu::DepthStencilState>,

    /// The multi-sampling properties of the pipeline.
    pub multisample: wgpu::MultisampleState,
}

#[derive(Default)]
pub(crate) struct RenderPipelinePool {
    pool: ResourcePool<RenderPipelineHandle, RenderPipelineDesc, RenderPipeline>,
}

impl RenderPipelinePool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &RenderPipelineDesc,
        pipeline_layout_pool: &PipelineLayoutPool,
        shader_module_pool: &mut ShaderModulePool,
    ) -> RenderPipelineHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(andreas): Manage pipeline layouts similar to other pools
            let pipeline_layout = pipeline_layout_pool.get(desc.pipeline_layout).unwrap();
            // TODO(cmc): certainly not unwrapping here
            let vertex_shader_module = shader_module_pool.get(desc.vertex_handle).unwrap();
            let fragment_shader_module = shader_module_pool.get(desc.fragment_handle).unwrap();

            let wgpu_desc = wgpu::RenderPipelineDescriptor {
                label: desc.label.get(),
                layout: Some(&pipeline_layout.layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module.shader_module,
                    entry_point: &desc.vertex_entrypoint,
                    buffers: &desc.vertex_buffers,
                },
                fragment: wgpu::FragmentState {
                    module: &fragment_shader_module.shader_module,
                    entry_point: &desc.fragment_entrypoint,
                    targets: &desc.render_targets,
                }
                .into(),
                primitive: desc.primitive,
                depth_stencil: desc.depth_stencil.clone(),
                multisample: desc.multisample,
                multiview: None, // Multi-layered render target support isn't widespread
            };

            RenderPipeline {
                last_frame_used: AtomicU64::new(0),
                pipeline: device.create_render_pipeline(&wgpu_desc),
            }
        })
    }

    pub fn frame_maintenance(
        &mut self,
        device: &wgpu::Device,
        frame_index: u64,
        shader_modules: &mut ShaderModulePool,
        pipeline_layouts: &mut PipelineLayoutPool,
    ) {
        // Kill any renderpipelines that haven't been used in this last frame
        self.pool.discard_unused_resources(frame_index);

        let descs = self.pool.resource_descs().cloned().collect::<Vec<_>>(); // TODO
        for desc in descs {
            // Make sure the shader modules we rely on don't get GC'd!
            shader_modules.register_resource_usage(desc.vertex_handle);
            shader_modules.register_resource_usage(desc.fragment_handle);

            let last_frame_modified = {
                let vertex_last_modified = shader_modules
                    .get(desc.vertex_handle)
                    .map(|sm| sm.last_frame_modified.load(Ordering::Acquire))
                    .unwrap();
                // .unwrap_or(0);
                let fragment_last_modified = shader_modules
                    .get(desc.fragment_handle)
                    .map(|sm| sm.last_frame_modified.load(Ordering::Acquire))
                    .unwrap();
                // .unwrap_or(0);
                u64::max(vertex_last_modified, fragment_last_modified)
            };

            if last_frame_modified >= frame_index {
                println!("rebuilding shader pipeline");

                // TODO: clearly this is horrible ^_^
                let handle = self.pool.get_handle(&desc, |_| unreachable!());
                let res = self.pool.get_resource_mut(handle).unwrap(); // TODO

                // TODO(andreas): Manage pipeline layouts similar to other pools
                let pipeline_layout = pipeline_layouts.get(desc.pipeline_layout).unwrap();
                // TODO(cmc): certainly not unwrapping here
                let vertex_shader_module = shader_modules.get(desc.vertex_handle).unwrap();
                let fragment_shader_module = shader_modules.get(desc.fragment_handle).unwrap();

                let wgpu_desc = wgpu::RenderPipelineDescriptor {
                    label: desc.label.get(),
                    layout: Some(&pipeline_layout.layout),
                    vertex: wgpu::VertexState {
                        module: &vertex_shader_module.shader_module,
                        entry_point: &desc.vertex_entrypoint,
                        buffers: &desc.vertex_buffers,
                    },
                    fragment: wgpu::FragmentState {
                        module: &fragment_shader_module.shader_module,
                        entry_point: &desc.fragment_entrypoint,
                        targets: &desc.render_targets,
                    }
                    .into(),
                    primitive: desc.primitive,
                    depth_stencil: desc.depth_stencil.clone(),
                    multisample: desc.multisample,
                    multiview: None, // Multi-layered render target support isn't widespread
                };

                res.pipeline = device.create_render_pipeline(&wgpu_desc);
            }
        }
    }

    pub fn get(&self, handle: RenderPipelineHandle) -> Result<&RenderPipeline, PoolError> {
        self.pool.get_resource(handle)
    }
}
