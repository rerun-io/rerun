//! Mesh renderer.
//!
//! Uses instancing to render instances of the same mesh in a single draw call.

use itertools::Itertools as _;

use crate::{
    include_file,
    mesh::{Mesh, MeshVertex},
    mesh_manager::MeshHandle,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayoutDesc, BindGroupLayoutHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::{RenderPipelineDesc, RenderPipelineHandle},
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

#[derive(Clone)]
struct MeshBatch {
    mesh: Mesh,
    count: u32,
}

#[derive(Clone)]
pub struct MeshDrawable {
    batches: Vec<MeshBatch>,
}

impl Drawable for MeshDrawable {
    type Renderer = MeshRenderer;
}

pub struct MeshInstance {
    pub mesh: MeshHandle,
    pub transformation: macaw::Conformal3,
}

impl MeshDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        instances: &[MeshInstance],
    ) -> anyhow::Result<Self> {
        let _mesh_renderer = ctx.renderers.get_or_create::<_, MeshRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        // Group by mesh to facilitate instancing.
        // We resolve the meshes here already, so the actual draw call doesn't need to know about the MeshManager.
        // Also, it helps failing early if something is wrong with a mesh!
        let mut batches = Vec::new();
        for (mesh_handle, instances) in &instances.iter().group_by(|instance| instance.mesh) {
            let mesh = ctx.meshes.get_mesh(mesh_handle)?;
            // TODO: upload transformation data
            batches.push(MeshBatch {
                mesh: mesh.clone(),
                count: instances.count() as _,
            });
        }

        Ok(MeshDrawable { batches })
    }
}

pub struct MeshRenderer {
    render_pipeline: RenderPipelineHandle,
    //bind_group_layout: BindGroupLayoutHandle,
}

impl Renderer for MeshRenderer {
    type DrawData = MeshDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        // let bind_group_layout = pools.bind_group_layouts.get_or_create(
        //     device,
        //     &BindGroupLayoutDesc {
        //         label: "mesh renderer".into(),
        //         entries: vec![], // TODO: No data at all??
        //     },
        // );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![shared_data.global_bindings.layout], //, bind_group_layout],
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
                vertex_buffers: vec![wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // Normal
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: std::mem::size_of::<f32>() as u64 * 3,
                            shader_location: 1,
                        },
                        // Texcoord
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: std::mem::size_of::<f32>() as u64 * (3 + 3),
                            shader_location: 2,
                        },
                    ],
                }],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
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

        MeshRenderer {
            render_pipeline,
            //bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(&pipeline.pipeline);

        for mesh_batch in &draw_data.batches {
            let vertex_and_index_buffer = pools
                .buffers
                .get_resource(&mesh_batch.mesh.vertex_and_index_buffer)?;

            pass.set_vertex_buffer(
                0,
                vertex_and_index_buffer
                    .buffer
                    .slice(mesh_batch.mesh.vertex_buffer_range.clone()),
            );
            pass.set_index_buffer(
                vertex_and_index_buffer
                    .buffer
                    .slice(mesh_batch.mesh.index_buffer_range.clone()),
                wgpu::IndexFormat::Uint32,
            );

            for material in &mesh_batch.mesh.materials {
                debug_assert!(mesh_batch.count > 0);
                pass.draw_indexed(material.index_range.clone(), 0, 0..mesh_batch.count);
            }
        }

        Ok(())
    }
}
