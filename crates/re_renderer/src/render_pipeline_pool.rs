use slotmap::new_key_type;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::resource_pool::*;

new_key_type! { pub(crate) struct RenderPipelineHandle; }

pub(crate) struct RenderPipeline {
    last_frame_used: AtomicU64,
    pub(crate) pipeline: wgpu::RenderPipeline,
}

impl Resource for RenderPipeline {
    fn register_use(&self, current_frame_index: u64) {
        // TODO: register use for dependent resources?
        self.last_frame_used
            .fetch_max(current_frame_index, Ordering::Relaxed);
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct ShaderDesc {
    // TODO(andreas) needs to be a path for reloading.
    // Our goal is to have shipped software embed the source (single file yay) and any development state reload automatically
    pub shader_code: String,
    pub entry_point: &'static str,
}

/// Renderpipeline descriptor, can be converted into wgpu::RenderPipeline (which isn't hashable or comparable)
#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct RenderPipelineDesc {
    /// Debug label of the pipeline. This will show up in graphics debuggers for easy identification.
    pub label: String,

    // TODO(andreas) make it easier to re-use bindgroup layouts
    // TODO(andreas) use SmallVec or simliar, limited to 4
    pub pipeline_layout: Vec<Vec<wgpu::BindGroupLayoutEntry>>,

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

pub(crate) struct RenderPipelinePool {
    pool: ResourcePool<RenderPipelineHandle, RenderPipelineDesc, RenderPipeline>,
}

impl RenderPipelinePool {
    pub fn new() -> Self {
        RenderPipelinePool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request_render_pipeline(
        &mut self,
        device: &wgpu::Device,
        desc: &RenderPipelineDesc,
    ) -> RenderPipelineHandle {
        self.pool.request(desc, |desc| {
            // TODO(andreas): Stop reading. Think. Add error handling. Some pointers https://github.com/gfx-rs/wgpu/issues/2130
            // TODO(andreas): Shader need to be managed separately - it's not uncommon to reuse a vertex shader across many pipelines.
            // TODO(andreas): Flawed assumption to have separate source per shader module. May or may not be the case!
            let vertex_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("vertex shader - {}", desc.label)),
                source: wgpu::ShaderSource::Wgsl(desc.vertex_shader.shader_code.clone().into()),
            });
            let fragment_shader_module =
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&format!("fragment shader - {}", desc.label)),
                    source: wgpu::ShaderSource::Wgsl(
                        desc.fragment_shader.shader_code.clone().into(),
                    ),
                });

            // TODO(andreas): Manage pipeline/bindgroup layouts similar to other pools. Important difference though that a user won't need a handle, so we can do special stuff there?
            let bind_group_layouts = desc
                .pipeline_layout
                .iter()
                .map(|layout_entries| {
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None, // TODO:
                        entries: &layout_entries,
                    })
                })
                .collect::<Vec<wgpu::BindGroupLayout>>();
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&desc.label),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|layout| layout)
                    .collect::<Vec<&wgpu::BindGroupLayout>>(),
                push_constant_ranges: &[], // Sadly, push constants aren't widely enough supported yet.
            });

            let wgpu_desc = wgpu::RenderPipelineDescriptor {
                label: Some(&desc.label),
                layout: Some(&pipeline_layout),
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
                last_frame_used: AtomicU64::new(0),
                pipeline: device.create_render_pipeline(&wgpu_desc),
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        // TODO(andreas) shader reloading goes here
        self.pool.frame_maintenance(frame_index);
    }

    pub fn render_pipeline(
        &self,
        handle: RenderPipelineHandle,
    ) -> Result<&RenderPipeline, PoolError> {
        self.pool.resource(handle)
    }
}
