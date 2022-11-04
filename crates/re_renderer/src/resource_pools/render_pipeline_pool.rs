use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context;
use smallvec::SmallVec;

use crate::debug_label::DebugLabel;

use super::{pipeline_layout_pool::*, resource::*, shader_module_pool::*, static_resource_pool::*};

slotmap::new_key_type! { pub struct GpuRenderPipelineHandle; }

pub struct GpuRenderPipeline {
    last_frame_used: AtomicU64,
    pub(crate) pipeline: wgpu::RenderPipeline,
}

impl UsageTrackedResource for GpuRenderPipeline {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

/// A copy of [`wgpu::VertexBufferLayout`] with a [`smallvec`] for the attributes.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct VertexBufferLayout {
    /// The stride, in bytes, between elements of this buffer.
    pub array_stride: wgpu::BufferAddress,

    /// How often this vertex buffer is "stepped" forward.
    pub step_mode: wgpu::VertexStepMode,

    /// The list of attributes which comprise a single vertex.
    pub attributes: SmallVec<[wgpu::VertexAttribute; 8]>,
}

impl VertexBufferLayout {
    fn to_wgpu_desc(&self) -> wgpu::VertexBufferLayout<'_> {
        wgpu::VertexBufferLayout {
            array_stride: self.array_stride,
            step_mode: self.step_mode,
            attributes: &self.attributes,
        }
    }
}

/// Renderpipeline descriptor, can be converted into [`wgpu::RenderPipeline`] (which isn't hashable or comparable)
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct RenderPipelineDesc {
    /// Debug label of the pipeline. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    pub pipeline_layout: GpuPipelineLayoutHandle,

    pub vertex_entrypoint: String,
    pub vertex_handle: GpuShaderModuleHandle,
    pub fragment_entrypoint: String,
    pub fragment_handle: GpuShaderModuleHandle,

    /// The format of any vertex buffers used with this pipeline.
    pub vertex_buffers: SmallVec<[VertexBufferLayout; 4]>,

    /// The color state of the render targets.
    pub render_targets: SmallVec<[Option<wgpu::ColorTargetState>; 4]>,

    /// The properties of the pipeline at the primitive assembly and rasterization level.
    pub primitive: wgpu::PrimitiveState,

    /// The effect of draw calls on the depth and stencil aspects of the output target, if any.
    pub depth_stencil: Option<wgpu::DepthStencilState>,

    /// The multi-sampling properties of the pipeline.
    pub multisample: wgpu::MultisampleState,
}
impl RenderPipelineDesc {
    fn create_render_pipeline(
        &self,
        device: &wgpu::Device,
        pipeline_layouts: &GpuPipelineLayoutPool,
        shader_modules: &GpuShaderModulePool,
    ) -> anyhow::Result<wgpu::RenderPipeline> {
        let pipeline_layout = pipeline_layouts
            .get_resource(self.pipeline_layout)
            .context("referenced pipeline layout not found")?;

        let vertex_shader_module = shader_modules
            .get(self.vertex_handle)
            .context("referenced vertex shader not found")?;
        let fragment_shader_module = shader_modules
            .get(self.fragment_handle)
            .context("referenced fragment shader not found")?;

        let buffers = self
            .vertex_buffers
            .iter()
            .map(|b| b.to_wgpu_desc())
            .collect::<Vec<_>>();

        Ok(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label.get(),
                layout: Some(&pipeline_layout.layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module.shader_module,
                    entry_point: &self.vertex_entrypoint,
                    buffers: &buffers,
                },
                fragment: wgpu::FragmentState {
                    module: &fragment_shader_module.shader_module,
                    entry_point: &self.fragment_entrypoint,
                    targets: &self.render_targets,
                }
                .into(),
                primitive: self.primitive,
                depth_stencil: self.depth_stencil.clone(),
                multisample: self.multisample,
                multiview: None, // Multi-layered render target support isn't widespread
            }),
        )
    }
}

#[derive(Default)]
pub struct GpuRenderPipelinePool {
    pool: StaticResourcePool<GpuRenderPipelineHandle, RenderPipelineDesc, GpuRenderPipeline>,
}

impl GpuRenderPipelinePool {
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        desc: &RenderPipelineDesc,
        pipeline_layout_pool: &GpuPipelineLayoutPool,
        shader_module_pool: &GpuShaderModulePool,
    ) -> GpuRenderPipelineHandle {
        self.pool.get_or_create(desc, |desc| {
            GpuRenderPipeline {
                last_frame_used: AtomicU64::new(0),
                pipeline: desc
                    // TODO(cmc): certainly not unwrapping here
                    .create_render_pipeline(device, pipeline_layout_pool, shader_module_pool)
                    .unwrap(),
            }
        })
    }

    pub fn frame_maintenance(
        &mut self,
        device: &wgpu::Device,
        frame_index: u64,
        shader_modules: &mut GpuShaderModulePool,
        pipeline_layouts: &mut GpuPipelineLayoutPool,
    ) {
        // Make sure the shader modules we rely on don't get GC'd!
        for desc in self.pool.resource_descs() {
            shader_modules.register_resource_usage(desc.vertex_handle);
            shader_modules.register_resource_usage(desc.fragment_handle);
        }

        // All render pipeline descriptors that refer to shader modules that have been
        // recompiled since last frame.
        let descs = self
            .pool
            .resource_descs()
            .filter_map(|desc| {
                let last_frame_modified = {
                    let vertex_last_modified = shader_modules
                        .get(desc.vertex_handle)
                        .map(|sm| sm.last_frame_modified.load(Ordering::Acquire))
                        .unwrap_or(0);
                    let fragment_last_modified = shader_modules
                        .get(desc.fragment_handle)
                        .map(|sm| sm.last_frame_modified.load(Ordering::Acquire))
                        .unwrap_or(0);
                    u64::max(vertex_last_modified, fragment_last_modified)
                };

                // TODO(cmc): Again, not nice, we'll see how things evolve.
                (last_frame_modified >= frame_index).then(|| desc.clone())
            })
            .collect::<Vec<_>>();

        // Recompile render pipelines referencing recompiled shader modules.
        for desc in descs {
            // TODO(cmc): obviously terrible, we'll see as things evolve.
            let handle = self.pool.get_or_create(&desc, |_| {
                unreachable!("the pool itself handed us that descriptor")
            });
            let res = self
                .pool
                .get_resource_mut(handle)
                .expect("the pool itself handed us that handle");

            let render_pipeline =
                match desc.create_render_pipeline(device, pipeline_layouts, shader_modules) {
                    Ok(sm) => sm,
                    Err(err) => {
                        re_log::error!(
                            err = re_error::format(err),
                            "couldn't recompile render pipeline"
                        );
                        continue;
                    }
                };

            re_log::debug!(
                label = desc.label.get(),
                "successfully recompiled render pipeline"
            );

            res.pipeline = render_pipeline;
            // NOTE: could technically have a `last_frame_modified` like shader modules do.
        }
    }

    pub fn get_resource(
        &self,
        handle: GpuRenderPipelineHandle,
    ) -> Result<&GpuRenderPipeline, PoolError> {
        self.pool.get_resource(handle)
    }
}
