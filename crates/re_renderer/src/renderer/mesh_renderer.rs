//! Mesh renderer.
//!
//! Uses instancing to render instances of the same mesh in a single draw call.
//! Instance data is kept in an instance-stepped vertex data, see [`GpuInstanceData`].

use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{
    include_file,
    mesh::{mesh_vertices, GpuMesh},
    resource_managers::{MeshHandle, MeshManager},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupLayoutDesc, BufferDesc, GpuBindGroupLayoutHandle, GpuBufferHandleStrong,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc, ShaderModuleDesc,
        VertexBufferLayout,
    },
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
    additive_tint_srgb: [u8; 4],
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
                // Tint color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: memoffset::offset_of!(GpuInstanceData, additive_tint_srgb) as _,
                    shader_location: shader_start_location + 2,
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
    instance_buffer: Option<GpuBufferHandleStrong>,
    batches: Vec<MeshBatch>,
}

impl Drawable for MeshDrawable {
    type Renderer = MeshRenderer;
}

pub struct MeshInstance {
    pub mesh: MeshHandle,
    pub world_from_mesh: macaw::Conformal3,

    /// Per-instance (as opposed to per-material/mesh!) tint color that is added to the albedo texture.
    /// Alpha channel is currently unused.
    pub additive_tint_srgb: [u8; 4],
}

impl MeshDrawable {
    /// Transforms and uploads mesh instance data to be consumed by gpu.
    ///
    /// Try bundling all mesh instances into a single drawable whenever possible.
    /// As with all drawables, data is alive only for a single frame!
    /// If you pass zero mesh instances, subsequent drawing will do nothing.
    /// Mesh data itself is gpu uploaded if not already present.
    pub fn new(ctx: &mut RenderContext, instances: &[MeshInstance]) -> anyhow::Result<Self> {
        crate::profile_function!();

        let _mesh_renderer = ctx.renderers.get_or_create::<_, MeshRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            &ctx.device,
            &mut ctx.resolver,
        );

        if instances.is_empty() {
            return Ok(MeshDrawable {
                batches: Vec::new(),
                instance_buffer: None,
            });
        }

        // Group by mesh to facilitate instancing.

        // TODO(andreas): Use a temp allocator
        let instance_buffer_size = (std::mem::size_of::<GpuInstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.resource_pools.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "MeshDrawable instance buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        );

        let mut mesh_runs = Vec::new();
        {
            let mut instance_buffer_staging = ctx.queue.write_buffer_with(
                ctx.resource_pools
                    .buffers
                    .get_resource(&instance_buffer)
                    .unwrap(),
                0,
                instance_buffer_size.try_into().unwrap(),
            );
            let instance_buffer_staging: &mut [GpuInstanceData] =
                bytemuck::cast_slice_mut(&mut instance_buffer_staging);

            let mut num_processed_instances = 0;
            for (mesh, instances) in &instances.iter().group_by(|instance| instance.mesh) {
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
                    gpu_instance.additive_tint_srgb = instance.additive_tint_srgb;
                }
                num_processed_instances += count;
                mesh_runs.push((mesh, count as u32));
            }
            assert_eq!(num_processed_instances, instances.len());
        }

        // We resolve the meshes here already, so the actual draw call doesn't need to know about the MeshManager.
        // Also, it helps failing early if something is wrong with a mesh!
        let batches: Result<Vec<_>, _> = mesh_runs
            .into_iter()
            .map(|(mesh_handle, count)| {
                MeshManager::get_or_create_gpu_resource(ctx, mesh_handle)
                    .map(|mesh| MeshBatch { mesh, count })
            })
            .collect();

        Ok(MeshDrawable {
            batches: batches?,
            instance_buffer: Some(instance_buffer),
        })
    }
}

pub struct MeshRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    pub bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for MeshRenderer {
    type DrawData = MeshDrawable;

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
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
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

                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None, //Some(wgpu::Face::Back), // TODO(andreas): Need to specify from outside if mesh is CW or CCW?
                    ..Default::default()
                },
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        MeshRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

        let Some(instance_buffer) = &draw_data.instance_buffer else {
            return Ok(()); // Instance buffer was empty.
        };

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        let instance_buffer = pools.buffers.get_resource(instance_buffer)?;
        pass.set_vertex_buffer(0, instance_buffer.slice(..));
        let mut instance_start_index = 0;

        for mesh_batch in &draw_data.batches {
            let vertex_buffer_combined = pools
                .buffers
                .get_resource(&mesh_batch.mesh.vertex_buffer_combined)?;
            let index_buffer = pools.buffers.get_resource(&mesh_batch.mesh.index_buffer)?;

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

            let instance_range = instance_start_index..(instance_start_index + mesh_batch.count);

            for material in &mesh_batch.mesh.materials {
                debug_assert!(mesh_batch.count > 0);

                let bind_group = pools.bind_groups.get_resource(&material.bind_group)?;
                pass.set_bind_group(1, bind_group, &[]);

                pass.draw_indexed(material.index_range.clone(), 0, instance_range.clone());
            }

            instance_start_index = instance_range.end;
        }

        Ok(())
    }
}
