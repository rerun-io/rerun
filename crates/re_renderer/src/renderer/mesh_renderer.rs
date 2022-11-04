//! Mesh renderer.
//!
//! Uses instancing to render instances of the same mesh in a single draw call.
//! Instance data is kept in an instance-stepped vertex data, see [`GpuInstanceData`].

use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{
    include_file,
    mesh::{mesh_vertices, GpuMesh},
    mesh_manager::GpuMeshHandle,
    resource_pools::{
        buffer_pool::{BufferDesc, GpuBufferHandleStrong},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::{GpuRenderPipelineHandle, RenderPipelineDesc, VertexBufferLayout},
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

/// Element in the gpu residing instance buffer.
///
/// Keep in sync with `mesh_vertex.wgsl`
#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuInstanceData {
    // Don't use alignend glam types because they enforce alignment.
    // (staging buffer might be 4 byte aligned only!)
    translation_and_scale: [f32; 4],
    rotation: [f32; 4],
}

impl GpuInstanceData {
    pub fn vertex_buffer_layout() -> VertexBufferLayout {
        let shader_start_location = mesh_vertices::next_free_shader_location();

        VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuInstanceData>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: smallvec![
                // Position and Scale.
                // We could move scale to a separate field, it's _probably_ not gonna have any impact at all
                // But then again it's easy and less confusing to always keep them fused.
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: memoffset::offset_of!(GpuInstanceData, translation_and_scale) as _,
                    shader_location: shader_start_location,
                },
                // Rotation (quaternion)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: memoffset::offset_of!(GpuInstanceData, rotation) as _,
                    shader_location: shader_start_location + 1,
                },
            ],
        }
    }
}

#[derive(Clone)]
struct MeshBatch {
    mesh: GpuMesh,
    count: u32,
}

#[derive(Clone)]
pub struct MeshDrawable {
    // There is a single instance buffer for all instances of all meshes.
    // This means we only ever need to bind the instance buffer once and then change the
    // instance range on every instanced draw call!
    instance_buffer: GpuBufferHandleStrong,
    batches: Vec<MeshBatch>,
}

impl Drawable for MeshDrawable {
    type Renderer = MeshRenderer;
}

pub struct MeshInstance {
    pub mesh: GpuMeshHandle,
    pub world_from_mesh: macaw::Conformal3,
}

impl MeshDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[MeshInstance],
    ) -> anyhow::Result<Self> {
        let _mesh_renderer = ctx.renderers.get_or_create::<_, MeshRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        // TODO(andreas): Use a temp allocator
        let instance_buffer_size = (std::mem::size_of::<GpuInstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.resource_pools.buffers.alloc(
            device,
            &BufferDesc {
                label: "MeshDrawable instance buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        );
        let mut instance_buffer_staging = queue.write_buffer_with(
            &ctx.resource_pools
                .buffers
                .get_resource(&instance_buffer)
                .unwrap()
                .buffer,
            0,
            instance_buffer_size.try_into().unwrap(),
        );
        let instance_buffer_staging: &mut [GpuInstanceData] =
            bytemuck::cast_slice_mut(&mut instance_buffer_staging);

        // Group by mesh to facilitate instancing.
        // We resolve the meshes here already, so the actual draw call doesn't need to know about the MeshManager.
        // Also, it helps failing early if something is wrong with a mesh!
        let mut batches = Vec::new();
        let mut num_processed_instances = 0;
        for (mesh_handle, instances) in &instances.iter().group_by(|instance| instance.mesh) {
            let mesh = ctx.meshes.get_mesh(mesh_handle)?;

            let mut count = 0;
            for (instance, gpu_instance) in instances.zip(
                instance_buffer_staging
                    .iter_mut()
                    .skip(num_processed_instances),
            ) {
                count += 1;
                gpu_instance.translation_and_scale =
                    instance.world_from_mesh.translation_and_scale().into();
                gpu_instance.rotation = instance.world_from_mesh.rotation().into();
            }

            batches.push(MeshBatch {
                mesh: mesh.clone(),
                count,
            });
            num_processed_instances += count as usize;
        }
        assert_eq!(num_processed_instances, instances.len());

        Ok(MeshDrawable {
            batches,
            instance_buffer,
        })
    }
}

pub struct MeshRenderer {
    render_pipeline: GpuRenderPipelineHandle,
}

impl Renderer for MeshRenderer {
    type DrawData = MeshDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![shared_data.global_bindings.layout],
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

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "mesh renderer".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,

                // Put instance vertex buffer on slot 0 since it doesn't change for several draws.
                vertex_buffers: std::iter::once(GpuInstanceData::vertex_buffer_layout())
                    .chain(mesh_vertices::vertex_buffer_layouts())
                    .collect(),

                render_targets: smallvec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None, //Some(wgpu::Face::Back), // TODO(andreas): Need to specify from outside if mesh is CW or CCW?
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Greater,
                    depth_write_enabled: true,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        MeshRenderer { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(&pipeline.pipeline);

        let instance_buffer = pools.buffers.get_resource(&draw_data.instance_buffer)?;
        pass.set_vertex_buffer(0, instance_buffer.buffer.slice(..));
        let mut instance_start_index = 0;

        for mesh_batch in &draw_data.batches {
            let vertex_buffer_combined = pools
                .buffers
                .get_resource(&mesh_batch.mesh.vertex_buffer_combined)?;
            let index_buffer = pools.buffers.get_resource(&mesh_batch.mesh.index_buffer)?;

            pass.set_vertex_buffer(
                1,
                vertex_buffer_combined
                    .buffer
                    .slice(mesh_batch.mesh.vertex_buffer_positions_range.clone()),
            );
            pass.set_vertex_buffer(
                2,
                vertex_buffer_combined
                    .buffer
                    .slice(mesh_batch.mesh.vertex_buffer_data_range.clone()),
            );
            pass.set_index_buffer(
                index_buffer
                    .buffer
                    .slice(mesh_batch.mesh.index_buffer_range.clone()),
                wgpu::IndexFormat::Uint32,
            );

            for material in &mesh_batch.mesh.materials {
                debug_assert!(mesh_batch.count > 0);
                let instance_end_index = instance_start_index + mesh_batch.count;
                pass.draw_indexed(
                    material.index_range.clone(),
                    0,
                    instance_start_index..instance_end_index,
                );
                instance_start_index = instance_end_index;
            }
        }

        Ok(())
    }
}
