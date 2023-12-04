use smallvec::SmallVec;

use crate::debug_label::DebugLabel;

use super::{
    pipeline_layout_pool::{GpuPipelineLayoutHandle, GpuPipelineLayoutPool},
    resource::PoolError,
    shader_module_pool::{GpuShaderModuleHandle, GpuShaderModulePool},
    static_resource_pool::{
        StaticResourcePool, StaticResourcePoolAccessor, StaticResourcePoolMemMoveAccessor,
        StaticResourcePoolReadLockAccessor,
    },
};

slotmap::new_key_type! { pub struct GpuRenderPipelineHandle; }

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
    /// Generates layouts with successive shader locations without gaps.
    pub fn from_formats(formats: impl Iterator<Item = wgpu::VertexFormat>) -> SmallVec<[Self; 4]> {
        formats
            .enumerate()
            .map(move |(location, format)| Self {
                array_stride: format.size(),
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: smallvec::smallvec![wgpu::VertexAttribute {
                    format,
                    offset: 0,
                    shader_location: location as u32,
                }],
            })
            .collect()
    }

    /// Generates attributes with successive shader locations without gaps
    pub fn attributes_from_formats(
        start_location: u32,
        formats: impl Iterator<Item = wgpu::VertexFormat>,
    ) -> SmallVec<[wgpu::VertexAttribute; 8]> {
        let mut offset = 0;
        formats
            .enumerate()
            .map(move |(location, format)| {
                let attribute = wgpu::VertexAttribute {
                    format,
                    offset,
                    shader_location: start_location + location as u32,
                };
                offset += format.size();
                attribute
            })
            .collect()
    }
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

#[derive(thiserror::Error, Debug)]
pub enum RenderPipelineCreationError {
    #[error("Referenced pipeline layout not found: {0}")]
    PipelineLayout(PoolError),

    #[error("Referenced vertex shader not found: {0}")]
    VertexShaderNotFound(PoolError),

    #[error("Referenced fragment shader not found: {0}")]
    FragmentShaderNotFound(PoolError),
}

impl RenderPipelineDesc {
    fn create_render_pipeline(
        &self,
        device: &wgpu::Device,
        pipeline_layouts: &GpuPipelineLayoutPool,
        shader_modules: &GpuShaderModulePool,
    ) -> Result<wgpu::RenderPipeline, RenderPipelineCreationError> {
        let pipeline_layouts = pipeline_layouts.resources();
        let pipeline_layout = pipeline_layouts
            .get(self.pipeline_layout)
            .map_err(RenderPipelineCreationError::PipelineLayout)?;

        let shader_modules = shader_modules.resources();
        let vertex_shader_module = shader_modules
            .get(self.vertex_handle)
            .map_err(RenderPipelineCreationError::VertexShaderNotFound)?;

        let fragment_shader_module = shader_modules
            .get(self.fragment_handle)
            .map_err(RenderPipelineCreationError::FragmentShaderNotFound)?;

        let buffers = self
            .vertex_buffers
            .iter()
            .map(|b| b.to_wgpu_desc())
            .collect::<Vec<_>>();

        Ok(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label.get(),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: vertex_shader_module,
                    entry_point: &self.vertex_entrypoint,
                    buffers: &buffers,
                },
                fragment: wgpu::FragmentState {
                    module: fragment_shader_module,
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

pub type GpuRenderPipelinePoolAccessor<'a> =
    dyn StaticResourcePoolAccessor<GpuRenderPipelineHandle, wgpu::RenderPipeline> + 'a;

pub type GpuRenderPipelinePoolMoveAccessor =
    StaticResourcePoolMemMoveAccessor<GpuRenderPipelineHandle, wgpu::RenderPipeline>;

#[derive(Default)]
pub struct GpuRenderPipelinePool {
    pool: StaticResourcePool<GpuRenderPipelineHandle, RenderPipelineDesc, wgpu::RenderPipeline>,
}

impl GpuRenderPipelinePool {
    pub fn get_or_create(
        &self,
        device: &wgpu::Device,
        desc: &RenderPipelineDesc,
        pipeline_layout_pool: &GpuPipelineLayoutPool,
        shader_module_pool: &GpuShaderModulePool,
    ) -> GpuRenderPipelineHandle {
        self.pool.get_or_create(desc, |desc| {
            sanity_check_vertex_buffers(&desc.vertex_buffers);

            // TODO(cmc): certainly not unwrapping here
            desc.create_render_pipeline(device, pipeline_layout_pool, shader_module_pool)
                .unwrap()
        })
    }

    pub fn begin_frame(
        &mut self,
        device: &wgpu::Device,
        frame_index: u64,
        shader_modules: &GpuShaderModulePool,
        pipeline_layouts: &GpuPipelineLayoutPool,
    ) {
        re_tracing::profile_function!();
        self.pool.current_frame_index = frame_index;

        // Recompile render pipelines referencing shader modules that have been recompiled this frame.
        self.pool.recreate_resources(|desc| {
            let frame_created = {
                let shader_modules = shader_modules.resources();
                let vertex_created = shader_modules
                    .get_statistics(desc.vertex_handle)
                    .map(|sm| sm.frame_created)
                    .unwrap_or(0);
                let fragment_created = shader_modules
                    .get_statistics(desc.fragment_handle)
                    .map(|sm| sm.frame_created)
                    .unwrap_or(0);
                u64::max(vertex_created, fragment_created)
            };
            // The frame counter just got bumped by one. So any shader that has `frame_created`,
            // equal the current frame now, must have been recompiled since the user didn't have a
            // chance yet to add new shaders for this frame!
            // (note that this assumes that shader `begin_frame` happens before pipeline `begin_frame`)
            if frame_created < frame_index {
                return None;
            }

            match desc.create_render_pipeline(device, pipeline_layouts, shader_modules) {
                Ok(sm) => {
                    // We don't know yet if this actually succeeded.
                    // But it's good to get feedback to the user that _something_ happened!
                    re_log::info!(label = desc.label.get(), "recompiled render pipeline");
                    Some(sm)
                }
                Err(err) => {
                    re_log::error!("Failed to compile render pipeline: {}", err);
                    None
                }
            }
        });
    }

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(
        &self,
    ) -> StaticResourcePoolReadLockAccessor<'_, GpuRenderPipelineHandle, wgpu::RenderPipeline> {
        self.pool.resources()
    }

    /// Takes out all resources from the pool.
    ///
    /// This is useful when the existing resources need to be accessed without
    /// taking a lock on the pool.
    /// Resource can be put with `return_resources`.
    pub fn take_resources(&mut self) -> GpuRenderPipelinePoolMoveAccessor {
        self.pool.take_resources()
    }

    /// Counterpart to `take_resources`.
    ///
    /// Logs an error if resources were added to the pool since `take_resources` was called.
    pub fn return_resources(&mut self, resources: GpuRenderPipelinePoolMoveAccessor) {
        self.pool.return_resources(resources);
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}

fn sanity_check_vertex_buffers(buffers: &[VertexBufferLayout]) {
    if buffers.is_empty() {
        return;
    }

    let mut locations = std::collections::BTreeSet::<u32>::default();
    let mut num_attributes: u32 = 0;

    for buffer in buffers {
        for attribute in &buffer.attributes {
            num_attributes += 1;
            assert!(
                locations.insert(attribute.shader_location),
                "Duplicate shader location {} in vertex buffers",
                attribute.shader_location
            );
        }
    }

    for i in 0..num_attributes {
        // This is technically allowed, but weird.
        assert!(
            locations.contains(&i),
            "Missing shader location {i} in vertex buffers"
        );
    }
}
