use std::sync::atomic::AtomicU64;

use crate::debug_label::DebugLabel;

use super::{pipeline_layout_pool::*, resource_pool::*};

slotmap::new_key_type! { pub(crate) struct RenderPipelineHandle; }

pub(crate) struct RenderPipeline {
    usage_state: AtomicU64,
    pub(crate) pipeline: wgpu::RenderPipeline,
}

impl UsageTrackedResource for RenderPipeline {
    fn usage_state(&self) -> &AtomicU64 {
        &self.usage_state
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct ShaderDesc {
    // TODO(andreas) needs to be a path for reloading.
    // Our goal is to have shipped software embed the source (single file yay) and any development state reload automatically
    pub shader_code: String,
    pub entry_point: &'static str,
}

/// Renderpipeline descriptor, can be converted into [`wgpu::RenderPipeline`] (which isn't hashable or comparable)
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct RenderPipelineDesc {
    /// Debug label of the pipeline. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    pub pipeline_layout: PipelineLayoutHandle,

    pub vertex_shader: ShaderDesc,
    pub fragment_shader: ShaderDesc,

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
    ) -> RenderPipelineHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(andreas): Stop reading. Think. Add error handling. Some pointers https://github.com/gfx-rs/wgpu/issues/2130
            // TODO(andreas): Shader need to be managed separately - it's not uncommon to reuse a vertex shader across many pipelines.
            // TODO(andreas): Flawed assumption to have separate source per shader module. May or may not be the case!
            let vertex_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("vertex shader - {:?}", desc.label.get())),
                source: wgpu::ShaderSource::Wgsl(desc.vertex_shader.shader_code.clone().into()),
            });
            let fragment_shader_module =
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&format!("fragment shader - {:?}", desc.label.get())),
                    source: wgpu::ShaderSource::Wgsl(
                        desc.fragment_shader.shader_code.clone().into(),
                    ),
                });

            // TODO(andreas): Manage pipeline layouts similar to other pools
            let pipeline_layout = pipeline_layout_pool
                .get_resource(desc.pipeline_layout)
                .unwrap();

            let wgpu_desc = wgpu::RenderPipelineDescriptor {
                label: desc.label.get(),
                layout: Some(&pipeline_layout.layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: desc.vertex_shader.entry_point,
                    buffers: &desc.vertex_buffers,
                },
                primitive: desc.primitive,
                depth_stencil: desc.depth_stencil.clone(),
                multisample: desc.multisample,
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: desc.fragment_shader.entry_point,
                    targets: &desc.render_targets,
                }),
                multiview: None, // Multi-layered render target support isn't widespread
            };

            RenderPipeline {
                usage_state: AtomicU64::new(0),
                pipeline: device.create_render_pipeline(&wgpu_desc),
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        // TODO(andreas) shader reloading goes here

        // Kill any renderpipelines that haven't been used in this last frame
        self.pool.discard_unused_resources(frame_index);
    }
}

impl<'a> ResourcePoolFacade<'a, RenderPipelineHandle, RenderPipelineDesc, RenderPipeline>
    for RenderPipelinePool
{
    fn pool(&'a self) -> &ResourcePool<RenderPipelineHandle, RenderPipelineDesc, RenderPipeline> {
        &self.pool
    }
}
