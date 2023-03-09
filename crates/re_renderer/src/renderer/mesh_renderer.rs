//! Mesh renderer.
//!
//! Uses instancing to render instances of the same mesh in a single draw call.
//! Instance data is kept in an instance-stepped vertex data.

use std::sync::Arc;

use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{
    include_file,
    mesh::{gpu_data::MaterialUniformBuffer, mesh_vertices, GpuMesh, Mesh},
    renderer::OutlineMaskProcessor,
    resource_managers::GpuMeshHandle,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupLayoutDesc, BufferDesc, GpuBindGroupLayoutHandle, GpuBuffer,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc, ShaderModuleDesc,
    },
    Color32,
};

use super::{
    DrawData, DrawPhase, FileResolver, FileSystem, OutlineMaskPreference, RenderContext, Renderer,
    SharedRendererData, WgpuResourcePools,
};

mod gpu_data {
    use ecolor::Color32;

    use crate::{mesh::mesh_vertices, wgpu_resources::VertexBufferLayout};

    /// Element in the gpu residing instance buffer.
    ///
    /// Keep in sync with `mesh_vertex.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct InstanceData {
        // Don't use aligned glam types because they enforce alignment.
        // (staging buffer might be 4 byte aligned only!)
        pub world_from_mesh_row_0: [f32; 4],
        pub world_from_mesh_row_1: [f32; 4],
        pub world_from_mesh_row_2: [f32; 4],

        pub world_from_mesh_normal_row_0: [f32; 3],
        pub world_from_mesh_normal_row_1: [f32; 3],
        pub world_from_mesh_normal_row_2: [f32; 3],

        pub additive_tint: Color32,
        // Need only the first two bytes, but we want to keep everything aligned to at least 4 bytes.
        pub outline_mask: [u8; 4],
    }

    impl InstanceData {
        pub fn vertex_buffer_layout() -> VertexBufferLayout {
            let shader_start_location = mesh_vertices::next_free_shader_location();

            VertexBufferLayout {
                array_stride: std::mem::size_of::<InstanceData>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: VertexBufferLayout::attributes_from_formats(
                    shader_start_location,
                    [
                        // Affine mesh transform.
                        wgpu::VertexFormat::Float32x4,
                        wgpu::VertexFormat::Float32x4,
                        wgpu::VertexFormat::Float32x4,
                        // Transposed inverse mesh transform.
                        wgpu::VertexFormat::Float32x3,
                        wgpu::VertexFormat::Float32x3,
                        wgpu::VertexFormat::Float32x3,
                        // Tint color
                        wgpu::VertexFormat::Unorm8x4,
                        // Outline mask.
                        // This adds a tiny bit of overhead to all instances during non-outline pass, but the alternative is having yet another vertex buffer.
                        wgpu::VertexFormat::Uint8x2,
                    ]
                    .into_iter(),
                ),
            }
        }
    }
}

#[derive(Clone)]
struct MeshBatch {
    mesh: GpuMesh,
    count: u32,
    /// Number of meshes out of `count` which have outlines.
    /// We put all instances with outlines at the start of the instance buffer range.
    count_with_outlines: u32,
}

#[derive(Clone)]
pub struct MeshDrawData {
    // There is a single instance buffer for all instances of all meshes.
    // This means we only ever need to bind the instance buffer once and then change the
    // instance range on every instanced draw call!
    instance_buffer: Option<GpuBuffer>,
    batches: Vec<MeshBatch>,
}

impl DrawData for MeshDrawData {
    type Renderer = MeshRenderer;
}

pub struct MeshInstance {
    /// Gpu mesh this instance refers to.
    pub gpu_mesh: GpuMeshHandle,

    /// Optional cpu representation of the mesh, not needed for rendering.
    pub mesh: Option<Arc<Mesh>>,

    /// Where this instance is placed in world space and how its oriented & scaled.
    pub world_from_mesh: macaw::Affine3A,

    /// Per-instance (as opposed to per-material/mesh!) tint color that is added to the albedo texture.
    /// Alpha channel is currently unused.
    pub additive_tint: Color32,

    /// Optional outline mask setting for this instance.
    pub outline_mask: OutlineMaskPreference,
}

impl Default for MeshInstance {
    fn default() -> Self {
        Self {
            gpu_mesh: GpuMeshHandle::Invalid,
            mesh: None,
            world_from_mesh: macaw::Affine3A::IDENTITY,
            additive_tint: Color32::TRANSPARENT,
            outline_mask: OutlineMaskPreference::NONE,
        }
    }
}

impl MeshDrawData {
    /// Transforms and uploads mesh instance data to be consumed by gpu.
    ///
    /// Try bundling all mesh instances into a single draw data instance whenever possible.
    /// If you pass zero mesh instances, subsequent drawing will do nothing.
    /// Mesh data itself is gpu uploaded if not already present.
    pub fn new(ctx: &mut RenderContext, instances: &[MeshInstance]) -> anyhow::Result<Self> {
        crate::profile_function!();

        let _mesh_renderer = ctx.renderers.write().get_or_create::<_, MeshRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if instances.is_empty() {
            return Ok(MeshDrawData {
                batches: Vec::new(),
                instance_buffer: None,
            });
        }

        // Group by mesh to facilitate instancing.

        // TODO(andreas): Use a temp allocator
        let instance_buffer_size =
            (std::mem::size_of::<gpu_data::InstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "MeshDrawData instance buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        let mut batches = Vec::new();
        {
            let mut instance_buffer_staging = ctx
                .cpu_write_gpu_read_belt
                .lock()
                .allocate::<gpu_data::InstanceData>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                instances.len(),
            );

            let mesh_manager = ctx.mesh_manager.read();
            let mut num_processed_instances = 0;
            // TODO(#1530) This grouping doesn't seem to do its job correctly. We're not actually batching correctly right now in all cases.
            for (mesh, instances) in &instances.iter().group_by(|instance| &instance.gpu_mesh) {
                let mut count = 0;
                let mut count_with_outlines = 0;

                // Put all instances with outlines at the start of the instance buffer range.
                let instances = instances
                    .sorted_by(|a, b| a.outline_mask.is_none().cmp(&b.outline_mask.is_none()));

                for instance in instances {
                    count += 1;
                    count_with_outlines += instance.outline_mask.is_some() as u32;

                    let world_from_mesh_mat3 = instance.world_from_mesh.matrix3;
                    let world_from_mesh_normal =
                        instance.world_from_mesh.matrix3.inverse().transpose();
                    instance_buffer_staging.push(gpu_data::InstanceData {
                        world_from_mesh_row_0: world_from_mesh_mat3
                            .row(0)
                            .extend(instance.world_from_mesh.translation.x)
                            .to_array(),
                        world_from_mesh_row_1: world_from_mesh_mat3
                            .row(1)
                            .extend(instance.world_from_mesh.translation.y)
                            .to_array(),
                        world_from_mesh_row_2: world_from_mesh_mat3
                            .row(2)
                            .extend(instance.world_from_mesh.translation.z)
                            .to_array(),
                        world_from_mesh_normal_row_0: world_from_mesh_normal.row(0).to_array(),
                        world_from_mesh_normal_row_1: world_from_mesh_normal.row(1).to_array(),
                        world_from_mesh_normal_row_2: world_from_mesh_normal.row(2).to_array(),
                        additive_tint: instance.additive_tint,
                        outline_mask: instance
                            .outline_mask
                            .0
                            .map_or([0, 0, 0, 0], |mask| [mask[0], mask[1], 0, 0]),
                    });
                }
                num_processed_instances += count;

                // We resolve the meshes here already, so the actual draw call doesn't need to know about the MeshManager.
                let mesh = mesh_manager.get(mesh)?;
                batches.push(MeshBatch {
                    mesh: mesh.clone(),
                    count: count as _,
                    count_with_outlines,
                });
            }
            assert_eq!(num_processed_instances, instances.len());
            instance_buffer_staging.copy_to_buffer(
                ctx.active_frame.encoder.lock().get(),
                &instance_buffer,
                0,
            );
        }

        Ok(MeshDrawData {
            batches,
            instance_buffer: Some(instance_buffer),
        })
    }
}

pub struct MeshRenderer {
    render_pipeline_shaded: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    pub bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for MeshRenderer {
    type RendererDrawData = MeshDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Opaque, DrawPhase::OutlineMask]
    }

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        crate::profile_function!();

        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<MaterialUniformBuffer>() as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                ],
            },
        );
        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "mesh renderer".into(),
                source: include_file!("../../shader/instanced_mesh.wgsl"),
            },
        );

        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None, //Some(wgpu::Face::Back), // TODO(andreas): Need to specify from outside if mesh is CW or CCW?
            ..Default::default()
        };
        // Put instance vertex buffer on slot 0 since it doesn't change for several draws.
        let vertex_buffers: smallvec::SmallVec<[_; 4]> =
            std::iter::once(gpu_data::InstanceData::vertex_buffer_layout())
                .chain(mesh_vertices::vertex_buffer_layouts())
                .collect();

        let render_pipeline_shaded = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "mesh renderer - shaded".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main_shaded".into(),
                fragment_handle: shader_module,
                vertex_buffers: vertex_buffers.clone(),
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive,
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        let render_pipeline_outline_mask = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "mesh renderer - outline mask".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main_outline_mask".into(),
                fragment_handle: shader_module,
                vertex_buffers,
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                primitive,
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(
                    shared_data.config.hardware_tier,
                ),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        MeshRenderer {
            render_pipeline_shaded,
            render_pipeline_outline_mask,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

        let Some(instance_buffer) = &draw_data.instance_buffer else {
            return Ok(()); // Instance buffer was empty.
        };

        let pipeline_handle = if phase == DrawPhase::OutlineMask {
            self.render_pipeline_outline_mask
        } else {
            self.render_pipeline_shaded
        };
        let pipeline = pools.render_pipelines.get_resource(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        pass.set_vertex_buffer(0, instance_buffer.slice(..));
        let mut instance_start_index = 0;

        for mesh_batch in &draw_data.batches {
            if phase == DrawPhase::OutlineMask && mesh_batch.count_with_outlines == 0 {
                instance_start_index += mesh_batch.count;
                continue;
            }

            let vertex_buffer_combined = &mesh_batch.mesh.vertex_buffer_combined;
            let index_buffer = &mesh_batch.mesh.index_buffer;

            pass.set_vertex_buffer(
                1,
                vertex_buffer_combined.slice(mesh_batch.mesh.vertex_buffer_positions_range.clone()),
            );
            pass.set_vertex_buffer(
                2,
                vertex_buffer_combined.slice(mesh_batch.mesh.vertex_buffer_data_range.clone()),
            );
            pass.set_index_buffer(
                index_buffer.slice(mesh_batch.mesh.index_buffer_range.clone()),
                wgpu::IndexFormat::Uint32,
            );

            let num_meshes_to_draw = if phase == DrawPhase::OutlineMask {
                mesh_batch.count_with_outlines
            } else {
                mesh_batch.count
            };
            let instance_range = instance_start_index..(instance_start_index + num_meshes_to_draw);

            for material in &mesh_batch.mesh.materials {
                debug_assert!(num_meshes_to_draw > 0);

                pass.set_bind_group(1, &material.bind_group, &[]);
                pass.draw_indexed(material.index_range.clone(), 0, instance_range.clone());
            }

            // Advance instance start index with *total* number of instances in this batch.
            instance_start_index += mesh_batch.count;
        }

        Ok(())
    }
}
